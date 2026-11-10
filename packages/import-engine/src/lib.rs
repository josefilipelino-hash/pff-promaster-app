//! # momento-import-engine
//!
//! Fluxo de uma importação:
//!
//! 1. [`collect_files`] expande ficheiros e pastas (opcionalmente recursivas), ignorando
//!    ficheiros ocultos e pastas de projeto `.momento`.
//! 2. [`create_session`] regista todos os caminhos no catálogo com estado `pending`.
//! 3. [`run_session`] processa os pendentes em paralelo (rayon). Cada ficheiro é lido uma
//!    vez: deteção do formato pelo conteúdo, hash BLAKE3, dimensões, EXIF e, no modo
//!    cópia, cópia verificada. Uma única thread grava no catálogo em lotes.
//! 4. Se a aplicação terminar a meio, os itens ainda `pending` são retomados por
//!    [`run_session`] na próxima abertura do projeto.
//!
//! Os originais nunca são alterados nem apagados.

use momento_core_types::{AssetId, CoreError, CoreResult, ImportSessionId};
use momento_database::assets::{InsertResult, NewAsset};
use momento_database::import::{ImportMode, ItemOutcome, ItemState, SessionCounts};
use momento_database::Catalog;
use momento_image_codecs::{
    detect, detect::SNIFF_LEN, format_id, is_missing_codec, CodecMetadata, CodecRegistry,
    FileFormat, FormatFamily, SupportLevel,
};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use std::sync::{mpsc, Mutex};

/// Tamanho máximo do lote de gravação no catálogo.
const BATCH: usize = 32;
/// Intervalo máximo entre gravações (progresso fluido com ficheiros grandes).
const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub mode: ImportMode,
    pub recursive: bool,
    /// Pasta dos originais do projeto (obrigatória no modo cópia).
    pub originals_dir: Option<PathBuf>,
    /// Pasta do projeto: nunca é importada (contém a cache de miniaturas).
    pub project_dir: Option<PathBuf>,
    /// Número de threads de processamento.
    pub threads: usize,
    /// Verificar cada cópia (reler e comparar o hash BLAKE3). Ligado por omissão.
    pub verify: bool,
    /// Segunda cópia, para um disco de segurança (ORIGINAL → Trabalho + ORIGINAL → Backup).
    pub backup_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub session_id: i64,
    pub total: u64,
    pub processed: u64,
    pub imported: u64,
    pub duplicates: u64,
    pub skipped: u64,
    pub errors: u64,
}

impl ImportProgress {
    fn from_counts(session: ImportSessionId, c: &SessionCounts) -> Self {
        ImportProgress {
            session_id: session.0,
            total: c.total,
            processed: c.total - c.pending,
            imported: c.imported,
            duplicates: c.duplicate + c.already_imported,
            skipped: c.unsupported + c.missing_codec,
            errors: c.error + c.corrupt,
        }
    }
}

/// Controlo de uma importação em curso.
#[derive(Debug, Default)]
pub struct ImportControl {
    cancel: AtomicBool,
    suspend: AtomicBool,
}

impl ImportControl {
    /// O utilizador cancelou: a sessão termina e os itens pendentes não são retomados.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    /// A aplicação vai fechar: a sessão fica por terminar e é retomada na próxima abertura.
    pub fn suspend(&self) {
        self.suspend.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
    fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Relaxed) || self.suspend.load(Ordering::Relaxed)
    }
}

