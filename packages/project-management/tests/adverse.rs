//! Fase 15 — condições adversas de um trabalho real: disco cheio (exportação, importação,
//! disco de segurança, catálogo), destino só de leitura, originais num disco só de leitura,
//! permissões negadas, e fugas de recursos (threads, descritores, memória) ao abrir/fechar
//! projetos repetidamente.
//!
//! Disco cheio e só de leitura precisam de sistemas de ficheiros reais (o root ignora
//! permissões). Preparação (Linux, root):
//!
//! ```text
//! mkdir -p /tmp/fs/cheio /tmp/fs/ro
//! mount -t tmpfs -o size=6m tmpfs /tmp/fs/cheio
//! mount -t tmpfs -o size=8m tmpfs /tmp/fs/ro   # copiar fotografias e depois:
//! MOMENTO_FULL_FS=/tmp/fs/cheio MOMENTO_RO_FS=/tmp/fs/ro cargo test --release --test adverse
//! ```
//!
//! O teste de permissões corre como utilizador sem privilégios:
//! `setpriv --reuid=65534 --regid=65534 --clear-groups <binário> permissions -- --exact`.
//! Sem as variáveis/sem privilégios reduzidos, cada cenário diz que foi saltado.

use image::{Rgb, RgbImage};
use momento_database::import::ImportMode;
use momento_export_engine::{Format, Recipe};
use momento_import_engine::ImportProgress;
use momento_project_management::export::{ExportProgress, ExportRequest, JobState};
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Rec {
    summaries: Mutex<Vec<ImportSummary>>,
}
impl SessionEvents for Rec {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, s: &ImportSummary) {
        self.summaries.lock().unwrap().push(s.clone());
    }
    fn thumbnails(&self, _: &ThumbBatch) {}
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    v.sort();
    v
}

fn blake(p: &Path) -> String {
    blake3::hash(&std::fs::read(p).unwrap()).to_hex().to_string()
}

/// JPEG com textura (≈ 0,5–1 MB): ocupa espaço como uma fotografia.
fn photo(seed: u32) -> Vec<u8> {
    let base = test_image(1600, 1067, seed, 1);
    let mut s = 0x2545_f491_4f6c_dd1du64 ^ seed as u64;
    let img = RgbImage::from_fn(1600, 1067, |x, y| {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let n = ((s >> 58) as i32) - 32;
        Rgb(base.get_pixel(x, y).0.map(|c| (c as i32 + n).clamp(0, 255) as u8))
    });
    jpeg_bytes(&img, Some(&spec_for(seed, 1)), 92)
}

fn source(dir: &Path, n: u32) -> Vec<PathBuf> {
    (0..n)
        .map(|i| {
            let p = dir.join(format!("IMG_{i:03}.jpg"));
            write(&p, &photo(i)).unwrap();
            p
        })
        .collect()
}

fn project(work: &Path, originals: Option<&Path>) -> ProjectPaths {
    create_project(&NewProject {
        name: "Casamento".into(),
        client_name: "Teste".into(),
        event_type: "wedding".into(),
        event_date: Some("2026-06-20".into()),
        location: String::new(),
        originals_dir: originals.map(|o| o.to_string_lossy().into_owned()),
        work_dir: work.to_string_lossy().into_owned(),
    })
    .unwrap()
}

fn import(
    s: &Arc<ProjectSession>,
    rec: &Rec,
    src: &Path,
    mode: ImportMode,
    backup: Option<&Path>,
) -> ImportSummary {
    s.start_import_with(
        &ImportRequest { paths: vec![src.to_string_lossy().into_owned()], mode, recursive: true },
        &ImportSafety {
            verify: true,
            backup_dir: backup.map(|b| b.to_string_lossy().into_owned()),
        },
    )
    .unwrap();
    s.wait_import();
    rec.summaries.lock().unwrap().last().cloned().unwrap()
}

