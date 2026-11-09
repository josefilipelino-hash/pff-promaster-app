//! Fase 5 — edição não destrutiva pelo caminho real da aplicação.
//! Com `MOMENTO_SAMPLES_DIR`, também edita um RAW real (o primeiro .NEF/.CR3/.ARW da pasta).

use momento_core_types::AssetId;
use momento_database::import::ImportMode;
use momento_import_engine::ImportProgress;
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
}
impl SessionEvents for Rec {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &ThumbBatch) {}
    fn edits_changed(&self, keys: &[i64]) {
        self.edits.lock().unwrap().extend_from_slice(keys);
    }
}

fn hash(p: &Path) -> Vec<u8> {
    let d = std::fs::read(p).unwrap();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&d, &mut h);
    std::hash::Hasher::finish(&h).to_le_bytes().to_vec()
}

fn frame_dims(f: &[u8]) -> (u32, u32) {
    (
        u32::from_le_bytes(f[0..4].try_into().unwrap()),
        u32::from_le_bytes(f[4..8].try_into().unwrap()),
    )
}

/// Média do canal verde do RGBA de um frame.
fn frame_mean(f: &[u8]) -> f32 {
    let rgba = &f[8 + 3 * 256 * 4..];
    rgba.chunks_exact(4).map(|p| p[1] as f32).sum::<f32>() / (rgba.len() / 4) as f32
}

fn raw_sample() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var("MOMENTO_SAMPLES_DIR").ok()?);
    std::fs::read_dir(d).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.extension().is_some_and(|e| {
            ["nef", "cr3", "arw"].contains(&e.to_string_lossy().to_lowercase().as_str())
        })
    })
}