/// Eventos emitidos durante a importação (para a interface e para as miniaturas).
#[derive(Debug, Clone)]
pub enum ImportEvent {
    Progress(ImportProgress),
    /// Fotografias acabadas de inserir (para gerar miniaturas).
    AssetsAdded(Vec<AssetId>),
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn inside(path: &Path, dir: Option<&Path>) -> bool {
    dir.is_some_and(|d| path.starts_with(d))
}

/// Expande a lista de caminhos escolhidos pelo utilizador.
///
/// - ficheiros indicados explicitamente entram sempre (mesmo que não sejam imagens: serão
///   registados como não suportados, para o utilizador saber porquê);
/// - pastas: ficheiros diretos ou recursivos; ficheiros e pastas ocultos são ignorados,
///   bem como pastas `*.momento` e a pasta do projeto atual.
pub fn collect_files(
    paths: &[PathBuf],
    recursive: bool,
    project_dir: Option<&Path>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for p in paths {
        if p.is_file() {
            if !inside(p, project_dir) && seen.insert(p.clone()) {
                out.push(p.clone());
            }
            continue;
        }
        if !p.is_dir() {
            // Caminho inexistente/inacessível: entra na lista e aparece no relatório com o
            // erro real (Fase 15 — antes era ignorado em silêncio).
            if seen.insert(p.clone()) {
                out.push(p.clone());
            }
            continue;
        }
        let mut walker = walkdir::WalkDir::new(p).follow_links(false).sort_by_file_name();
        if !recursive {
            walker = walker.max_depth(1);
        }
        let iter = walker.into_iter().filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            if e.depth() == 0 {
                return true;
            }
            if is_hidden(&name) {
                return false;
            }
            if e.file_type().is_dir()
                && (name.ends_with(".momento") || inside(e.path(), project_dir))
            {
                return false;
            }
            true
        });
        for e in iter {
            match e {
                Ok(e) => {
                    if e.file_type().is_file() && seen.insert(e.path().to_path_buf()) {
                        out.push(e.path().to_path_buf());
                    }
                }
                // Pasta ilegível (permissões, cartão com erros): registada como item com erro
                // para o relatório — nunca um "concluído" com fotografias em falta.
                Err(err) => {
                    if let Some(path) = err.path() {
                        if seen.insert(path.to_path_buf()) {
                            out.push(path.to_path_buf());
                        }
                    }
                }
            }
        }
    }
    out
}

/// Cria a sessão e regista os ficheiros a importar.
pub fn create_session(
    catalog: &mut Catalog,
    paths: &[PathBuf],
    options: &ImportOptions,
) -> CoreResult<(ImportSessionId, usize)> {
    if options.mode == ImportMode::Copy && options.originals_dir.is_none() {
        return Err(CoreError::Other("o modo cópia exige a pasta dos originais".into()));
    }
    let files = collect_files(paths, options.recursive, options.project_dir.as_deref());
    let session = catalog.create_import_session(options.mode, options.recursive)?;
    let as_str: Vec<String> = files.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    catalog.add_import_items(session, &as_str)?;
    Ok((session, files.len()))
}

enum Processed {
    Asset(Box<NewAsset>),
    Skip(ItemState, Option<String>),
}

struct Shared<'a> {
    registry: &'a CodecRegistry,
    options: &'a ImportOptions,
    session: ImportSessionId,
    /// Hashes das fotografias já no catálogo (antes desta importação).
    catalog_hashes: HashSet<[u8; 32]>,
    /// Hashes já no catálogo ou reclamados por um ficheiro desta importação.
    hashes: Mutex<HashSet<[u8; 32]>>,
    /// Destinos reservados no modo cópia (evita colisões entre threads).
    reserved: Mutex<HashSet<PathBuf>>,
}

fn mtime(meta: &std::fs::Metadata) -> Option<i64> {
    meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

fn sanitize(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '.' { c } else { '_' })
        .collect();
    // Nunca "." nem ".." (EXIF corrompido não pode tirar a cópia da pasta dos originais).
    let t = t.trim_start_matches('.').trim_matches('_').to_string();
    if t.is_empty() {
        "Sem_camara".into()
    } else {
        t
    }
}

/// Destino no modo cópia: `<originais>/<câmara>/<nome>`; colisões recebem sufixo `_1`, `_2`…
/// Destino de uma cópia em `base/<câmara>/`. Se o nome já existir com **o mesmo
/// conteúdo** (cópia feita numa importação interrompida), é reutilizado — nada de
/// duplicados `_1` órfãos. Devolve (caminho, já existia).
fn copy_destination(
    shared: &Shared,
    base: &Path,
    file_name: &str,
    camera: Option<&str>,
    hash: &[u8; 32],
) -> CoreResult<(PathBuf, bool)> {
    let dir = base.join(sanitize(camera.unwrap_or("Sem_camara")));
    let (stem, ext) = match file_name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (file_name.to_string(), String::new()),
    };
    let mut reserved = shared.reserved.lock().unwrap_or_else(|e| e.into_inner());
    for n in 0..10_000 {
        let name = if n == 0 { format!("{stem}{ext}") } else { format!("{stem}_{n}{ext}") };
        let candidate = dir.join(&name);
        // Sem distinguir maiúsculas: em macOS/Windows IMG_0001.JPG e img_0001.jpg são o
        // mesmo ficheiro — reservar só por bytes deixava uma cópia substituir a outra.
        let key = dir.join(name.to_lowercase());
        if reserved.contains(&key) {
            continue;
        }
        if !candidate.exists() {
            reserved.insert(key);
            return Ok((candidate, false));
        }
        if std::fs::read(&candidate).is_ok_and(|d| <[u8; 32]>::from(blake3::hash(&d)) == *hash) {
            reserved.insert(key);
            return Ok((candidate, true));
        }
    }
    Err(CoreError::Other("não foi possível encontrar um nome livre".into()))
}