fn wait_job(s: &ProjectSession, id: u64) -> ExportProgress {
    let t = Instant::now();
    loop {
        let p = s.export_jobs().into_iter().find(|p| p.id == id).unwrap();
        if matches!(
            p.state,
            JobState::Done | JobState::Cancelled | JobState::Failed | JobState::Paused
        ) {
            return p;
        }
        assert!(t.elapsed() < Duration::from_secs(300), "exportação presa: {p:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn tiff16() -> Recipe {
    let mut r = momento_export_engine::presets::builtin()
        .into_iter()
        .find(|r| r.format == Format::Tiff)
        .unwrap();
    r.naming.subfolder = String::new();
    r
}

fn jpeg() -> Recipe {
    let mut r = momento_export_engine::presets::builtin()
        .into_iter()
        .find(|r| r.id == "builtin:galeria")
        .unwrap();
    r.naming.subfolder = String::new();
    r
}

/// Enche o sistema de ficheiros (deixa `keep` bytes livres).
fn fill(dir: &Path, keep: u64) -> PathBuf {
    let f = dir.join("enchimento.bin");
    let free = fs4::available_space(dir).unwrap();
    let n = free.saturating_sub(keep);
    let mut file = std::fs::File::create(&f).unwrap();
    use std::io::Write;
    let chunk = vec![0u8; 64 * 1024];
    let mut left = n;
    while left > 0 {
        let k = left.min(chunk.len() as u64) as usize;
        if file.write_all(&chunk[..k]).is_err() {
            break;
        }
        left -= k as u64;
    }
    let _ = file.sync_all();
    f
}

/// Os cenários partilham o mesmo disco pequeno: um de cada vez.
static FULL_FS: Mutex<()> = Mutex::new(());

fn env_dir(var: &str) -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var(var).ok()?);
    d.is_dir().then_some(d)
}

fn clean(dir: &Path) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            std::fs::remove_dir_all(&p).unwrap();
        } else {
            std::fs::remove_file(&p).unwrap();
        }
    }
}

