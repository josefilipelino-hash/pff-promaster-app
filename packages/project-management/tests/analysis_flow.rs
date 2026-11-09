//! Fase 3 — fluxo completo da seleção assistida pelo caminho real da aplicação:
//! importar → analisar (métricas + modelos locais) → grupos/recomendações → mudar modo →
//! aceitar sugestões → fechar → reabrir.
//!
//! As rajadas são sintéticas (textura determinística desfocada progressivamente, com
//! carimbos de hora de uma rajada real). Com `MOMENTO_FACES_DIR`, uma fotografia real com
//! rostos dessa pasta é usada como base da rajada (nada é copiado para o repositório).

use image::{imageops, Rgb, RgbImage};
use momento_core_types::AssetId;
use momento_database::import::ImportMode;
use momento_import_engine::ImportProgress;
use momento_project_management::analysis::AnalysisProgress;
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Default)]
struct Recorder {
    progress: Mutex<Vec<AnalysisProgress>>,
}

impl SessionEvents for Recorder {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &ThumbBatch) {}
    fn analysis(&self, p: &AnalysisProgress) {
        self.progress.lock().unwrap().push(p.clone());
    }
}

/// Textura determinística com detalhe fino (para a nitidez ser mensurável).
fn texture(w: u32, h: u32, seed: u32) -> RgbImage {
    let mut s = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    };
    let base = test_image(w, h, seed, 1);
    let mut img = RgbImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let b = base.get_pixel(x, y).0;
        let check = if ((x / 6) + (y / 6)) % 2 == 0 { 30i32 } else { -30 };
        let n = (next() % 41) as i32 - 20;
        let v = |c: u8| (c as i32 + check + n).clamp(0, 255) as u8;
        *p = Rgb([v(b[0]), v(b[1]), v(b[2])]);
    }
    img
}

