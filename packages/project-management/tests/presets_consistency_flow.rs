//! Fase 7 — presets e consistência pelo caminho real: medir, agrupar, aplicar presets
//! (internos e com LUT) ao casamento completo e sincronizar com uma referência.

use image::{Rgb, RgbImage};
use momento_database::import::ImportMode;
use momento_image_processing::develop::consistency::MatchOptions;
use momento_image_processing::develop::Settings;
use momento_import_engine::ImportProgress;
use momento_project_management::consistency::Criterion;
use momento_project_management::preset_store::PresetStore;
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

/// A mesma cena (pessoa, vestido branco, fundo) com outra luz: ganho e dominante.
fn shot(gain: f32, cast: [f32; 3], shift: u32) -> RgbImage {
    let base = RgbImage::from_fn(1200, 800, |x, y| {
        let (u, v) = ((x + shift) as f32 / 1200.0, y as f32 / 800.0);
        if (u - 0.5).powi(2) / 0.01 + (v - 0.35).powi(2) / 0.02 < 1.0 {
            Rgb([215, 160, 130])
        } else if (0.35..0.65).contains(&u) && v > 0.5 {
            Rgb([235, 233, 228])
        } else if v < 0.3 {
            Rgb([120, 150, 190])
        } else {
            let t = ((x / 40 + y / 40) % 2) as u8;
            Rgb([90 + t * 40, 80 + t * 30, 60 + t * 20])
        }
    });
    let lin = |v: u8| (v as f32 / 255.0).powf(2.2);
    RgbImage::from_fn(1200, 800, |x, y| {
        let p = base.get_pixel(x, y);
        Rgb(std::array::from_fn(|i| {
            ((lin(p[i]) * cast[i] * gain).min(1.0).powf(1.0 / 2.2) * 255.0) as u8
        }))
    })
}

fn frame_rgb(f: &[u8]) -> (u32, u32, Vec<[f32; 3]>) {
    let w = u32::from_le_bytes(f[0..4].try_into().unwrap());
    let h = u32::from_le_bytes(f[4..8].try_into().unwrap());
    let px = f[8 + 3 * 256 * 4..]
        .chunks_exact(4)
        .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
        .collect();
    (w, h, px)
}

/// Média do vestido (zona branca) e da imagem toda.
fn dress_and_mean(f: &[u8]) -> ([f32; 3], f32) {
    let (w, h, px) = frame_rgb(f);
    let mut acc = [0.0; 3];
    let mut n = 0.0;
    for y in (h * 65 / 100)..(h * 85 / 100) {
        for x in (w * 42 / 100)..(w * 58 / 100) {
            let p = px[(y * w + x) as usize];
            for i in 0..3 {
                acc[i] += p[i];
            }
            n += 1.0;
        }
    }
    let mean = px.iter().map(|p| p[1]).sum::<f32>() / px.len() as f32;
    (acc.map(|v| v / n), mean)
}