/// Exportar para um disco cheio: aviso antes, erro claro, nenhum ficheiro meio escrito, a
/// fila não fica presa e a exportação seguinte (noutro disco) funciona.
#[test]
fn export_to_full_disk() {
    let Some(full) = env_dir("MOMENTO_FULL_FS") else {
        eprintln!("SALTADO: MOMENTO_FULL_FS não definido");
        return;
    };
    let _one_at_a_time = FULL_FS.lock().unwrap_or_else(|e| e.into_inner());
    clean(&full);
    let tmp = tempfile::tempdir().unwrap();
    source(&tmp.path().join("fonte"), 4);
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let paths = project(&work, None);
    let rec = Arc::new(Rec::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    import(&s, &rec, &tmp.path().join("fonte"), ImportMode::Reference, None);
    let keys: Vec<i64> = s.list_photos().unwrap().iter().map(|p| p.id.0).collect();
    let dest = full.join("Entrega");
    let req = ExportRequest {
        keys: keys.clone(),
        recipes: vec![tiff16()],
        destination: dest.to_string_lossy().into_owned(),
        name: "TIFF".into(),
        ..Default::default()
    };
    // 4 × 1600×1067×6 bytes ≈ 41 MB num disco de 6 MB: a estimativa avisa.
    let est = s.estimate_export(&req).unwrap();
    assert!(!est.enough_space, "a estimativa devia avisar: {est:?}");
    // Mesmo assim (o utilizador insiste): falha limpa.
    let id = s.start_export(req.clone()).unwrap();
    let p = wait_job(&s, id);
    eprintln!(
        "disco cheio → {:?} ({:?}), {} feitos, {} falhados",
        p.state, p.paused_reason, p.files_done, p.files_failed
    );
    // Comportamento esperado (Fase 13): pausa com o motivo, à espera de espaço — nunca
    // "concluída", nunca um ficheiro a meio. O utilizador liberta espaço ou cancela.
    assert_eq!(p.state, JobState::Paused);
    assert_eq!(p.paused_reason.as_deref(), Some("noSpace"));
    s.cancel_export(id);
    let t = Instant::now();
    let p = loop {
        let p = s.export_jobs().into_iter().find(|p| p.id == id).unwrap();
        if p.state != JobState::Paused || t.elapsed() > Duration::from_secs(10) {
            break p;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(p.state, JobState::Cancelled, "{p:?}");
    // Ficheiros que existem estão completos (abrem) — nenhum meio escrito; nenhum temporário.
    for f in files(&dest) {
        assert!(!f.to_string_lossy().ends_with(".tmp"), "temporário deixado: {}", f.display());
        image::open(&f).unwrap_or_else(|e| panic!("ficheiro incompleto {}: {e}", f.display()));
    }
    // A fila continua a trabalhar.
    let ok = tmp.path().join("ok");
    let p2 = wait_job(
        &s,
        s.start_export(ExportRequest {
            recipes: vec![jpeg()],
            destination: ok.to_string_lossy().into_owned(),
            ..req
        })
        .unwrap(),
    );
    assert_eq!(p2.state, JobState::Done, "{:?}", p2.errors);
    assert_eq!(files(&ok).len(), 4);
    assert!(s.validate_catalog().unwrap().ok());
    s.close();
    clean(&full);
}

/// Importação por cópia para um disco de originais que enche: as fotografias que não cabem
/// ficam com erro (não entram pela metade), os originais do cartão ficam intactos, e
/// depois de libertar espaço a mesma importação traz as que faltavam (sem duplicados).
#[test]
fn import_copy_to_full_disk_then_resume() {
    let Some(full) = env_dir("MOMENTO_FULL_FS") else {
        eprintln!("SALTADO: MOMENTO_FULL_FS não definido");
        return;
    };
    let _one_at_a_time = FULL_FS.lock().unwrap_or_else(|e| e.into_inner());
    clean(&full);
    let tmp = tempfile::tempdir().unwrap();
    let card = tmp.path().join("cartão");
    // Tantas fotografias quantas cabem no disco vazio (com folga), para a 2.ª passagem.
    let one = photo(0).len() as u64;
    let n = ((fs4::available_space(&full).unwrap() * 7 / 10) / one).clamp(4, 10) as u32;
    let srcs = source(&card, n);
    let hashes: Vec<String> = srcs.iter().map(|p| blake(p)).collect();
    let total: u64 = srcs.iter().map(|p| std::fs::metadata(p).unwrap().len()).sum();
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let originals = full.join("Originais");
    let filler = fill(&full, total / 2);
    let paths = project(&work, Some(&originals));
    let rec = Arc::new(Rec::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    let sum = import(&s, &rec, &card, ImportMode::Copy, None);
    eprintln!("disco de originais cheio → {:?}", sum.counts);
    assert!(sum.counts.imported > 0 && sum.counts.imported < n as u64, "{:?}", sum.counts);
    assert_eq!(sum.counts.imported + sum.counts.error, n as u64, "{:?}", sum.issues);
    assert!(sum.issues.iter().all(|i| i.message.as_deref().is_some_and(|m| !m.is_empty())));
    assert_eq!(s.list_photos().unwrap().len() as u64, sum.counts.imported);
    for f in files(&originals) {
        assert!(
            !f.file_name().unwrap().to_string_lossy().ends_with(".tmp"),
            "temporário: {}",
            f.display()
        );
        assert!(hashes.contains(&blake(&f)), "cópia incompleta: {}", f.display());
    }
    for (p, h) in srcs.iter().zip(&hashes) {
        assert_eq!(&blake(p), h);
    }
    // Libertar espaço e repetir: entram as que faltavam, as outras são reconhecidas.
    std::fs::remove_file(filler).unwrap();
    let sum2 = import(&s, &rec, &card, ImportMode::Copy, None);
    eprintln!("depois de libertar espaço → {:?}", sum2.counts);
    assert_eq!(s.list_photos().unwrap().len(), n as usize, "{:?}", sum2.counts);
    assert_eq!(
        files(&originals).len(),
        n as usize,
        "sem cópias duplicadas: {:?}",
        files(&originals)
    );
    assert!(s.validate_catalog().unwrap().ok());
    s.close();
    clean(&full);
}

/// Disco de segurança cheio durante a importação: o utilizador fica a saber quais as
/// fotografias que NÃO têm segunda cópia (nunca um "concluído" enganador).
#[test]
fn import_with_full_backup_disk() {
    let Some(full) = env_dir("MOMENTO_FULL_FS") else {
        eprintln!("SALTADO: MOMENTO_FULL_FS não definido");
        return;
    };
    let _one_at_a_time = FULL_FS.lock().unwrap_or_else(|e| e.into_inner());
    clean(&full);
    let tmp = tempfile::tempdir().unwrap();
    let card = tmp.path().join("cartão");
    let one = photo(0).len() as u64;
    let n = ((fs4::available_space(&full).unwrap() * 7 / 10) / one).clamp(4, 8) as u32;
    let srcs = source(&card, n);
    let total: u64 = srcs.iter().map(|p| std::fs::metadata(p).unwrap().len()).sum();
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let originals = tmp.path().join("Originais");
    let backup = full.join("Segurança");
    std::fs::create_dir_all(&backup).unwrap();
    let filler = fill(&full, total / 2);
    let paths = project(&work, Some(&originals));
    let rec = Arc::new(Rec::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    let sum = import(&s, &rec, &card, ImportMode::Copy, Some(&backup));
    eprintln!(
        "disco de segurança cheio → {:?} {:?}",
        sum.counts,
        sum.issues.iter().map(|i| i.message.clone()).take(2).collect::<Vec<_>>()
    );
    let in_backup =
        files(&backup).iter().filter(|f| !f.to_string_lossy().ends_with(".tmp")).count() as u64;
    assert!(in_backup < n as u64, "o disco devia ter enchido");
    // Cada fotografia sem cópia de segurança aparece como problema, com mensagem.
    assert!(
        sum.issues.len() as u64 >= n as u64 - in_backup,
        "fotografias sem backup sem aviso: {:?}",
        sum
    );
    assert!(
        files(&backup).iter().all(|f| !f.to_string_lossy().ends_with(".tmp")),
        "temporários no backup"
    );
    std::fs::remove_file(filler).unwrap();
    // Com espaço: repetir a importação completa tudo, sem cópias duplicadas.
    let sum2 = import(&s, &rec, &card, ImportMode::Copy, Some(&backup));
    eprintln!("depois de libertar o disco de segurança → {:?}", sum2.counts);
    assert_eq!(s.list_photos().unwrap().len(), n as usize, "{:?}", sum2.issues);
    assert_eq!(files(&originals).len(), n as usize, "{:?}", files(&originals));
    assert_eq!(files(&backup).len(), n as usize, "{:?}", files(&backup));
    assert!(s.validate_catalog().unwrap().ok());
    s.close();
    clean(&full);
}

/// Catálogo num disco que enche: gravar uma edição falha com erro (sem pânico, sem
/// corromper); as edições anteriores ficam; com espaço, volta a gravar.
#[test]
fn catalog_on_full_disk() {
    let Some(full) = env_dir("MOMENTO_FULL_FS") else {
        eprintln!("SALTADO: MOMENTO_FULL_FS não definido");
        return;
    };
    let _one_at_a_time = FULL_FS.lock().unwrap_or_else(|e| e.into_inner());
    clean(&full);
    let tmp = tempfile::tempdir().unwrap();
    source(&tmp.path().join("fonte"), 3);
    let paths = project(&full, None);
    let rec = Arc::new(Rec::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    import(&s, &rec, &tmp.path().join("fonte"), ImportMode::Reference, None);
    let k = s.list_photos().unwrap()[0].id.0;
    let mut st = s.edit_view(k).unwrap().settings;
    st.light.exposure = 0.5;
    s.commit_edit(k, "exposure", &st).unwrap();
    s.set_stars(&[s.list_photos().unwrap()[1].asset], 4).unwrap();
    let filler = fill(&full, 0);
    let mut failures = 0;
    for i in 0..30 {
        st.light.contrast = i as f32;
        if s.commit_edit(k, "contrast", &st).is_err() {
            failures += 1;
        }
        if s.set_stars(&[s.list_photos().unwrap()[2].asset], (i % 5 + 1) as u8).is_err() {
            failures += 1;
        }
    }
    eprintln!("catálogo sem espaço: {failures} gravações recusadas de 60");
    std::fs::remove_file(filler).unwrap();
    st.light.contrast = 33.0;
    s.commit_edit(k, "contrast", &st).unwrap();
    assert!(s.validate_catalog().unwrap().ok());
    s.close();
    drop(s);
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    let v = s.edit_view(k).unwrap().settings;
    assert_eq!((v.light.exposure, v.light.contrast), (0.5, 33.0), "edições depois de reabrir");
    assert_eq!(s.list_photos().unwrap()[1].stars, 4);
    s.close();
    clean(&full);
}

/// Destino só de leitura: erro claro e imediato; originais num disco só de leitura
/// (cartão protegido contra escrita): importa, mostra, exporta.
#[test]
fn read_only_volumes() {
    let Some(ro) = env_dir("MOMENTO_RO_FS") else {
        eprintln!("SALTADO: MOMENTO_RO_FS não definido");
        return;
    };
    let card: Vec<PathBuf> =
        files(&ro).into_iter().filter(|p| p.extension().is_some_and(|e| e == "jpg")).collect();
    assert!(card.len() >= 2, "MOMENTO_RO_FS deve ter fotografias .jpg");
    let tmp = tempfile::tempdir().unwrap();
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let paths = project(&work, None);
    let rec = Arc::new(Rec::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    let sum = import(&s, &rec, &ro, ImportMode::Reference, None);
    assert_eq!(sum.counts.imported as usize, card.len(), "{sum:?}");
    let t = Instant::now();
    while s.thumbnails_remaining() > 0 {
        assert!(t.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(50));
    }
    let keys: Vec<i64> = s.list_photos().unwrap().iter().map(|p| p.id.0).collect();
    let mut st = s.edit_view(keys[0]).unwrap().settings;
    st.light.exposure = 0.3;
    s.commit_edit(keys[0], "exposure", &st).unwrap();
    // Exportar PARA o disco só de leitura.
    let bad = ro.join("Entrega");
    let req = ExportRequest {
        keys: keys.clone(),
        recipes: vec![jpeg()],
        destination: bad.to_string_lossy().into_owned(),
        name: "x".into(),
        ..Default::default()
    };
    let r = s.estimate_export(&req);
    eprintln!(
        "estimativa para destino só de leitura: {:?}",
        r.as_ref().map(|e| (&e.errors, e.enough_space))
    );
    let outcome = s.start_export(req.clone()).map(|id| wait_job(&s, id));
    match &outcome {
        Err(e) => eprintln!("recusada ao iniciar: {e}"),
        Ok(p) => {
            eprintln!("destino só de leitura → {:?} {:?}", p.state, p.errors.first());
            assert_ne!(p.state, JobState::Done);
            assert!(!p.errors.is_empty());
        }
    }
    // Exportar DO disco só de leitura para um disco normal.
    let ok = tmp.path().join("ok");
    let p = wait_job(
        &s,
        s.start_export(ExportRequest { destination: ok.to_string_lossy().into_owned(), ..req })
            .unwrap(),
    );
    assert_eq!(p.state, JobState::Done, "{:?}", p.errors);
    assert_eq!(files(&ok).len(), card.len());
    // Mover para o lixo um original num disco só de leitura: recusado, catálogo intacto.
    let rows = s.list_photos().unwrap();
    let r = s.move_to_trash(&[rows[1].id.0], true);
    eprintln!("lixo num disco só de leitura: {:?}", r.as_ref().map(|x| x.failed.len()));
    assert!(card.iter().all(|p| p.exists()), "nada pode desaparecer do disco só de leitura");
    assert_eq!(
        s.list_photos().unwrap().len(),
        rows.len(),
        "a fotografia não pode sair do catálogo se o ficheiro ficou"
    );
    assert!(s.validate_catalog().unwrap().ok());
    s.close();
}

fn non_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .map(|l| l.split_whitespace().nth(2) != Some("0"))
        })
        .unwrap_or(!cfg!(target_os = "linux"))
}

/// Permissões negadas (como utilizador sem privilégios): original ilegível e pasta de
/// destino sem escrita — erros claros, sem pânico, o resto continua.
#[test]
fn permissions() {
    if !non_root() {
        eprintln!("SALTADO: a correr como root (ver o cabeçalho para correr com setpriv)");
        return;
    }
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let card = tmp.path().join("cartão");
    let srcs = source(&card, 4);
    std::fs::set_permissions(&srcs[0], std::fs::Permissions::from_mode(0o000)).unwrap();
    let locked = card.join("trancada");
    std::fs::create_dir_all(&locked).unwrap();
    write(&locked.join("IMG_X.jpg"), &photo(77)).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let paths = project(&work, None);
    let rec = Arc::new(Rec::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    let sum = import(&s, &rec, &card, ImportMode::Reference, None);
    eprintln!(
        "permissões → {:?} {:?}",
        sum.counts,
        sum.issues.iter().map(|i| (&i.source_path, &i.message)).collect::<Vec<_>>()
    );
    assert_eq!(sum.counts.imported, 3, "{sum:?}");
    assert!(
        sum.issues.iter().any(|i| i.source_path.ends_with("IMG_000.jpg")),
        "original ilegível sem aviso: {sum:?}"
    );
    let keys: Vec<i64> = s.list_photos().unwrap().iter().map(|p| p.id.0).collect();
    let dest = tmp.path().join("sem-escrita");
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o555)).unwrap();
    let req = ExportRequest {
        keys,
        recipes: vec![jpeg()],
        destination: dest.to_string_lossy().into_owned(),
        name: "x".into(),
        ..Default::default()
    };
    match s.start_export(req).map(|id| wait_job(&s, id)) {
        Err(e) => eprintln!("exportação recusada ao iniciar: {e}"),
        Ok(p) => {
            eprintln!("destino sem escrita → {:?} {:?}", p.state, p.errors.first());
            assert_ne!(p.state, JobState::Done);
            assert!(!p.errors.is_empty());
        }
    }
    assert!(s.validate_catalog().unwrap().ok());
    s.close();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn threads() -> usize {
    std::fs::read_dir("/proc/self/task").map(|d| d.count()).unwrap_or(0)
}
fn fds() -> usize {
    std::fs::read_dir("/proc/self/fd").map(|d| d.count()).unwrap_or(0)
}
fn rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok()))
        .map(|p| p * 4096 / 1_048_576)
        .unwrap_or(0)
}

/// Fugas de recursos: abrir e fechar o projeto 25 vezes (com miniaturas, previews,
/// renders do editor e uma exportação em cada) não pode acumular threads, descritores de
/// ficheiros nem memória.
#[test]
fn no_leaks_across_open_close_cycles() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    source(&tmp.path().join("fonte"), 6);
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let paths = project(&work, None);
    let rec = Arc::new(Rec::default());
    {
        let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
        import(&s, &rec, &tmp.path().join("fonte"), ImportMode::Reference, None);
        s.close();
    }
    let mut samples = Vec::new();
    for cycle in 0..25 {
        let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
        let rows = s.list_photos().unwrap();
        for r in &rows {
            s.serve_image(r.id.0, momento_project_management::paths::CacheLevel::Preview).unwrap();
            let v = s.edit_view(r.id.0).unwrap();
            let mut st = v.settings.clone();
            st.light.exposure = (cycle % 5) as f32 * 0.1;
            s.render_edit(r.id.0, &st, 800, 800, RenderQuality::Fast, false).unwrap();
        }
        if cycle % 5 == 0 {
            let out = tmp.path().join(format!("out{cycle}"));
            let keys: Vec<i64> = rows.iter().map(|p| p.id.0).collect();
            let p = wait_job(
                &s,
                s.start_export(ExportRequest {
                    keys,
                    recipes: vec![jpeg()],
                    destination: out.to_string_lossy().into_owned(),
                    name: "x".into(),
                    ..Default::default()
                })
                .unwrap(),
            );
            assert_eq!(p.state, JobState::Done);
        }
        s.close();
        drop(s);
        std::thread::sleep(Duration::from_millis(100));
        samples.push((threads(), fds(), rss_mb()));
    }
    eprintln!("(threads, fds, RSS MB) por ciclo: {samples:?}");
    let (t5, f5, _) = samples[4];
    let (t, f, _) = *samples.last().unwrap();
    // RSS oscila ~50 MB entre ciclos (o alocador devolve memória a ritmos diferentes e os
    // outros testes deste ficheiro correm em paralelo no mesmo processo): compara-se o
    // patamar (máximo de 5 ciclos) do início com o do fim, não amostras isoladas.
    let plateau = |w: &[(usize, usize, u64)]| w.iter().map(|x| x.2).max().unwrap_or(0);
    let (m5, m) = (plateau(&samples[4..9]), plateau(&samples[samples.len() - 5..]));
    assert!(t <= t5 + 2, "threads a acumular: {t5} → {t}");
    assert!(f <= f5 + 2, "descritores a acumular: {f5} → {f}");
    assert!(m <= m5 + 60, "memória a crescer: {m5} MB → {m} MB");
}