fn spec(camera: u32, secs: u32, centis: u32) -> ExifSpec {
    let mut s = spec_for(camera, 1);
    let t = 14 * 3600 + secs;
    s.date_time_original =
        Some(format!("2026:06:15 {:02}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60));
    s.sub_sec = Some(format!("{centis:02}"));
    s
}

fn face_base() -> Option<RgbImage> {
    let dir = PathBuf::from(std::env::var("MOMENTO_FACES_DIR").ok()?);
    let p = dir.join("portrait.jpg");
    let p = if p.is_file() {
        p
    } else {
        std::fs::read_dir(&dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "jpg"))?
    };
    Some(image::open(p).ok()?.to_rgb8())
}

/// Rajada de 10 (0,15 s entre disparos): a 4.ª é a mais nítida; as outras com desfocagem
/// crescente. Mais 3 momentos isolados e um duplicado exato.
fn generate(dir: &Path) -> (String, Vec<String>) {
    let base = face_base().unwrap_or_else(|| texture(1600, 1067, 7));
    let blur = [2.4f32, 1.6, 0.9, 0.0, 0.6, 1.2, 2.0, 3.0, 4.0, 5.0];
    let mut names = Vec::new();
    for (i, &sigma) in blur.iter().enumerate() {
        let img = if sigma > 0.0 { imageops::blur(&base, sigma) } else { base.clone() };
        let name = format!("DSC_{:04}.jpg", 100 + i);
        let t = i as u32 * 15;
        write(
            &dir.join("rajada").join(&name),
            &jpeg_bytes(&img, Some(&spec(0, t / 100, t % 100)), 92),
        )
        .unwrap();
        names.push(name);
    }
    for k in 0..3u32 {
        let img = texture(1600, 1067, 100 + k);
        write(
            &dir.join(format!("soltas/DSC_{:04}.jpg", 300 + k)),
            &jpeg_bytes(&img, Some(&spec(0, 600 + k * 120, 0)), 90),
        )
        .unwrap();
    }
    // Isolada e muito desfocada → rejeição sugerida.
    let img = imageops::blur(&texture(1600, 1067, 200), 6.0);
    write(&dir.join("soltas/DSC_0400.jpg"), &jpeg_bytes(&img, Some(&spec(0, 1200, 0)), 90))
        .unwrap();
    ("DSC_0103.jpg".into(), names)
}

#[test]
fn analyze_group_recommend_accept_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let (sharpest, burst) = generate(&source);

    let paths = create_project(&NewProject {
        name: "Casamento".into(),
        client_name: "Ana & Pedro".into(),
        event_type: "wedding".into(),
        event_date: Some("2026-06-15".into()),
        location: "Sintra".into(),
        originals_dir: None,
        work_dir: work.to_string_lossy().into_owned(),
    })
    .unwrap();
    let rec = Arc::new(Recorder::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![source.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: true,
    })
    .unwrap();
    s.wait_import();
    let photos = s.list_photos().unwrap();
    assert_eq!(photos.len(), 14);
    let id_of = |n: &str| photos.iter().find(|p| p.file_name == n).unwrap().id;

    // Sem análise iniciada pelo utilizador, nada corre sozinho.
    assert!(!s.analysis_running());
    let st = s.analysis_status().unwrap();
    assert_eq!(st.counts.pending, 14);
    assert!(st.models_available, "modelos em models/ (scripts/prepare-models.py)");

    let t = Instant::now();
    assert_eq!(s.start_analysis(false).unwrap(), 14);
    assert!(s.start_analysis(false).is_err(), "só uma análise de cada vez");
    s.wait_analysis();
    let elapsed = t.elapsed();
    eprintln!(
        "análise de 14 fotografias: {elapsed:.2?} ({:.0} ms/foto)",
        elapsed.as_millis() as f64 / 14.0
    );
    let progress = rec.progress.lock().unwrap().clone();
    assert!(progress.last().unwrap().finished);
    assert_eq!(progress.last().unwrap().done, 14);

    let st = s.analysis_status().unwrap();
    assert_eq!((st.counts.analyzed, st.counts.failed, st.counts.pending), (14, 0, 0));

    // A rajada forma um grupo de sequência; a mais nítida é MELHOR.
    let groups = s.list_groups().unwrap();
    let g = groups.iter().find(|g| g.members.contains(&id_of(&sharpest))).expect("rajada agrupada");
    assert_eq!(g.kind, "sequence");
    assert_eq!(g.members.len(), burst.len(), "{groups:?}");
    for m in &g.members {
        let d = s.analysis_details(*m).unwrap();
        let (a, r) = (d.analysis.unwrap(), d.recommendation.unwrap());
        let name = &photos.iter().find(|p| p.id == *m).unwrap().file_name;
        eprintln!("{name}: {} peak {:.2} sharp {:.2} motion {:.2} noise {:.1} | focus {:.2} mot {:.2} noise {:.2} overall {:.3} {}", r.label, a.focus_peak, a.sharpness, a.motion_blur, a.noise, r.focus, r.motion, r.noise, r.overall, r.reasons);
    }
    assert_eq!(g.members[0], id_of(&sharpest), "a mais nítida é a primeira");
    let d = s.analysis_details(id_of(&sharpest)).unwrap();
    let r = d.recommendation.clone().unwrap();
    assert_eq!(r.label, "best");
    assert_eq!(r.summary, "bestInSequence");
    assert!(r.reasons.contains("sharp"), "{}", r.reasons);
    assert!(d.analysis.as_ref().unwrap().width <= 2048);
    let worst = s.analysis_details(id_of("DSC_0109.jpg")).unwrap().recommendation.unwrap();
    assert_eq!(worst.label, "reject", "{worst:?}");
    if face_base().is_some() {
        assert!(!d.faces.is_empty(), "rosto detetado na fotografia real");
        assert!(r.eyes.is_some());
        eprintln!(
            "rostos: {:?}",
            d.faces
                .iter()
                .map(|f| (f.px, f.blink_left, f.blink_right, f.smile, f.sharpness))
                .collect::<Vec<_>>()
        );
    }
    // Isolada desfocada.
    let blurred = s.analysis_details(id_of("DSC_0400.jpg")).unwrap().recommendation.unwrap();
    assert_eq!(blurred.label, "reject");
    assert!(blurred.reasons.contains("outOfFocus"), "{}", blurred.reasons);

    let balanced = s.analysis_status().unwrap().summary;
    eprintln!("equilibrada: {balanced:?}");

    // Modo rigoroso: no máximo 1 MELHOR por grupo, menos alternativas.
    let strict = s
        .set_selection_settings(momento_ai_analysis::selection::Settings {
            mode: momento_ai_analysis::selection::Mode::Strict,
            target: None,
        })
        .unwrap();
    eprintln!("rigorosa: {strict:?}");
    assert!(strict.best + strict.alternative <= balanced.best + balanced.alternative);
    let recs = s.list_recommendations().unwrap();
    let best_in_burst = recs
        .iter()
        .filter(|r| r.group_id == Some(g.id) || g.members.contains(&r.asset))
        .filter(|r| r.label == "best")
        .count();
    assert_eq!(best_in_burst, 1);

    // Nenhuma recomendação mexe nas decisões do utilizador nem apaga ficheiros.
    assert!(s.list_photos().unwrap().iter().all(|p| p.flag == 0));
    assert!(source.join("rajada").join("DSC_0109.jpg").is_file());

    // Aceitar o grupo: favorita na melhor, rejeitadas nas sugeridas; o resto fica.
    let groups = s.list_groups().unwrap();
    let g = groups.iter().find(|g| g.members.contains(&id_of(&sharpest))).unwrap();
    s.set_flag(&[id_of("DSC_0108.jpg")], 1).unwrap(); // decisão manual: mantém-se
    let changed = s.accept_group(g.id).unwrap();
    assert!(changed >= 2);
    let after = s.list_photos().unwrap();
    let flag = |n: &str| after.iter().find(|p| p.file_name == n).unwrap().flag;
    assert_eq!(flag(&sharpest), 1);
    assert_eq!(flag("DSC_0109.jpg"), -1);
    assert_eq!(flag("DSC_0108.jpg"), 1, "decisão do utilizador mantida");
    // Reversível como qualquer classificação.
    s.set_flag(&[id_of("DSC_0109.jpg")], 0).unwrap();

    // Objetivo de entrega: ≈ 3 fotografias.
    let target = s
        .set_selection_settings(momento_ai_analysis::selection::Settings {
            mode: momento_ai_analysis::selection::Mode::Balanced,
            target: Some(3),
        })
        .unwrap();
    eprintln!("objetivo 3: {target:?}");
    let picks = after.iter().filter(|p| p.flag == 1).count() as u64;
    assert!(target.best + target.alternative + picks <= 3 + picks, "{target:?}");

    s.close();
    drop(s);
    let s2 = ProjectSession::open(&paths.root, Arc::new(Recorder::default())).unwrap();
    let st = s2.analysis_status().unwrap();
    assert_eq!(st.counts.analyzed, 14);
    assert_eq!(st.settings.target, Some(3), "definições guardadas no catálogo");
    assert!(!s2.analysis_running(), "nada pendente: não reinicia");
    assert_eq!(s2.list_recommendations().unwrap().len(), 14);
    let _: Vec<AssetId> = s2.list_groups().unwrap().into_iter().flat_map(|g| g.members).collect();
    s2.close();
}

