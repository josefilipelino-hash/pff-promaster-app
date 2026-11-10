//! Fase 13 — testes deliberados de falhas, pelo caminho real da aplicação:
//! encerramento (SIGKILL) durante a importação e durante a exportação, disco removido,
//! ficheiro desaparecido, base de dados bloqueada, projeto aberto duas vezes, RAW
//! corrompido e catálogo danificado.
//!
//! Para "matar a meio", o teste volta a lançar o próprio executável como processo filho
//! (`MOMENTO_DRILL=import|export`), deixa-o trabalhar e mata-o com `Child::kill()`
//! (SIGKILL em Unix, TerminateProcess no Windows): nada corre no fim do processo.

use momento_database::import::ImportMode;
use momento_database::safety::Availability;
use momento_database::Catalog;
use momento_export_engine::presets::builtin;
use momento_import_engine::ImportProgress;
use momento_project_management::export::{ExportRequest, JobState};
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Quiet;
impl SessionEvents for Quiet {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &ThumbBatch) {}
}

fn photos(dir: &Path, n: u32, w: u32, h: u32) {
    for i in 0..n {
        let img = test_image(w, h, i, 1);
        write(&dir.join(format!("IMG_{i:04}.jpg")), &jpeg_bytes(&img, Some(&spec_for(i, 1)), 80))
            .unwrap();
    }
}

fn project(work: &Path, originals: Option<&Path>) -> PathBuf {
    create_project(&NewProject {
        name: "Resiliência".into(),
        client_name: "Teste".into(),
        event_type: "wedding".into(),
        event_date: None,
        location: String::new(),
        originals_dir: originals.map(|p| p.to_string_lossy().into_owned()),
        work_dir: work.to_string_lossy().into_owned(),
    })
    .unwrap()
    .root
}

fn open(root: &Path) -> Arc<ProjectSession> {
    ProjectSession::open(root, Arc::new(Quiet)).unwrap()
}

fn files_in(dir: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return v };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            v.extend(files_in(&p));
        } else {
            v.push(p);
        }
    }
    v.sort();
    v
}

fn spawn_drill(kind: &str, args: &[&Path]) -> std::process::Child {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", "drill_child", "--nocapture", "--test-threads=1"])
        .env("MOMENTO_DRILL", kind)
        .env(
            "MOMENTO_DRILL_ARGS",
            args.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>().join("\u{1f}"),
        )
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    c.spawn().unwrap()
}

/// Processo filho: importa ou exporta e fica à espera de ser morto.
#[test]
fn drill_child() {
    let Ok(kind) = std::env::var("MOMENTO_DRILL") else { return };
    let args: Vec<PathBuf> =
        std::env::var("MOMENTO_DRILL_ARGS").unwrap().split('\u{1f}').map(PathBuf::from).collect();
    let s = open(&args[0]);
    match kind.as_str() {
        "import" => {
            s.start_import_with(
                &ImportRequest {
                    paths: vec![args[1].to_string_lossy().into_owned()],
                    mode: ImportMode::Copy,
                    recursive: true,
                },
                &ImportSafety {
                    verify: true,
                    backup_dir: Some(args[2].to_string_lossy().into_owned()),
                },
            )
            .unwrap();
        }
        "export" => {
            let keys: Vec<i64> = s.list_photos().unwrap().iter().map(|r| r.id.0).collect();
            let mut r = builtin().into_iter().next().unwrap();
            r.naming.subfolder.clear();
            s.start_export(ExportRequest {
                keys,
                recipes: vec![r],
                destination: args[1].to_string_lossy().into_owned(),
                name: "Exportar".into(),
                ..Default::default()
            })
            .unwrap();
        }
        _ => {}
    }
    std::thread::sleep(Duration::from_secs(600));
}

