//! Fase 11 — álbum pelo caminho real da aplicação: criar, montar automaticamente com os
//! momentos, verificar, mudar um layout, exportar JPEG/PDF pela fila a partir dos
//! originais, presets de laboratório; originais intactos.

use momento_album::export::ExportKind;
use momento_album::{AlbumSpec, LabPreset};
use momento_database::import::ImportMode;
use momento_database::library::Library;
use momento_import_engine::ImportProgress;
use momento_project_management::album::LabStore;
use momento_project_management::export::JobState;
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Quiet;
impl SessionEvents for Quiet {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &ThumbBatch) {}
}

fn hash(p: &Path) -> u64 {
    let d = std::fs::read(p).unwrap();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&d, &mut h);
    std::hash::Hasher::finish(&h)
}

#[test]
fn album_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    let dest = tmp.path().join("laboratorio");
    std::fs::create_dir_all(&work).unwrap();
    for i in 0..12 {
        let (w, h) = if i % 3 == 0 { (1200, 1800) } else { (1800, 1200) };
        write(
            &src.join(format!("IMG_{i:02}.jpg")),
            &jpeg_bytes(&test_image(w, h, i, 1), Some(&spec_for(i, 1)), 90),
        )
        .unwrap();
    }
    let originals: Vec<(std::path::PathBuf, u64)> = std::fs::read_dir(&src)
        .unwrap()
        .flatten()
        .map(|e| {
            let h = hash(&e.path());
            (e.path(), h)
        })
        .collect();
    let paths = create_project(&NewProject {
        name: "Casamento".into(),
        client_name: "Ana & Pedro".into(),
        event_type: "wedding".into(),
        event_date: Some("2026-06-15".into()),
        location: String::new(),
        originals_dir: None,
        work_dir: work.to_string_lossy().into_owned(),
    })
    .unwrap();
    let s = ProjectSession::open(&paths.root, Arc::new(Quiet)).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![src.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    let rows = s.list_photos().unwrap();
    let ms = s.list_moments().unwrap();
    let mid = |k: &str| ms.iter().find(|m| m.kind.as_deref() == Some(k)).unwrap().id;
    // 0–3 preparação, 4–7 cerimónia, 8–11 festa (ordem de captura).
    let mut sorted = rows.clone();
    sorted.sort_by(|a, b| a.capture_time.cmp(&b.capture_time));
    for (i, r) in sorted.iter().enumerate() {
        let k = ["bride_prep", "ceremony", "party"][i / 4];
        s.assign_moment(&[r.asset], Some(mid(k))).unwrap();
    }

    // Criar (20×20 cm, 8 páginas, 150 DPI para o teste ser rápido) e montar.
    let spec = AlbumSpec {
        width_mm: 200.0,
        height_mm: 200.0,
        pages: 8,
        dpi: 150,
        format: "20x20".into(),
        confirmed: true,
        ..Default::default()
    };
    let a = s.create_album("Ana & Pedro", spec).unwrap();
    assert_eq!(s.list_albums().unwrap().len(), 1);
    let r = s.auto_album(&a.id, Default::default()).unwrap();
    assert_eq!(r.album.spreads.len(), 4);
    assert_eq!(r.used, 12, "todas as fotografias cabem");
    let order: Vec<i64> = r.album.photos();
    let pos = |k: i64| sorted.iter().position(|x| x.id.0 == k).unwrap();
    // Depois da abertura, a narrativa segue os momentos.
    let chapters: Vec<usize> = order[1..].iter().map(|k| pos(*k) / 4).collect();
    assert!(chapters.windows(2).all(|w| w[0] <= w[1]), "{chapters:?}");
    let stored = s.get_album(&a.id).unwrap();
    assert_eq!(stored, r.album, "gravado no catálogo");
    let issues = s.album_preflight(&stored).unwrap();
    assert!(
        !issues.iter().any(|i| i.severity == momento_album::preflight::Severity::Error),
        "{issues:?}"
    );

    // Mudar o layout de um spread (as mesmas fotografias).
    let before: Vec<i64> = stored.spreads[1].slots.iter().filter_map(|x| x.photo).collect();
    let tpl = momento_album::layouts::templates_for(before.len()).last().unwrap().clone();
    let changed = s.apply_album_layout(&stored, 1, &tpl, 15.0, 5.0, &[]).unwrap();
    assert_eq!(changed.spreads[1].template, tpl);
    let after: Vec<i64> = changed.spreads[1].slots.iter().filter_map(|x| x.photo).collect();
    assert_eq!(before, after);
    s.save_album(&changed).unwrap();

    // Exportar JPEG por spread e PDF de impressão, pela fila.
    let wait = |id: u64| {
        let t = Instant::now();
        loop {
            match s.job_state(id) {
                Some(JobState::Done) => return,
                Some(JobState::Failed) => {
                    panic!("{:?}", s.export_jobs().into_iter().find(|p| p.id == id))
                }
                _ => {}
            }
            assert!(t.elapsed() < Duration::from_secs(300));
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    wait(s.start_album_export(&a.id, ExportKind::Jpeg, dest.to_str().unwrap()).unwrap());
    let mut files: Vec<_> = std::fs::read_dir(&dest).unwrap().flatten().map(|e| e.path()).collect();
    files.sort();
    assert_eq!(files.len(), 4);
    let img = image::open(&files[0]).unwrap();
    // (400 + 2×3) mm a 150 DPI.
    assert_eq!(img.width(), (406.0f32 / 25.4 * 150.0).round() as u32);
    wait(s.start_album_export(&a.id, ExportKind::PrintPdf, dest.to_str().unwrap()).unwrap());
    let pdf = std::fs::read(dest.join("ana-e-pedro-impressao.pdf")).unwrap();
    let t = String::from_utf8_lossy(&pdf);
    assert!(t.contains("/Count 4") && t.contains("/TrimBox"));
    let job = s.export_jobs().into_iter().last().unwrap();
    assert!(job.index.unwrap().ends_with(".pdf"));

    // Presets de laboratório (biblioteca global).
    let lib = Library::open_in_memory().unwrap();
    let store = LabStore { lib: &lib };
    let p = store
        .save(&LabPreset {
            name: "Laboratório X".into(),
            dpi: 300,
            bleed_mm: 5.0,
            safe_mm: 15.0,
            gutter_mm: 20.0,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(store.list().unwrap()[0].bleed_mm, 5.0);
    assert!(store.delete(&p.id).unwrap());
    assert!(s.delete_album(&a.id).unwrap());

    for (p, h) in &originals {
        assert_eq!(hash(p), *h, "{p:?}");
    }
    s.close();
}