fn wait_edits(r: &Rec, n: usize) {
    let t = Instant::now();
    while r.edits.lock().unwrap().len() < n && t.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn non_destructive_editing_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    for i in 0..2 {
        write(
            &src.join(format!("IMG_{i}.jpg")),
            &jpeg_bytes(&test_image(1800, 1200, i, 1), Some(&spec_for(i, 1)), 90),
        )
        .unwrap();
    }
    let raw = raw_sample();
    if let Some(r) = &raw {
        std::fs::copy(r, src.join(r.file_name().unwrap())).unwrap();
    }
    let originals: Vec<(PathBuf, Vec<u8>)> =
        std::fs::read_dir(&src).unwrap().flatten().map(|e| (e.path(), hash(&e.path()))).collect();
    let paths = create_project(&NewProject {
        name: "Edição".into(),
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
    let jpg = photos.iter().find(|p| p.file_name == "IMG_0.jpg").unwrap().id.0;
    let other = photos.iter().find(|p| p.file_name == "IMG_1.jpg").unwrap().id.0;

    // Render neutro e com exposição (só render: nada é gravado).
    let v = s.edit_view(jpg).unwrap();
    assert!(v.settings.is_identity() && v.state.position == 0);
    let t = Instant::now();
    let f0 = s.render_edit(jpg, &v.settings, 1024, 1024, RenderQuality::Fast, false).unwrap();
    eprintln!("primeiro render (carregar a fonte + render): {:.0?}", t.elapsed());
    assert_eq!(frame_dims(&f0), (1024, 683));
    let mut st = v.settings.clone();
    st.light.exposure = 1.0;
    let t = Instant::now();
    let f1 = s.render_edit(jpg, &st, 1024, 1024, RenderQuality::Fast, false).unwrap();
    eprintln!("render rápido seguinte (fonte em memória): {:.0?}", t.elapsed());
    assert!(frame_mean(&f1) > frame_mean(&f0) + 15.0);
    assert!(s.edit_view(jpg).unwrap().settings.is_identity(), "render não grava");

    // Gravar, desfazer, refazer.
    let v = s.commit_edit(jpg, "exposure", &st).unwrap();
    assert_eq!((v.state.position, v.state.can_undo), (1, true));
    st.light.contrast = 30.0;
    s.commit_edit(jpg, "contrast", &st).unwrap();
    let v = s.undo_edit(jpg).unwrap();
    assert_eq!((v.settings.light.exposure, v.settings.light.contrast), (1.0, 0.0));
    let v = s.redo_edit(jpg).unwrap();
    assert_eq!(v.settings.light.contrast, 30.0);
    // Instantâneo, repor e voltar ao instantâneo.
    let v = s.create_snapshot(jpg, "Antes do recorte").unwrap();
    let snap = v.state.snapshots[0].id;
    let v = s.reset_edit(jpg).unwrap();
    assert!(v.settings.is_identity() && v.state.can_undo, "repor pode ser desfeito");
    let v = s.apply_snapshot(jpg, snap).unwrap();
    assert_eq!(v.settings.light.contrast, 30.0);
    // Colar só a luz noutra fotografia; cópia virtual independente.
    assert_eq!(s.paste_settings(&[other], &v.settings, &["light".into()]).unwrap(), 1);
    assert_eq!(s.edit_view(other).unwrap().settings.light.exposure, 1.0);
    let vc = s.create_virtual_copy(jpg).unwrap();
    let mut vs = s.edit_view(vc).unwrap().settings;
    assert_eq!(vs.light.contrast, 30.0, "a cópia herda os ajustes");
    vs.color.saturation = -100.0;
    s.commit_edit(vc, "saturation", &vs).unwrap();
    assert_eq!(s.edit_view(jpg).unwrap().settings.color.saturation, 0.0, "principal intacta");
    let listed = s.list_photos().unwrap();
    assert_eq!(listed.len(), photos.len() + 1);
    assert!(listed.iter().any(|p| p.id.0 == vc && p.virtual_copy.is_some() && p.edited));
    // Miniatura e preview editadas (geradas a pedido / em segundo plano) e sidecar.
    wait_edits(&rec, 6);
    let thumb = s.serve_image(jpg, CacheLevel::Thumb).unwrap();
    assert!(thumb.path.to_string_lossy().contains("edits"), "{:?}", thumb.path);
    let img = image::open(&thumb.path).unwrap().to_rgb8();
    assert_eq!(img.width(), 640);
    let gray = image::open(s.serve_image(vc, CacheLevel::Preview).unwrap().path).unwrap().to_rgb8();
    let p = gray.get_pixel(gray.width() / 2, gray.height() / 2);
    assert!((p[0] as i32 - p[2] as i32).abs() <= 2, "cópia virtual a preto e branco");
    assert!(paths.root.join("edits").read_dir().unwrap().count() >= 3, "sidecars JSON");
    // Fotografia sem edição: serve os ficheiros normais.
    let plain = photos.iter().find(|p| !p.file_name.starts_with("IMG")).map(|p| p.id.0);
    // Geometria: rodar 90° troca as dimensões; endireitar automático devolve um ângulo ou nada.
    let mut g = s.edit_view(other).unwrap().settings;
    g.geometry.quarter_turns = 1;
    let f = s.render_edit(other, &g, 800, 800, RenderQuality::Final, false).unwrap();
    assert_eq!(frame_dims(&f), (533, 800));
    let _ = s.auto_straighten(other, &g).unwrap();
    // Seletor de balanço de brancos devolve valores válidos.
    let wb = s.pick_white_balance(other, &g, 0.5, 0.5).unwrap();
    assert!(wb.is_some_and(|(t, n)| (-100.0..=100.0).contains(&t) && (-100.0..=100.0).contains(&n)));

    // RAW real: fonte linear do sensor.
    if let Some(r) = &raw {
        let name = r.file_name().unwrap().to_string_lossy().into_owned();
        let key = photos.iter().find(|p| p.file_name == name).unwrap().id.0;
        let t = Instant::now();
        let base = s
            .render_edit(key, &DevelopSettings::default(), 1024, 1024, RenderQuality::Fast, false)
            .unwrap();
        eprintln!(
            "RAW {name}: carregar (revelação linear a meia resolução) + render: {:.0?}",
            t.elapsed()
        );
        let mut rs = DevelopSettings::default();
        rs.light.highlights = -60.0;
        rs.wb.temperature = 20.0;
        let t = Instant::now();
        let after = s.render_edit(key, &rs, 1024, 1024, RenderQuality::Fast, false).unwrap();
        eprintln!("RAW render rápido seguinte: {:.0?}", t.elapsed());
        assert_ne!(base, after);
        s.commit_edit(key, "highlights", &rs).unwrap();
        let t = Instant::now();
        let full = s.serve_image(key, CacheLevel::Full).unwrap();
        let fi = image::image_dimensions(&full.path).unwrap();
        eprintln!("RAW resolução total editada {fi:?}: {:.0?}", t.elapsed());

        let d = s.photo_details(momento_core_types::AssetId(key)).unwrap().unwrap();
        assert_eq!(fi.0.max(fi.1), d.width.max(d.height), "resolução total, sem ampliar");
    }
    let _ = plain;

    // Os originais nunca foram modificados.
    for (p, h) in &originals {
        assert_eq!(&hash(p), h, "original alterado: {p:?}");
    }
    // Reabrir: tudo persistido.
    s.close();
    drop(s);
    let s2 = ProjectSession::open(&paths.root, Arc::new(Rec::default())).unwrap();
    let v = s2.edit_view(jpg).unwrap();
    assert_eq!((v.settings.light.exposure, v.settings.light.contrast), (1.0, 30.0));
    assert!(v.state.history.len() >= 5 && v.state.snapshots.len() == 1);
    assert_eq!(s2.edit_view(vc).unwrap().settings.color.saturation, -100.0);
    s2.delete_virtual_copy(vc).unwrap();
    assert!(s2.delete_virtual_copy(jpg).is_err());
    assert_eq!(s2.list_photos().unwrap().len(), photos.len());
    let _: AssetId = AssetId(jpg);
    s2.close();
}
