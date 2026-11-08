//! Importação de ficheiros **reais** de vários formatos pelo caminho completo da aplicação.
//!
//! `MOMENTO_SAMPLES_DIR=/a:/b cargo test -p momento-project-management --release --test real_formats -- --nocapture`
//!
//! Ignorado sem a variável. Gera `import_report.md` em `MOMENTO_REPORT_DIR`.

use momento_database::import::ImportMode;
use momento_import_engine::ImportProgress;
use momento_project_management::paths::CacheLevel;
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Rec {
    st: Mutex<(Vec<ImportSummary>, u64)>,
    cv: Condvar,
}
impl SessionEvents for Rec {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, s: &ImportSummary) {
        self.st.lock().unwrap().0.push(s.clone());
    }
    fn thumbnails(&self, b: &ThumbBatch) {
        self.st.lock().unwrap().1 += (b.ready.len() + b.failed.len()) as u64;
        self.cv.notify_all();
    }
}

#[test]
fn import_real_files_of_all_formats() {
    let Ok(dirs) = std::env::var("MOMENTO_SAMPLES_DIR") else {
        eprintln!("MOMENTO_SAMPLES_DIR não definido: teste ignorado");
        return;
    };
    let out = PathBuf::from(
        std::env::var("MOMENTO_REPORT_DIR")
            .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned()),
    );
    std::fs::create_dir_all(&out).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let paths = create_project(&NewProject {
        name: "Formatos reais".into(),
        client_name: "Teste".into(),
        event_type: "other".into(),
        event_date: None,
        location: String::new(),
        originals_dir: None,
        work_dir: tmp.path().to_string_lossy().into_owned(),
    })
    .unwrap();
    let rec = Arc::new(Rec::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    let sources: Vec<String> = dirs.split(':').map(String::from).collect();
    let t = Instant::now();
    s.start_import(&ImportRequest { paths: sources, mode: ImportMode::Reference, recursive: true })
        .unwrap();
    s.wait_import();
    let import_time = t.elapsed();
    let summary = rec.st.lock().unwrap().0.last().cloned().unwrap();
    let imported = summary.counts.imported;

    // Esperar pelas miniaturas.
    let t2 = Instant::now();
    {
        let mut st = rec.st.lock().unwrap();
        while st.1 < imported && t2.elapsed() < Duration::from_secs(600) {
            st = rec.cv.wait_timeout(st, Duration::from_secs(1)).unwrap().0;
        }
    }
    let thumb_time = t2.elapsed();

    let mut md = String::from("| Ficheiro | Formato | Resultado | Dimensões | Bits | Cor | Câmara | Miniatura | Preview 2560 | Resolução total |\n|---|---|---|---|---|---|---|---|---|---|\n");
    let mut by_state: BTreeMap<String, u64> = BTreeMap::new();
    for p in s.list_photos().unwrap() {
        let d = s.photo_details(p.id).unwrap().unwrap();
        let thumb = if p.thumb == momento_database::assets::ThumbState::Ready {
            "✅".to_string()
        } else {
            format!("❌ {}", d.thumb_error.clone().unwrap_or_default())
        };
        let t = Instant::now();
        let pv = previews::ensure(&s.paths, s.registry(), p.id, CacheLevel::Preview);
        let pv_cell = match pv {
            Ok(_) => format!("✅ {:.0} ms", t.elapsed().as_secs_f64() * 1000.0),
            Err(e) => format!("❌ {e}"),
        };
        let t = Instant::now();
        let full = previews::ensure(&s.paths, s.registry(), p.id, CacheLevel::Full);
        let full_cell = match full {
            Ok(sv) if sv.path.starts_with(&paths.root) => {
                format!("✅ gerada {:.0} ms", t.elapsed().as_secs_f64() * 1000.0)
            }
            Ok(_) => "✅ original".to_string(),
            Err(e) => format!("❌ {e}"),
        };
        let result = match &d.partial_note {
            Some(n) => format!("⚠️ importada, suporte parcial — {n}"),
            None => "✅ importada".into(),
        };
        *by_state
            .entry(if d.partial_note.is_some() {
                "importada (parcial)".into()
            } else {
                "importada".into()
            })
            .or_default() += 1;
        writeln!(
            md,
            "| {} | {} | {result} | {}×{} | {} | {} | {} | {thumb} | {pv_cell} | {full_cell} |",
            d.file_name,
            d.format,
            p.width,
            p.height,
            d.bit_depth.map(|b| b.to_string()).unwrap_or("—".into()),
            d.color_space.clone().unwrap_or("—".into()),
            [d.camera_make.clone(), d.camera_model.clone()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" "),
        )
        .unwrap();
    }
    for i in &summary.issues {
        let state = format!("{:?}", i.state);
        *by_state.entry(state.clone()).or_default() += 1;
        let name =
            PathBuf::from(&i.source_path).file_name().unwrap().to_string_lossy().into_owned();
        writeln!(
            md,
            "| {name} | — | {} {state} — {} | — | — | — | — | — | — | — |",
            match i.state {
                momento_database::import::ItemState::Corrupt => "🟥",
                momento_database::import::ItemState::MissingCodec => "🟧",
                momento_database::import::ItemState::Duplicate
                | momento_database::import::ItemState::AlreadyImported => "⬜",
                _ => "⬛",
            },
            i.message.clone().unwrap_or_default()
        )
        .unwrap();
    }
    writeln!(
        md,
        "\nImportação: {import_time:.2?} · miniaturas: {thumb_time:.2?} · contagens: {:?}",
        summary.counts
    )
    .unwrap();
    writeln!(md, "Por estado: {by_state:?}").unwrap();
    std::fs::write(out.join("import_report.md"), &md).unwrap();
    println!("{md}");
    assert!(imported > 0);
    assert_eq!(summary.counts.pending, 0, "nada fica pendente");
    s.close();
}