/// Cópia atómica (temporário + `fsync` + `rename`) e, se pedido, verificada relendo o
/// destino. Um encerramento a meio deixa só um temporário `.<nome>.<pid>-<n>.tmp`, que é
/// removido na abertura seguinte — nunca um ficheiro meio escrito com o nome final.
fn copy_verified(
    shared: &Shared,
    base: &Path,
    file_name: &str,
    camera: Option<&str>,
    data: &[u8],
    hash: &[u8; 32],
) -> CoreResult<PathBuf> {
    use std::io::Write;
    let (dest, existed) = copy_destination(shared, base, file_name, camera, hash)?;
    if existed {
        return Ok(dest);
    }
    let dir = dest.parent().ok_or_else(|| CoreError::Other("destino sem pasta".into()))?;
    std::fs::create_dir_all(dir)?;
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let tmp = dir.join(format!(
        ".{}.{}-{}.tmp",
        dest.file_name().and_then(|n| n.to_str()).unwrap_or("f"),
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let res = (|| -> CoreResult<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
        drop(f);
        if shared.options.verify {
            let back: [u8; 32] = blake3::hash(&std::fs::read(&tmp)?).into();
            if back != *hash {
                return Err(CoreError::Corrupt("a cópia não coincide com o original".into()));
            }
        }
        // Nunca substituir um ficheiro que entretanto apareceu com este nome.
        if dest.exists() {
            return Err(CoreError::Other(format!("{} já existe no destino", dest.display())));
        }
        std::fs::rename(&tmp, &dest)?;
        Ok(())
    })();
    if let Err(e) = res {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(dest)
}

/// Limite de segurança por ficheiro (as maiores fotografias — RAW de 150–200 MP, TIFF de
/// 16 bits — ficam muito abaixo; vídeos de vários GB não).
const MAX_PHOTO_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Liberta o hash reclamado quando a fotografia afinal não entra (cópia falhou): um
/// ficheiro idêntico noutra pasta do cartão pode ainda entrar.
fn release(shared: &Shared, hash: &[u8; 32]) {
    shared.hashes.lock().unwrap_or_else(|e| e.into_inner()).remove(hash);
}

fn process_file(path_str: &str, shared: &Shared) -> Processed {
    match process_file_inner(path_str, shared) {
        Ok(p) => p,
        Err(e) => Processed::Skip(ItemState::Error, Some(e.to_string())),
    }
}

fn process_file_inner(path_str: &str, shared: &Shared) -> CoreResult<Processed> {
    // "Já importada" decide-se pelo CONTEÚDO, nunca só pelo caminho (Fase 15): o cartão de
    // um segundo fotógrafo, ou o mesmo cartão formatado, monta no mesmo sítio e a câmara
    // recomeça em IMG_0001 — o caminho repete-se com fotografias diferentes.
    let path = Path::new(path_str);
    let meta = std::fs::metadata(path)?;
    if meta.is_dir() {
        // Só chega aqui uma pasta que não foi possível percorrer.
        std::fs::read_dir(path)?;
        return Err(CoreError::Other("não foi possível ler o conteúdo desta pasta".into()));
    }
    let ext = path.extension().and_then(|e| e.to_str());
    // Formato pelo cabeçalho ANTES de ler o ficheiro inteiro: vídeos de vários GB no cartão
    // não são carregados para a memória.
    let head = {
        use std::io::Read;
        let mut buf = Vec::with_capacity(SNIFF_LEN);
        std::fs::File::open(path)?.take(SNIFF_LEN as u64).read_to_end(&mut buf)?;
        buf
    };
    let Some(det) = detect(&head, ext) else {
        return Ok(Processed::Skip(
            ItemState::Unsupported,
            Some("formato desconhecido ou não é uma imagem".into()),
        ));
    };
    match det.format.family() {
        FormatFamily::Vector | FormatFamily::Document => {
            return Ok(Processed::Skip(
                ItemState::Unsupported,
                Some(format!("{} não é uma fotografia (gráfico/documento)", format_id(det.format))),
            ));
        }
        _ => {}
    }
    if !shared.registry.supports_decode(det.format) {
        return Ok(Processed::Skip(
            ItemState::MissingCodec,
            Some(format!(
                "formato {} reconhecido, mas sem descodificador nesta plataforma",
                format_id(det.format)
            )),
        ));
    }
    if meta.len() > MAX_PHOTO_BYTES {
        return Ok(Processed::Skip(
            ItemState::Unsupported,
            Some(format!(
                "ficheiro com {} MB: grande demais para uma fotografia",
                meta.len() / 1_048_576
            )),
        ));
    }
    let data = std::fs::read(path)?;
    let hash: [u8; 32] = blake3::hash(&data).into();
    if shared.catalog_hashes.contains(&hash) {
        return Ok(Processed::Skip(ItemState::AlreadyImported, None));
    }
    {
        let mut hashes = shared.hashes.lock().unwrap_or_else(|e| e.into_inner());
        if !hashes.insert(hash) {
            return Ok(Processed::Skip(
                ItemState::Duplicate,
                Some("conteúdo idêntico a outra fotografia".into()),
            ));
        }
    }
    let probe = match shared.registry.probe(&data, det.format) {
        Ok(p) => p,
        Err(e) => {
            // Libertar o hash: o ficheiro não entrou no catálogo.
            shared.hashes.lock().unwrap_or_else(|e| e.into_inner()).remove(&hash);
            return Ok(classify_probe_error(e, det.format, data.len()));
        }
    };
    let codec_meta = shared.registry.metadata(&data, det.format).unwrap_or_default();
    // EXIF genérico; os valores do adaptador (ex.: LibRaw, mais fiável em RAW) têm prioridade.
    let exif = momento_metadata::read_exif(&data);
    let exif = merge_metadata(exif, &codec_meta);
    let codec_id =
        shared.registry.candidates(det.format, |c| c.decode).first().map(|c| c.id().to_string());
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

    let camera = exif.camera_model.as_deref();
    let (final_path, source_path) = match shared.options.mode {
        ImportMode::Reference => (path_str.to_string(), None),
        ImportMode::Copy => {
            let base = shared
                .options
                .originals_dir
                .as_ref()
                .ok_or_else(|| CoreError::Other("sem pasta dos originais".into()))?;
            let dest = copy_verified(shared, base, &file_name, camera, &data, &hash)
                .inspect_err(|_| release(shared, &hash))?;
            (dest.to_string_lossy().into_owned(), Some(path_str.to_string()))
        }
    };
    // Segunda cópia (disco de segurança), com a mesma verificação.
    let backup_path = match &shared.options.backup_dir {
        Some(b) => Some(
            copy_verified(shared, b, &file_name, camera, &data, &hash)
                .inspect_err(|_| release(shared, &hash))?
                .to_string_lossy()
                .into_owned(),
        ),
        None => None,
    };

    Ok(Processed::Asset(Box::new(NewAsset {
        backup_path,
        path: final_path,
        source_path,
        file_name,
        file_size: meta.len(),
        file_mtime: mtime(&meta),
        content_hash: hash,
        format: format_id(det.format).to_string(),
        width: probe.width,
        height: probe.height,
        orientation: exif.orientation,
        capture_time: exif.capture_time,
        capture_offset: exif.capture_offset,
        camera_make: exif.camera_make,
        camera_model: exif.camera_model,
        camera_serial: exif.camera_serial,
        lens: exif.lens,
        iso: exif.iso,
        exposure_time: exif.exposure_time,
        f_number: exif.f_number,
        focal_length: exif.focal_length,
        focal_length_35mm: exif.focal_length_35mm,
        import_session_id: Some(shared.session),
        bit_depth: Some(probe.bits_per_sample).filter(|b| *b > 0),
        color_space: codec_meta.color_space.clone(),
        codec: codec_id,
        codec_detail: codec_meta.detail.clone(),
        embedded_preview: codec_meta
            .embedded_preview
            .filter(|(w, h)| *w > 0 && *h > 0)
            .map(|(w, h)| format!("{w}×{h}")),
        partial_note: match &codec_meta.support {
            Some(SupportLevel::Partial(n)) => Some(n.clone()),
            _ => None,
        },
        gps: exif.gps,
    })))
}

/// Tamanho abaixo do qual um RAW recusado pela LibRaw é tratado como danificado: nenhum
/// RAW real de câmara tem menos de 256 KB.
const SMALLEST_PLAUSIBLE_RAW: usize = 256 * 1024;

/// Classifica um erro de leitura do cabeçalho num estado compreensível pelo utilizador.
fn classify_probe_error(e: CoreError, format: FileFormat, size: usize) -> Processed {
    if is_missing_codec(&e) {
        return Processed::Skip(ItemState::MissingCodec, Some(e.to_string()));
    }
    match e {
        CoreError::Corrupt(m) => Processed::Skip(ItemState::Corrupt, Some(m)),
        CoreError::UnsupportedFormat(m)
            if format.family() == FormatFamily::Raw && size < SMALLEST_PLAUSIBLE_RAW =>
        {
            Processed::Skip(
                ItemState::Corrupt,
                Some(format!("ficheiro RAW incompleto ou danificado ({size} bytes): {m}")),
            )
        }
        CoreError::UnsupportedFormat(m) if format.family() == FormatFamily::Raw => Processed::Skip(
            ItemState::Unsupported,
            Some(format!("câmara ou variante RAW não suportada pela LibRaw {}: {m}", "0.22.3")),
        ),
        CoreError::UnsupportedFormat(m) => Processed::Skip(ItemState::Unsupported, Some(m)),
        CoreError::Io(e) => Processed::Skip(ItemState::Error, Some(e.to_string())),
        other => Processed::Skip(ItemState::Corrupt, Some(other.to_string())),
    }
}

/// Combina o EXIF genérico com os metadados do adaptador (que têm prioridade).
fn merge_metadata(
    mut e: momento_metadata::ExifSummary,
    c: &CodecMetadata,
) -> momento_metadata::ExifSummary {
    if let Some(o) = c.orientation {
        e.orientation = o;
    }
    macro_rules! prefer {
        ($($f:ident),*) => { $( if c.$f.is_some() { e.$f = c.$f.clone(); } )* };
    }
    prefer!(
        capture_time,
        camera_make,
        camera_model,
        lens,
        iso,
        exposure_time,
        f_number,
        focal_length,
        focal_length_35mm
    );
    e
}

/// Processa os itens pendentes de uma sessão. Bloqueia até terminar ou ser cancelada.
/// Devolve as contagens finais.
pub fn run_session(
    catalog_path: &Path,
    session: ImportSessionId,
    options: &ImportOptions,
    registry: &CodecRegistry,
    control: &ImportControl,
    on_event: &(dyn Fn(ImportEvent) + Sync),
) -> CoreResult<SessionCounts> {
    let mut catalog = Catalog::open_existing(catalog_path)?;
    let pending = catalog.pending_import_items(session)?;
    let shared = Shared {
        registry,
        options,
        session,
        catalog_hashes: catalog.content_hashes()?,
        hashes: Mutex::new(catalog.content_hashes()?),
        reserved: Mutex::new(HashSet::new()),
    };
    on_event(ImportEvent::Progress(ImportProgress::from_counts(
        session,
        &catalog.import_session_counts(session)?,
    )));

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(options.threads.max(1))
        .thread_name(|i| format!("import-{i}"))
        .build()
        .map_err(|e| CoreError::Other(e.to_string()))?;

    let (tx, rx) = mpsc::sync_channel::<(String, Processed)>(BATCH);
    // Erro ao gravar no catálogo (disco cheio, catálogo ocupado): os produtores param e o
    // canal fecha — antes ficavam bloqueados em `send` e a importação (e o fecho do
    // projeto, que espera por ela) nunca terminava (Fase 15).
    let abort = std::sync::atomic::AtomicBool::new(false);
    let result = std::thread::scope(|scope| -> CoreResult<()> {
        let rx = rx;
        let shared_ref = &shared;
        let pending_ref = &pending;
        let abort_ref = &abort;
        scope.spawn(move || {
            pool.install(|| {
                pending_ref.par_iter().for_each_with(tx, |tx, p| {
                    if control.stopped() || abort_ref.load(Ordering::SeqCst) {
                        return;
                    }
                    let r = process_file(p, shared_ref);
                    let _ = tx.send((p.clone(), r));
                });
            });
        });

        // Escritor único: grava em lotes.
        let mut batch: Vec<(String, Processed)> = Vec::with_capacity(BATCH);
        let flush = |batch: &mut Vec<(String, Processed)>,
                     catalog: &mut Catalog|
         -> CoreResult<()> {
            if batch.is_empty() {
                return Ok(());
            }
            let mut outcomes = Vec::with_capacity(batch.len());
            let mut new_assets = Vec::new();
            let mut asset_paths = Vec::new();
            for (path, p) in batch.drain(..) {
                match p {
                    Processed::Asset(a) => {
                        asset_paths.push(path);
                        new_assets.push(*a);
                    }
                    Processed::Skip(state, message) => outcomes.push(ItemOutcome {
                        source_path: path,
                        state,
                        asset_id: None,
                        message,
                    }),
                }
            }
            let mut added = Vec::new();
            if !new_assets.is_empty() {
                let results = catalog.insert_assets(&new_assets)?;
                for ((path, asset), res) in asset_paths.into_iter().zip(&new_assets).zip(results) {
                    let (state, id, msg) = match res {
                        InsertResult::Inserted(id) => {
                            added.push(id);
                            (ItemState::Imported, Some(id), None)
                        }
                        InsertResult::DuplicateContent(id) => {
                            // Cópia nossa, redundante — mas só se NÃO for o ficheiro da
                            // fotografia que já está no catálogo (a cópia pode ter reutilizado
                            // um ficheiro idêntico existente): nunca apagar esse (Fase 15).
                            let theirs: Option<String> = catalog
                                .connection()
                                .query_row("SELECT path FROM asset WHERE id = ?1", [id.0], |r| {
                                    r.get(0)
                                })
                                .ok();
                            if asset.source_path.is_some()
                                && theirs.as_deref() != Some(asset.path.as_str())
                                && asset.backup_path.as_deref() != Some(asset.path.as_str())
                            {
                                let _ = std::fs::remove_file(&asset.path);
                            }
                            (
                                ItemState::Duplicate,
                                Some(id),
                                Some("conteúdo idêntico a outra fotografia".into()),
                            )
                        }
                        InsertResult::DuplicatePath(id) => {
                            // O mesmo caminho já está no catálogo. Se o conteúdo é OUTRO (cartão
                            // reutilizado/formatado, ficheiro substituído), não é "já importada":
                            // a fotografia nova não entrou e a catalogada já não está neste
                            // ficheiro — o utilizador tem de saber (Fase 15).
                            let theirs: Option<Vec<u8>> = catalog
                                .connection()
                                .query_row(
                                    "SELECT content_hash FROM asset WHERE id = ?1",
                                    [id.0],
                                    |r| r.get(0),
                                )
                                .ok();
                            if theirs.as_deref().is_some_and(|h| h != asset.content_hash.as_slice())
                            {
                                (
                                    ItemState::Error,
                                    Some(id),
                                    Some("já existe no projeto uma fotografia DIFERENTE com este caminho (o ficheiro foi substituído — p. ex. cartão reutilizado); copie estes ficheiros para outra pasta e importe-os de lá".into()),
                                )
                            } else {
                                (ItemState::AlreadyImported, Some(id), None)
                            }
                        }
                    };
                    outcomes.push(ItemOutcome {
                        source_path: path,
                        state,
                        asset_id: id,
                        message: msg,
                    });
                }
            }
            catalog.record_import_outcomes(session, &outcomes)?;
            if !added.is_empty() {
                on_event(ImportEvent::AssetsAdded(added));
            }
            on_event(ImportEvent::Progress(ImportProgress::from_counts(
                session,
                &catalog.import_session_counts(session)?,
            )));
            Ok(())
        };
        let mut last_flush = std::time::Instant::now();
        loop {
            match rx.recv_timeout(FLUSH_INTERVAL) {
                Ok(item) => batch.push(item),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if batch.len() >= BATCH || (!batch.is_empty() && last_flush.elapsed() >= FLUSH_INTERVAL)
            {
                if let Err(e) = flush(&mut batch, &mut catalog) {
                    abort.store(true, Ordering::SeqCst);
                    drop(rx);
                    return Err(e);
                }
                last_flush = std::time::Instant::now();
            }
        }
        flush(&mut batch, &mut catalog)
    });
    result?;

    let counts = catalog.import_session_counts(session)?;
    if control.suspend.load(Ordering::SeqCst) && counts.pending > 0 {
        tracing::info!(session = session.0, ?counts, "importação suspensa (será retomada)");
        return Ok(counts);
    }
    let cancelled = control.cancel.load(Ordering::SeqCst);
    catalog.finish_import_session(session, cancelled)?;
    tracing::info!(session = session.0, ?counts, cancelled, "importação terminada");
    Ok(counts)
}

#[cfg(test)]
mod tests;
