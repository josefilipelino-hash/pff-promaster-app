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
use momento_image_codecs::{detect, detect::SNIFF_LEN, format_id, CodecRegistry};
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
            skipped: c.unsupported,
            errors: c.error,
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
        for e in iter.flatten() {
            if e.file_type().is_file() && seen.insert(e.path().to_path_buf()) {
                out.push(e.path().to_path_buf());
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
    known_paths: HashSet<String>,
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
    let t = t.trim_matches('_').to_string();
    if t.is_empty() {
        "Sem_camara".into()
    } else {
        t
    }
}

/// Destino no modo cópia: `<originais>/<câmara>/<nome>`; colisões recebem sufixo `_1`, `_2`…
fn copy_destination(shared: &Shared, file_name: &str, camera: Option<&str>) -> CoreResult<PathBuf> {
    let base = shared
        .options
        .originals_dir
        .as_ref()
        .ok_or_else(|| CoreError::Other("sem pasta dos originais".into()))?;
    let dir = base.join(sanitize(camera.unwrap_or("Sem_camara")));
    let (stem, ext) = match file_name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (file_name.to_string(), String::new()),
    };
    let mut reserved = shared.reserved.lock().unwrap_or_else(|e| e.into_inner());
    for n in 0..10_000 {
        let name = if n == 0 { format!("{stem}{ext}") } else { format!("{stem}_{n}{ext}") };
        let candidate = dir.join(name);
        if !candidate.exists() && !reserved.contains(&candidate) {
            reserved.insert(candidate.clone());
            return Ok(candidate);
        }
    }
    Err(CoreError::Other("não foi possível encontrar um nome livre".into()))
}

fn process_file(path_str: &str, shared: &Shared) -> Processed {
    match process_file_inner(path_str, shared) {
        Ok(p) => p,
        Err(e) => Processed::Skip(ItemState::Error, Some(e.to_string())),
    }
}

fn process_file_inner(path_str: &str, shared: &Shared) -> CoreResult<Processed> {
    if shared.known_paths.contains(path_str) {
        return Ok(Processed::Skip(ItemState::AlreadyImported, None));
    }
    let path = Path::new(path_str);
    let meta = std::fs::metadata(path)?;
    let data = std::fs::read(path)?;
    let ext = path.extension().and_then(|e| e.to_str());
    let Some(det) = detect(&data[..data.len().min(SNIFF_LEN)], ext) else {
        return Ok(Processed::Skip(ItemState::Unsupported, Some("formato desconhecido".into())));
    };
    if !shared.registry.supports_decode(det.format) {
        return Ok(Processed::Skip(
            ItemState::Unsupported,
            Some(format!("formato {} ainda não suportado", format_id(det.format))),
        ));
    }
    let hash: [u8; 32] = blake3::hash(&data).into();
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
            return Err(e);
        }
    };
    let exif = momento_metadata::read_exif(&data);
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

    let (final_path, source_path) = match shared.options.mode {
        ImportMode::Reference => (path_str.to_string(), None),
        ImportMode::Copy => {
            let dest = copy_destination(shared, &file_name, exif.camera_model.as_deref())?;
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&dest, &data)?;
            // Verificação: reler do disco e comparar o hash.
            let verify: [u8; 32] = blake3::hash(&std::fs::read(&dest)?).into();
            if verify != hash {
                let _ = std::fs::remove_file(&dest);
                return Err(CoreError::Corrupt("a cópia não coincide com o original".into()));
            }
            (dest.to_string_lossy().into_owned(), Some(path_str.to_string()))
        }
    };

    Ok(Processed::Asset(Box::new(NewAsset {
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
    })))
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
        known_paths: catalog.known_paths()?,
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
    let result = std::thread::scope(|scope| -> CoreResult<()> {
        let shared_ref = &shared;
        let pending_ref = &pending;
        scope.spawn(move || {
            pool.install(|| {
                pending_ref.par_iter().for_each_with(tx, |tx, p| {
                    if control.stopped() {
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
                            if asset.source_path.is_some() {
                                let _ = std::fs::remove_file(&asset.path); // cópia nossa, redundante
                            }
                            (
                                ItemState::Duplicate,
                                Some(id),
                                Some("conteúdo idêntico a outra fotografia".into()),
                            )
                        }
                        InsertResult::DuplicatePath(id) => {
                            (ItemState::AlreadyImported, Some(id), None)
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
                flush(&mut batch, &mut catalog)?;
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
