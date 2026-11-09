//! Fase 6 — edição automática pelo caminho real da aplicação: uma fotografia, fila em
//! segundo plano (progresso, pausa, retoma, cancelamento), AI Denoise e originais intactos.
//! Com `MOMENTO_SAMPLES_DIR`, também edita automaticamente um RAW real.

use image::{Rgb, RgbImage};
use momento_database::import::ImportMode;
use momento_image_processing::develop::auto::{AiDenoise, AutoOptions, Intensity};
use momento_import_engine::ImportProgress;
use momento_project_management::auto_edit::{AiDenoiseProgress, AutoEditProgress};
use momento_project_management::paths::CacheLevel;
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Rec {
    edits: Mutex<Vec<i64>>,
    auto: Mutex<Vec<AutoEditProgress>>,
    ai: Mutex<Vec<AiDenoiseProgress>>,
}
impl SessionEvents for Rec {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &ThumbBatch) {}
    fn edits_changed(&self, keys: &[i64]) {
        self.edits.lock().unwrap().extend_from_slice(keys);
    }
    fn auto_edit(&self, p: &AutoEditProgress) {
        self.auto.lock().unwrap().push(p.clone());
    }
    fn ai_denoise(&self, p: &AiDenoiseProgress) {
        self.ai.lock().unwrap().push(p.clone());
    }
}

fn hash(p: &Path) -> u64 {
    let d = std::fs::read(p).unwrap();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&d, &mut h);
    std::hash::Hasher::finish(&h)
}

/// Fotografias diferentes: escura, clara, com dominante azul, com ruído forte.
fn variant(i: u32) -> RgbImage {
    let base = test_image(1500, 1000, i, 1);
    let mut s = 0x2545_f491_4f6c_dd1du64 ^ i as u64;
    RgbImage::from_fn(1500, 1000, |x, y| {
        let p = base.get_pixel(x, y);
        let f = |v: u8, k: f32| (v as f32 * k).clamp(0.0, 255.0);
        let c = match i % 4 {
            0 => [f(p[0], 0.35), f(p[1], 0.35), f(p[2], 0.35)],
            1 => [f(p[0], 1.4), f(p[1], 1.4), f(p[2], 1.4)],
            2 => [f(p[0], 0.75), f(p[1], 0.9), f(p[2], 1.25)],
            _ => {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                let n = ((s >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 60.0;
                [f(p[0], 1.0) + n, f(p[1], 1.0) + n, f(p[2], 1.0) + n]
            }
        };
        Rgb(c.map(|v| v.clamp(0.0, 255.0) as u8))
    })
}

fn raw_sample() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var("MOMENTO_SAMPLES_DIR").ok()?);
    std::fs::read_dir(d).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.extension().is_some_and(|e| {
            ["nef", "cr3", "arw"].contains(&e.to_string_lossy().to_lowercase().as_str())
        })
    })
}