#[test]
fn presets_and_reference_sync() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    // 0–2: exterior, luz neutra; 3–5: interior quente e mais escuro (outra câmara).
    let lights = [
        (1.0, [1.0, 1.0, 1.0]),
        (0.9, [0.95, 1.0, 1.05]),
        (1.1, [1.0, 1.0, 1.0]),
        (0.35, [1.35, 1.0, 0.6]),
        (0.45, [1.3, 1.0, 0.65]),
        (0.4, [1.4, 1.0, 0.6]),
    ];
    for (i, (g, c)) in lights.iter().enumerate() {
        let mut spec = spec_for(i as u32, 1);
        if i >= 3 {
            spec.iso = Some(3200);
            spec.exposure_time = Some((1, 60));
            spec.f_number = Some((28, 10));
            spec.date_time_original = Some(format!("2026:06:15 21:{:02}:00", i));
            spec.make = Some("SONY".into());
            spec.model = Some("ILCE-7M4".into());
        } else {
            spec.iso = Some(100);
            spec.exposure_time = Some((1, 250));
            spec.f_number = Some((80, 10));
            spec.date_time_original = Some(format!("2026:06:15 15:{:02}:00", i));
            spec.make = Some("Canon".into());
            spec.model = Some("EOS R5".into());
        }
        write(
            &src.join(format!("IMG_{i}.jpg")),
            &jpeg_bytes(&shot(*g, *c, i as u32 * 7), Some(&spec), 92),
        )
        .unwrap();
    }
    let originals: Vec<(PathBuf, u64)> =
        std::fs::read_dir(&src).unwrap().flatten().map(|e| (e.path(), hash(&e.path()))).collect();
    let paths = create_project(&NewProject {
        name: "Consistência".into(),
        client_name: "Teste".into(),
        event_type: "wedding".into(),
        event_date: None,
        location: String::new(),
        originals_dir: None,
        work_dir: work.to_string_lossy().into_owned(),
    })
    .unwrap();
    let lut_dir = tmp.path().join("app-luts");
    set_lut_search_paths(vec![lut_dir.clone()]);
    let s = ProjectSession::open(&paths.root, Arc::new(Quiet)).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![src.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    let photos = s.list_photos().unwrap();
    let key =
        |i: usize| photos.iter().find(|p| p.file_name == format!("IMG_{i}.jpg")).unwrap().id.0;
    let keys: Vec<i64> = (0..6).map(key).collect();

    // Medir e agrupar.
    assert_eq!(s.measure_consistency().unwrap(), 6);
    assert_eq!(s.measure_consistency().unwrap(), 0, "só as pendentes");
    for c in [
        Criterion::Combined,
        Criterion::Lighting,
        Criterion::Camera,
        Criterion::Scene,
        Criterion::Time,
        Criterion::WhiteBalance,
        Criterion::Flash,
    ] {
        let (g, missing) = s.consistency_groups(c).unwrap();
        assert_eq!(missing, 0);
        assert_eq!(g.iter().map(|x| x.ids.len()).sum::<usize>(), 6, "{c:?}");
    }
    let (light, _) = s.consistency_groups(Criterion::Lighting).unwrap();
    assert_eq!(light.len(), 2, "{light:?}");
    let indoor = light.iter().find(|g| g.code == "lighting.dark").expect("interior escuro");
    assert_eq!(indoor.ids.len(), 3);
    let (wb, _) = s.consistency_groups(Criterion::WhiteBalance).unwrap();
    let warm_ids: Vec<i64> = wb
        .iter()
        .filter(|g| g.code.contains("arm"))
        .flat_map(|g| g.ids.iter().map(|a| a.0))
        .collect();
    assert!(
        keys[3..].iter().all(|k| warm_ids.contains(k)),
        "interior com dominante quente: {wb:?}"
    );
    let (comb, _) = s.consistency_groups(Criterion::Combined).unwrap();
    assert_eq!(comb.len(), 2);

    // Presets: interno ao casamento completo, com Amount.
    let lib = momento_database::library::Library::open_in_memory().unwrap();
    let store = PresetStore { lib: &lib, lut_dir: lut_dir.clone() };
    let film = store.get("builtin:film-warm").unwrap();
    assert_eq!(
        s.apply_preset(&keys, &film.name, &film.settings, &film.components, 80.0).unwrap(),
        6
    );
    let v = s.edit_view(keys[0]).unwrap();
    assert_eq!(v.state.history.last().unwrap().label, "preset:Film Warm");
    assert!((v.settings.light.contrast - film.settings.light.contrast * 0.8).abs() < 1e-3);
    assert!(
        (v.settings.grading.highlights_sat - film.settings.grading.highlights_sat * 0.8).abs()
            < 1e-3
    );
    // Preset com LUT: guardada na aplicação e copiada para o projeto.
    let cube = tmp.path().join("teste.cube");
    let mut text = String::from("LUT_3D_SIZE 2\n");
    for b in 0..2 {
        for g in 0..2 {
            for r in 0..2 {
                text +=
                    &format!("{} {} {}\n", r as f32 * 0.9, g as f32 * 0.95, b as f32 * 0.7 + 0.1);
            }
        }
    }
    std::fs::write(&cube, &text).unwrap();
    let lp = store.import(&cube).unwrap().presets.remove(0);
    let before = s
        .render_edit(
            keys[1],
            &s.edit_view(keys[1]).unwrap().settings,
            300,
            300,
            RenderQuality::Final,
            false,
        )
        .unwrap();
    s.apply_preset(&keys[1..2], &lp.name, &lp.settings, &lp.components, 100.0).unwrap();
    let lv = s.edit_view(keys[1]).unwrap().settings;
    assert_eq!(lv.lut.id, lp.settings.lut.id);
    assert!(paths.root.join("luts").join(format!("{}.cube", lv.lut.id)).is_file());
    let after = s.render_edit(keys[1], &lv, 300, 300, RenderQuality::Final, false).unwrap();
    assert_ne!(before, after);
    s.undo_edit(keys[1]).unwrap();

    // Sincronizar com a referência: a referência tem um aspeto; as do interior são outra luz.
    let mut r = s.edit_view(keys[0]).unwrap().settings;
    r.light.exposure = 0.2;
    r.wb.temperature = 5.0;
    r.hsl.saturation[3] = -40.0;
    s.commit_edit(keys[0], "manual", &r).unwrap();
    let render = |k: i64| {
        let st = s.edit_view(k).unwrap().settings;
        s.render_edit(k, &st, 400, 400, RenderQuality::Final, false).unwrap()
    };
    let (dr, mr) = dress_and_mean(&render(keys[0]));
    let (dn_before, mn_before) = dress_and_mean(&render(keys[4]));
    assert_eq!(
        s.start_sync(keys[0], keys.clone(), MatchOptions::default()).unwrap(),
        5,
        "a referência não entra"
    );
    s.wait_auto_edit();
    let chroma = |c: [f32; 3]| (c[0] - c[2]) / (c[0] + c[1] + c[2]);
    for &k in &keys[1..] {
        let st = s.edit_view(k).unwrap();
        assert_eq!(st.state.history.last().unwrap().label, "sync");
        assert_eq!(st.settings.hsl.saturation[3], -40.0, "aspeto copiado");
        let (d, m) = dress_and_mean(&render(k));
        assert!((m - mr).abs() < 12.0, "brilho {m} vs referência {mr}");
        assert!((chroma(d) - chroma(dr)).abs() < 0.04, "vestido {d:?} vs {dr:?}");
    }
    let s4 = s.edit_view(keys[4]).unwrap().settings;
    assert!(
        s4.light.exposure > r.light.exposure + 0.8 && s4.wb.temperature < r.wb.temperature - 15.0,
        "não é cópia: {:?} {:?}",
        s4.light,
        s4.wb
    );
    assert!(
        (mn_before - mr).abs() > 25.0 && (chroma(dn_before) - chroma(dr)).abs() > 0.06,
        "antes eram diferentes"
    );

    for (p, h) in &originals {
        assert_eq!(&hash(p), h, "original alterado: {p:?}");
    }
    let _ = Settings::default();
    s.close();
}
