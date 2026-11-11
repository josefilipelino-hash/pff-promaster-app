//! Classificações para o Lightroom pelo caminho real: importar RAW reais, analisar, escrever
//! os `.xmp` (Melhor 5★, Alternativa 3★, Rever 2★) — os RAW nunca mudam.
//! Com `MOMENTO_SAMPLES_DIR` (RAW reais); sem ela, só a parte sem RAW.

use momento_database::import::ImportMode;
use momento_import_engine::ImportProgress;
use momento_project_management::lightroom::{stars_for_label, LightroomOptions};
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Rec;
impl SessionEvents for Rec {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &ThumbBatch) {}
}

#[test]
fn lightroom_ratings_from_the_assisted_selection() {
    let tmp = tempfile::tempdir().unwrap();
    let card = tmp.path().join("cartao");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&card).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    let samples = std::env::var("MOMENTO_SAMPLES_DIR").ok().map(PathBuf::from);
    let mut raws = Vec::new();
    if let Some(d) = &samples {
        for name in
            ["Nikon D810.nef", "Canon EOS R3.cr3", "Sony ILCE-7M4 (A7M4).ARW", "Pentax QS1.dng"]
        {
            let from = d.join(name);
            if from.is_file() {
                let to = card.join(name.replace(' ', "_"));
                std::fs::copy(&from, &to).unwrap();
                raws.push(to);
            }
        }
    }
    for i in 0..4 {
        write(
            &card.join(format!("JPG_{i}.jpg")),
            &jpeg_bytes(&test_image(900, 600, i, 1), Some(&spec_for(i, 1)), 90),
        )
        .unwrap();
    }
    let before: Vec<(PathBuf, Vec<u8>)> = std::fs::read_dir(&card)
        .unwrap()
        .flatten()
        .map(|e| (e.path(), std::fs::read(e.path()).unwrap()))
        .collect();
    let paths = create_project(&NewProject {
        name: "Lightroom".into(),
        client_name: "Teste".into(),
        event_type: "wedding".into(),
        event_date: None,
        location: String::new(),
        originals_dir: None,
        work_dir: work.to_string_lossy().into_owned(),
    })
    .unwrap();
    let s = ProjectSession::open(&paths.root, Arc::new(Rec)).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![card.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    s.start_analysis(false).unwrap();
    let t = Instant::now();
    while s.analysis_running() {
        assert!(t.elapsed() < Duration::from_secs(600), "análise não terminou");
        std::thread::sleep(Duration::from_millis(100));
    }
    let recs = s.list_recommendations().unwrap();
    let photos = s.list_photos().unwrap();
    let label_of = |name: &str| {
        let a = photos.iter().find(|p| p.file_name == name).unwrap().asset;
        recs.iter().find(|r| r.asset == a).map(|r| r.label.clone())
    };
    let r = s.export_lightroom_ratings(&LightroomOptions::default()).unwrap();
    eprintln!("{r:?}");
    assert!(r.failed.is_empty() && r.unavailable.is_empty(), "{r:?}");
    for raw in &raws {
        let name = raw.file_name().unwrap().to_string_lossy().into_owned();
        let side = raw.with_extension("xmp");
        let expected = label_of(&name).and_then(|l| stars_for_label(&l));
        let is_dng = name.to_lowercase().ends_with(".dng");
        match (expected, is_dng) {
            (Some(stars), false) => {
                let x = std::fs::read_to_string(&side).unwrap();
                assert!(x.contains(&format!("xmp:Rating=\"{stars}\"")), "{name}: {x}");
            }
            (Some(_), true) => {
                assert!(!side.exists() && r.not_raw.contains(&name), "DNG: só no relatório");
            }
            (None, _) => assert!(!side.exists(), "{name}: sem etiqueta para estrelas → sem .xmp"),
        }
    }
    // JPEG: nunca escritos (o Lightroom não lê .xmp ao lado de JPEG).
    assert!(std::fs::read_dir(&card).unwrap().flatten().all(|e| {
        let n = e.file_name().to_string_lossy().into_owned();
        !(n.starts_with("JPG_") && n.ends_with(".xmp"))
    }));
    // Estrelas também na aplicação (fotografias sem estrelas do utilizador).
    let after = s.list_photos().unwrap();
    for p in after.iter().filter(|p| p.virtual_copy.is_none()) {
        if let Some(stars) = label_of(&p.file_name).and_then(|l| stars_for_label(&l)) {
            assert_eq!(p.stars, stars, "{}", p.file_name);
        }
    }
    // Originais intactos.
    for (p, bytes) in &before {
        assert_eq!(&std::fs::read(p).unwrap(), bytes, "{p:?}");
    }
    s.close();
}