fn kill_when(child: &mut std::process::Child, mut ready: impl FnMut() -> bool) {
    let t = Instant::now();
    while !ready() {
        assert!(t.elapsed() < Duration::from_secs(120), "o filho não começou");
        std::thread::sleep(Duration::from_millis(5));
    }
    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn killed_during_import_resumes_without_orphans_or_partial_files() {
    let tmp = tempfile::tempdir().unwrap();
    let (card, work, orig, backup) = (
        tmp.path().join("cartao"),
        tmp.path().join("trabalho"),
        tmp.path().join("originais"),
        tmp.path().join("backup"),
    );
    for d in [&work, &orig, &backup] {
        std::fs::create_dir_all(d).unwrap();
    }
    const N: usize = 400;
    photos(&card, N as u32, 640, 427);
    let root = project(&work, Some(&orig));
    let mut child = spawn_drill("import", &[&root, &card, &backup]);
    let cat = root.join("catalog.db");
    let mut at_kill = 0;
    kill_when(&mut child, || {
        at_kill = Catalog::open_existing(&cat).and_then(|c| c.photo_count()).unwrap_or(0) as usize;
        at_kill >= 10
    });
    let at_kill = Catalog::open_existing(&cat).unwrap().photo_count().unwrap() as usize;
    assert!(at_kill < N, "a importação terminou antes de ser interrompida ({at_kill}); aumentar N");

    // Reabrir: sessão interrompida detetada, catálogo validado por completo, importação retomada.
    let s = open(&root);
    let rep = s.open_report();
    assert!(rep.previous_unclean && rep.validated_full, "{rep:?}");
    s.wait_import();
    assert_eq!(s.list_photos().unwrap().len(), N, "todas as fotografias, sem duplicados");
    // Trabalho e backup: exatamente N ficheiros cada (sem `_1` órfãos nem temporários), todos
    // iguais aos originais do cartão.
    for dir in [&orig, &backup] {
        let fs = files_in(dir);
        assert_eq!(
            fs.len(),
            N,
            "{dir:?}: {:?}",
            fs.iter()
                .filter(|p| p.to_string_lossy().contains("_1.")
                    || p.to_string_lossy().ends_with(".tmp"))
                .collect::<Vec<_>>()
        );
    }
    let c = Catalog::open_existing(&cat).unwrap();
    for o in c.originals().unwrap() {
        assert_eq!(
            <[u8; 32]>::from(blake3::hash(&std::fs::read(&o.path).unwrap())),
            o.content_hash
        );
        let b = o.backup_path.expect("cópia de segurança registada");
        assert_eq!(<[u8; 32]>::from(blake3::hash(&std::fs::read(&b).unwrap())), o.content_hash);
    }
    assert!(c.validate(true).unwrap().ok());
    assert!(s.safety_events(10).unwrap().iter().any(|e| e.1 == "sessao_interrompida"));
    s.close();
    // Fecho limpo: a abertura seguinte não vê interrupção.
    let s = open(&root);
    assert!(!s.open_report().previous_unclean);
    s.close();
}

#[test]
fn killed_during_export_leaves_no_partial_files_and_resumes() {
    let tmp = tempfile::tempdir().unwrap();
    let (src, work, dest) =
        (tmp.path().join("fotos"), tmp.path().join("trabalho"), tmp.path().join("entrega"));
    std::fs::create_dir_all(&work).unwrap();
    const N: usize = 160;
    photos(&src, N as u32, 1200, 800);
    let root = project(&work, None);
    let s = open(&root);
    s.start_import(&ImportRequest {
        paths: vec![src.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    s.close();
    drop(s);

    let mut child = spawn_drill("export", &[&root, &dest]);
    kill_when(&mut child, || {
        files_in(&dest).iter().filter(|p| p.extension().is_some_and(|e| e == "jpg")).count() >= 5
    });
    let done_at_kill =
        files_in(&dest).iter().filter(|p| p.extension().is_some_and(|e| e == "jpg")).count();
    assert!(done_at_kill < N, "a exportação terminou antes de ser interrompida");
    // Nenhum ficheiro final meio escrito: todos os .jpg abrem.
    for f in files_in(&dest).iter().filter(|p| p.extension().is_some_and(|e| e == "jpg")) {
        image::open(f).unwrap_or_else(|e| panic!("{f:?} meio escrito: {e}"));
    }

    let s = open(&root);
    assert!(s.open_report().previous_unclean);
    let pending = s.pending_exports().unwrap();
    assert_eq!(pending.len(), 1, "exportação interrompida registada");
    assert!(
        pending[0].done >= 1 && (pending[0].done as usize) < N && pending[0].total as usize == N,
        "{pending:?}"
    );
    let id = s.resume_export(&pending[0].id).unwrap();
    let t = Instant::now();
    while !matches!(
        s.export_jobs().iter().find(|j| j.id == id).map(|j| j.state),
        Some(JobState::Done)
    ) {
        assert!(t.elapsed() < Duration::from_secs(300));
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(s.pending_exports().unwrap().is_empty(), "registo removido no fim");
    let out: Vec<PathBuf> = files_in(&dest);
    assert!(
        out.iter().all(|p| !p.file_name().unwrap().to_string_lossy().ends_with(".tmp")),
        "temporários limpos"
    );
    let jpgs = out.iter().filter(|p| p.extension().is_some_and(|e| e == "jpg")).count();
    // Cada fotografia exportada; no máximo a que estava a ser gravada no instante do
    // encerramento sai duas vezes (_1) — documentado em RELIABILITY.md.
    assert!((N..=N + 1).contains(&jpgs), "{jpgs} ficheiros para {N} fotografias");
    for f in out.iter().filter(|p| p.extension().is_some_and(|e| e == "jpg")) {
        image::open(f).unwrap();
    }
    s.close();
}

#[test]
fn disk_removed_goes_offline_and_reconnects_missing_file_relinks() {
    let tmp = tempfile::tempdir().unwrap();
    let disk = tmp.path().join("DiscoExterno");
    let src = disk.join("Casamento");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    photos(&src, 6, 320, 213);
    let root = project(&work, None);
    let s = open(&root);
    s.start_import(&ImportRequest {
        paths: vec![src.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    let t = Instant::now();
    while s.thumbnails_remaining() > 0 && t.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let keys: Vec<i64> = s.list_photos().unwrap().iter().map(|r| r.id.0).collect();

    // Disco "desligado".
    let unplugged = tmp.path().join("desligado");
    std::fs::rename(&disk, &unplugged).unwrap();
    let a = s.check_availability().unwrap();
    assert_eq!((a.offline, a.missing), (6, 0));
    assert!(
        s.list_photos().unwrap().iter().all(|r| r.availability == Availability::Offline),
        "referências mantidas"
    );
    // A miniatura já feita continua a ser servida; a preview pede o original → erro claro.
    assert!(s.serve_image(keys[0], paths::CacheLevel::Thumb).is_ok());
    let e = s.serve_image(keys[1], paths::CacheLevel::Preview).unwrap_err();
    assert!(matches!(e, momento_core_types::CoreError::Unavailable(_)), "{e:?}");
    // Exportar com o disco desligado: falha por ficheiro, a fila continua, nada é apagado.
    let dest = tmp.path().join("entrega");
    let id = s
        .start_export(ExportRequest {
            keys: keys.clone(),
            recipes: vec![builtin().into_iter().next().unwrap()],
            destination: dest.to_string_lossy().into_owned(),
            name: "x".into(),
            ..Default::default()
        })
        .unwrap();
    let t = Instant::now();
    let job = loop {
        let j = s.export_jobs().into_iter().find(|j| j.id == id).unwrap();
        if matches!(j.state, JobState::Done | JobState::Failed) {
            break j;
        }
        assert!(t.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(job.files_failed, 6);
    assert!(job.errors[0].message.contains("indisponível"), "{:?}", job.errors[0]);

    // Disco de volta → reconexão automática.
    std::fs::rename(&unplugged, &disk).unwrap();
    let a = s.check_availability().unwrap();
    assert_eq!((a.ok, a.reconnected), (6, 6));

    // Ficheiro desaparecido com o disco presente → em falta; relink pela pasta para onde
    // foi movido (confirmado pelo conteúdo).
    let moved = tmp.path().join("movidas");
    std::fs::create_dir_all(&moved).unwrap();
    std::fs::rename(src.join("IMG_0003.jpg"), moved.join("IMG_0003.jpg")).unwrap();
    let a = s.check_availability().unwrap();
    assert_eq!(a.missing, 1);
    let r = s.relink_folder(&moved).unwrap();
    assert_eq!((r.relinked, r.still_unavailable), (1, 0));
    // Relink para um ficheiro com outro conteúdo é recusado.
    std::fs::write(tmp.path().join("falso.jpg"), b"outro").unwrap();
    assert!(s.relink_file(keys[0], &tmp.path().join("falso.jpg")).is_err());

    // Remover só do catálogo: o ficheiro fica. Sem confirmação: recusado.
    assert!(s.remove_from_catalog(&keys[..1], false).is_err());
    let file0 = src.join("IMG_0000.jpg");
    assert!(file0.exists());
    assert_eq!(s.remove_from_catalog(&keys[..1], true).unwrap().removed, 1);
    assert!(file0.exists(), "remover do catálogo nunca apaga o original");
    assert_eq!(s.list_photos().unwrap().len(), 5);
    s.close();
}

#[test]
fn locked_database_waits_and_second_window_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let src = tmp.path().join("f");
    photos(&src, 3, 200, 133);
    let root = project(&work, None);
    let s = open(&root);
    s.start_import(&ImportRequest {
        paths: vec![src.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    // Mesmo projeto aberto outra vez (outra janela/instância): recusado.
    assert!(matches!(
        ProjectSession::open(&root, Arc::new(Quiet)),
        Err(momento_core_types::CoreError::ProjectInUse)
    ));

    // Outro processo segura a base de dados em escrita durante 1,5 s: a classificação espera
    // (busy_timeout) e é gravada quando o bloqueio sai — sem erro nem perda.
    let cat = root.join("catalog.db");
    let locker = std::thread::spawn(move || {
        let c = rusqlite::Connection::open(&cat).unwrap();
        c.execute_batch("BEGIN EXCLUSIVE").unwrap();
        std::thread::sleep(Duration::from_millis(1500));
        c.execute_batch("COMMIT").unwrap();
    });
    std::thread::sleep(Duration::from_millis(200));
    let ids: Vec<_> = s.list_photos().unwrap().iter().map(|r| r.asset).collect();
    let t = Instant::now();
    s.set_stars(&ids, 4).unwrap();
    assert!(t.elapsed() >= Duration::from_millis(1000), "esperou pelo bloqueio");
    locker.join().unwrap();
    assert!(s.list_photos().unwrap().iter().all(|r| r.stars == 4));
    s.close();
}

#[test]
fn corrupted_raw_and_damaged_catalog() {
    let tmp = tempfile::tempdir().unwrap();
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let src = tmp.path().join("f");
    photos(&src, 4, 320, 213);
    // RAW corrompido: cabeçalho TIFF/CR2 válido, dados truncados e lixo.
    let mut bad = b"II*\x00\x10\x00\x00\x00CR\x02\x00".to_vec();
    bad.extend((0..40_000u32).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8));
    std::fs::write(src.join("CORROMPIDO.CR2"), &bad).unwrap();
    let root = project(&work, None);
    let s = open(&root);
    s.start_import(&ImportRequest {
        paths: vec![src.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    let summary = {
        s.wait_import();
        s.list_photos().unwrap()
    };
    assert_eq!(summary.len(), 4, "o RAW corrompido não entra; as outras sim; sem pânico");
    s.close();
    drop(s);

    // Catálogo danificado no disco (páginas sobrescritas): a abertura deteta, guarda o
    // danificado e restaura a cópia de segurança válida mais recente.
    let cat = root.join("catalog.db");
    let len = std::fs::metadata(&cat).unwrap().len();
    let mut data = std::fs::read(&cat).unwrap();
    for b in data.iter_mut().skip(4096).take((len as usize).saturating_sub(8192).min(65_536)) {
        *b ^= 0x5a;
    }
    std::fs::write(&cat, &data).unwrap();
    let s = open(&root);
    let rep = s.open_report();
    assert!(rep.restored_from.is_some() && rep.damaged_kept.is_some(), "{rep:?}");
    assert!(Path::new(rep.damaged_kept.as_ref().unwrap()).exists(), "o danificado nunca é apagado");
    assert!(s.validate_catalog().unwrap().ok());
    s.close();
}

/// Fase 15: um catálogo de uma versão MAIS RECENTE da aplicação (aberto por engano com uma
/// instalação antiga) não é "dano": a abertura recusa com a mensagem certa e NADA é
/// trocado — antes, era posto de lado e substituído por uma cópia antiga (trabalho perdido).
#[test]
fn newer_catalog_is_refused_not_replaced_by_an_old_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let src = tmp.path().join("f");
    photos(&src, 2, 320, 213);
    let root = project(&work, None);
    let s = open(&root);
    s.start_import(&ImportRequest {
        paths: vec![src.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    s.create_backup().unwrap();
    s.close();
    drop(s);
    let cat = root.join("catalog.db");
    rusqlite::Connection::open(&cat).unwrap().execute_batch("PRAGMA user_version = 999;").unwrap();
    let before = std::fs::read(&cat).unwrap();
    let err = ProjectSession::open(&root, Arc::new(Quiet)).err().expect("devia recusar");
    assert!(matches!(err, momento_core_types::CoreError::CatalogTooNew { .. }), "{err}");
    assert_eq!(std::fs::read(&cat).unwrap(), before, "o catálogo não pode ser trocado");
    let aside: Vec<_> = std::fs::read_dir(root.join("backups"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("danificado"))
        .collect();
    assert!(aside.is_empty(), "nada posto de lado");
}