/// Débito da análise com fotografias de câmara de 24 MP (JPEG 6000×4000).
/// `MOMENTO_PERF_N=300 cargo test --release --test analysis_flow -- --ignored --nocapture`
#[test]
#[ignore]
fn analysis_throughput_24mp() {
    let n: u32 = std::env::var("MOMENTO_PERF_N").ok().and_then(|v| v.parse().ok()).unwrap_or(120);
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let base = face_base()
        .map(|b| imageops::resize(&b, 6000, 4000, imageops::FilterType::Triangle))
        .unwrap_or_else(|| texture(6000, 4000, 3));
    let jpg = jpeg_bytes(&base, Some(&spec(0, 0, 0)), 90);
    eprintln!("JPEG de 24 MP: {:.1} MB", jpg.len() as f64 / 1e6);
    for i in 0..n {
        // Mesmo conteúdo, ficheiros distintos (cabeçalho EXIF diferente → hash diferente).
        let b = jpeg_bytes(&base, Some(&spec(0, i * 30, i % 100)), 90 - (i % 3) as u8);
        write(&source.join(format!("IMG_{i:05}.jpg")), &b).unwrap();
    }
    let paths = create_project(&NewProject {
        name: "Desempenho".into(),
        client_name: "Teste".into(),
        event_type: "wedding".into(),
        event_date: None,
        location: String::new(),
        originals_dir: None,
        work_dir: work.to_string_lossy().into_owned(),
    })
    .unwrap();
    let s = ProjectSession::open(&paths.root, Arc::new(Recorder::default())).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![source.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    let t = Instant::now();
    s.start_analysis(false).unwrap();
    s.wait_analysis();
    let e = t.elapsed();
    let per = e.as_secs_f64() / n as f64;
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    eprintln!(
        "{n} fotografias de 24 MP em {e:.1?} → {:.0} ms/foto ({cores} núcleos, {} threads de análise); 15 000 fotografias ≈ {:.0} min",
        per * 1000.0,
        (cores / 2).max(1),
        per * 15_000.0 / 60.0
    );
    assert_eq!(s.analysis_status().unwrap().counts.analyzed, n as u64);
    s.close();
}