/// Edição com uma LUT que não está neste computador (projeto copiado sem a pasta `luts`):
/// a fotografia NÃO é exportada com outras cores — falha com o motivo; as outras saem.
#[test]
fn export_refuses_edit_with_missing_lut() {
    let tmp = tempfile::tempdir().unwrap();
    source(&tmp.path().join("fonte"), 3);
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let paths = project(&work, None);
    let rec = Arc::new(Rec::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    import(&s, &rec, &tmp.path().join("fonte"), ImportMode::Reference, None);
    let keys: Vec<i64> = s.list_photos().unwrap().iter().map(|p| p.id.0).collect();
    let mut st = s.edit_view(keys[0]).unwrap().settings;
    st.lut.id = "lut-que-nao-existe".into();
    st.lut.name = "Filme 400".into();
    st.lut.amount = 100.0;
    s.commit_edit(keys[0], "lut", &st).unwrap();
    let out = tmp.path().join("out");
    let p = wait_job(
        &s,
        s.start_export(ExportRequest {
            keys,
            recipes: vec![jpeg()],
            destination: out.to_string_lossy().into_owned(),
            name: "x".into(),
            ..Default::default()
        })
        .unwrap(),
    );
    eprintln!("LUT em falta → {:?} {:?}", p.state, p.errors.first());
    assert_eq!((p.files_done, p.files_failed), (2, 1));
    assert!(p.errors[0].message.contains("Filme 400"), "{:?}", p.errors);
    assert_eq!(files(&out).len(), 2);
    s.close();
}

/// Fase 15 (encontrado na aplicação real): original substituído por OUTRA fotografia no
/// mesmo caminho (cartão reutilizado no modo "manter no local"). A verificação marca-o
/// "em falta" e a exportação recusa-o — nunca exporta a fotografia nova com as edições
/// da antiga.
#[test]
fn replaced_original_is_never_exported_as_the_catalogued_photo() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("cartão");
    let files_src = source(&src, 3);
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let paths = project(&work, None);
    let rec = Arc::new(Rec::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    import(&s, &rec, &src, ImportMode::Reference, None);
    let keys: Vec<i64> = s.list_photos().unwrap().iter().map(|p| p.id.0).collect();
    // Outra fotografia gravada por cima da primeira.
    write(&files_src[0], &photo(999)).unwrap();
    let sum = s.check_availability().unwrap();
    eprintln!("original substituído → {sum:?}");
    assert!(s
        .list_photos()
        .unwrap()
        .iter()
        .any(|p| p.availability == momento_database::safety::Availability::Missing));
    let out = tmp.path().join("out");
    let p = wait_job(
        &s,
        s.start_export(ExportRequest {
            keys,
            recipes: vec![jpeg()],
            destination: out.to_string_lossy().into_owned(),
            name: "x".into(),
            ..Default::default()
        })
        .unwrap(),
    );
    eprintln!("exportação → {:?} {:?}", p.state, p.errors.first());
    assert_eq!((p.files_done, p.files_failed), (2, 1), "{:?}", p.errors);
    s.close();
}