fn wait<F: Fn() -> bool>(f: F, secs: u64) -> bool {
    let t = Instant::now();
    while !f() {
        if t.elapsed() > Duration::from_secs(secs) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    true
}

#[test]
fn automatic_editing_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    for i in 0..8 {
        write(
            &src.join(format!("IMG_{i}.jpg")),
            &jpeg_bytes(&variant(i), Some(&spec_for(i, 1)), 92),
        )
        .unwrap();
    }
    if let Some(r) = raw_sample() {
        std::fs::copy(&r, src.join(r.file_name().unwrap())).unwrap();
    }
    let originals: Vec<(PathBuf, u64)> =
        std::fs::read_dir(&src).unwrap().flatten().map(|e| (e.path(), hash(&e.path()))).collect();
    let paths = create_project(&NewProject {
        name: "Automático".into(),
        client_name: "Teste".into(),
        event_type: "wedding".into(),
        event_date: None,
        location: String::new(),
        originals_dir: None,
        work_dir: work.to_string_lossy().into_owned(),
    })
    .unwrap();
    let rec = Arc::new(Rec::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![src.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    let photos = s.list_photos().unwrap();
    let key = |name: &str| photos.iter().find(|p| p.file_name == name).unwrap().id.0;
    let jpgs: Vec<i64> = (0..8).map(|i| key(&format!("IMG_{i}.jpg"))).collect();

    // Pré-visualização (não grava) e edição de uma fotografia (entrada "auto" no histórico).
    let (preview, _) = s.auto_edit_preview(jpgs[0], &AutoOptions::default()).unwrap();
    assert!(preview.light.exposure > 0.5, "escura: {:?}", preview.light);
    assert!(s.edit_view(jpgs[0]).unwrap().settings.is_identity(), "pré-visualização não grava");
    let t = Instant::now();
    let r = s.auto_edit_photo(jpgs[0], &AutoOptions::default()).unwrap();
    eprintln!("auto (1 fotografia, JPEG 1,5 MP): {:.0?} — {:?}", t.elapsed(), r.report);
    assert_eq!(r.view.state.history.last().unwrap().label, "auto");
    assert!(r.view.settings.light.exposure > 0.5);
    let v = s.undo_edit(jpgs[0]).unwrap();
    assert!(v.settings.is_identity(), "desfazer devolve o original");

    // Fila: começa em pausa → nada é processado; retoma → tudo; cada fotografia é diferente.
    let opts = AutoOptions { intensity: Intensity::Balanced, ..Default::default() };
    assert_eq!(s.start_auto_edit(vec![jpgs[0], jpgs[1]], opts.clone()).unwrap(), 2);
    s.pause_auto_edit();
    // Acrescentar à fila em curso (mesmas opções).
    assert_eq!(s.start_auto_edit(jpgs[2..].to_vec(), opts.clone()).unwrap(), 8);
    assert!(s.start_auto_edit(vec![jpgs[0]], AutoOptions::default()).is_err(), "opções diferentes");
    let st = s.auto_edit_status();
    assert!(st.paused && st.total == 8);
    std::thread::sleep(Duration::from_millis(300));
    let done_paused = s.auto_edit_status().done;
    assert!(done_paused <= 2, "em pausa não avança (no máximo as que já tinham começado)");
    s.resume_auto_edit();
    s.wait_auto_edit();
    let last = rec.auto.lock().unwrap().last().cloned().unwrap();
    assert!(last.finished && last.done == 8 && last.failed == 0, "{last:?}");
    assert!(rec.auto.lock().unwrap().iter().any(|p| p.paused));
    let settings: Vec<_> = jpgs.iter().map(|&k| s.edit_view(k).unwrap().settings).collect();
    assert!(settings[0].light.exposure > settings[1].light.exposure + 0.5, "escura vs clara");
    assert!(settings[2].wb.temperature > 3.0, "dominante azul → aquecer: {:?}", settings[2].wb);
    assert!(
        settings[3].detail.noise_luminance > settings[1].detail.noise_luminance,
        "ruidosa: {:?} vs {:?}",
        settings[3].detail,
        settings[1].detail
    );
    let distinct: std::collections::HashSet<String> =
        settings.iter().map(|s| format!("{:?}", s.light)).collect();
    assert!(distinct.len() >= 6, "não são os mesmos parâmetros para todas");
    // Miniaturas editadas geradas e avisadas.
    assert!(jpgs.iter().all(|k| rec.edits.lock().unwrap().contains(k)));
    assert!(s
        .serve_image(jpgs[1], CacheLevel::Thumb)
        .unwrap()
        .path
        .to_string_lossy()
        .contains("edits"));

    // Cancelar: o que não foi processado não é tocado.
    s.reset_edit(jpgs[5]).unwrap();
    s.reset_edit(jpgs[6]).unwrap();
    s.start_auto_edit(vec![jpgs[5], jpgs[6]], AutoOptions::default()).unwrap();
    s.pause_auto_edit();
    s.cancel_auto_edit();
    s.wait_auto_edit();
    let last = rec.auto.lock().unwrap().last().cloned().unwrap();
    assert!(last.finished && last.cancelled, "{last:?}");
    let untouched = [jpgs[5], jpgs[6]]
        .iter()
        .filter(|&&k| s.edit_view(k).unwrap().settings.is_identity())
        .count();
    assert_eq!(untouched as u64, 2 - last.done);

    // AI Denoise (se o modelo existir): o render não espera; o cálculo avisa quando acaba.
    if s.ai_denoise_available() {
        let mut st = s.edit_view(jpgs[3]).unwrap().settings;
        st.detail.ai_denoise = 50.0;
        let t = Instant::now();
        let f = s.render_edit(jpgs[3], &st, 800, 800, RenderQuality::Final, false).unwrap();
        assert!(t.elapsed() < Duration::from_secs(3), "render não bloqueia: {:?}", t.elapsed());
        assert!(f.len() > 8);
        assert!(
            wait(|| rec.ai.lock().unwrap().iter().any(|p| p.key == jpgs[3] && p.finished), 120),
            "AI Denoise terminou"
        );
        eprintln!("AI Denoise no editor (fonte 1,5 MP): {:.1?}", t.elapsed());
        let denoised = s.render_edit(jpgs[3], &st, 800, 800, RenderQuality::Final, false).unwrap();
        assert_ne!(f, denoised, "depois de pronto, o render usa a fonte sem ruído");
        // Fila com AI Denoise quando necessário.
        let ai = AutoOptions { ai_denoise: AiDenoise::Always, ..Default::default() };
        s.start_auto_edit(vec![jpgs[3]], ai).unwrap();
        s.wait_auto_edit();
        assert!(s.edit_view(jpgs[3]).unwrap().settings.detail.ai_denoise > 0.0);
    } else {
        eprintln!("modelo do AI Denoise em falta: parte ignorada");
    }

    // RAW real.
    if let Some(r) = raw_sample() {
        let k = key(&r.file_name().unwrap().to_string_lossy());
        let t = Instant::now();
        let res = s.auto_edit_photo(k, &AutoOptions::default()).unwrap();
        eprintln!("auto RAW: {:.1?} — {:?}", t.elapsed(), res.report);
        assert!(!res.view.settings.is_identity());
    }

    for (p, h) in &originals {
        assert_eq!(&hash(p), h, "original alterado: {p:?}");
    }
    s.close();
}
