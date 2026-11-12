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

    // Zoom: tamanho nativo e render só da zona visível, a partir da fonte em resolução total.
    assert_eq!(s.native_size(jpg, &v.settings).unwrap(), (1800, 1200));
    let mut z = v.settings.clone();
    z.geometry.crop = momento_image_processing::develop::Crop { x: 0.4, y: 0.4, w: 0.25, h: 0.25 };
    let fz =
        s.render_edit_overlay(jpg, &z, 450, 300, RenderQuality::Final, false, None, true).unwrap();
    assert_eq!(frame_dims(&fz), (450, 300), "zona de 450×300 px do original a 100 %");
    // Mesma zona pela fonte de edição: imagens equivalentes (média igual a ±2 níveis).
    let fl =
        s.render_edit_overlay(jpg, &z, 450, 300, RenderQuality::Final, false, None, false).unwrap();
    assert!((frame_mean(&fz) - frame_mean(&fl)).abs() < 2.0);

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
    // Formato 4:5 nas duas fotografias (cada uma com o seu recorte): proporção exata em
    // píxeis, altura toda; "original" volta à fotografia inteira.
    assert_eq!(s.apply_aspect(&[jpg, other], "4:5").unwrap(), 2);
    for k in [jpg, other] {
        let g = s.edit_view(k).unwrap().settings.geometry;
        assert_eq!(g.aspect, "4:5");
        assert!(
            ((g.crop.w * 1800.0) / (g.crop.h * 1200.0) - 0.8).abs() < 1e-3
                && (g.crop.h - 1.0).abs() < 1e-5,
            "{:?}",
            g.crop
        );
    }
    assert!(s.apply_aspect(&[other], "nada").is_err());
    s.apply_aspect(&[jpg, other], "original").unwrap();
    let g = s.edit_view(other).unwrap().settings.geometry;
    assert!((g.crop.w - 1.0).abs() < 1e-5 && (g.crop.h - 1.0).abs() < 1e-5);
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

/// Média RGB de um frame.
fn frame_rgb(f: &[u8]) -> [f32; 3] {
    let rgba = &f[8 + 3 * 256 * 4..];
    let n = (rgba.len() / 4) as f32;
    let mut s = [0f32; 3];
    for p in rgba.chunks_exact(4) {
        for c in 0..3 {
            s[c] += p[c] as f32;
        }
    }
    s.map(|v| v / n)
}

/// Auditoria final: um JPEG com perfil Adobe RGB embutido é revelado com as cores do perfil
/// (antes era tratado como sRGB e ficava dessaturado na edição e na exportação).
#[test]
fn embedded_icc_profile_is_honoured() {
    use momento_color_management::{ColorSpace, InputColor, OutputColor};
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    let px = [120u8, 160, 80];
    let img = image::RgbImage::from_pixel(600, 400, image::Rgb(px));
    let adobe = OutputColor::builtin(ColorSpace::AdobeRgb, false).unwrap();
    std::fs::write(
        src.join("adobe.jpg"),
        momento_image_processing::encode_jpeg(&img, 95, Some(adobe.icc())).unwrap(),
    )
    .unwrap();
    std::fs::write(
        src.join("srgb.jpg"),
        momento_image_processing::encode_jpeg(&img, 95, None).unwrap(),
    )
    .unwrap();
    let paths = create_project(&NewProject {
        name: "Perfis".into(),
        client_name: "Teste".into(),
        event_type: "wedding".into(),
        event_date: None,
        location: String::new(),
        originals_dir: None,
        work_dir: work.to_string_lossy().into_owned(),
    })
    .unwrap();
    let s = ProjectSession::open(&paths.root, Arc::new(Rec::default())).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![src.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    let photos = s.list_photos().unwrap();
    let id = |n: &str| photos.iter().find(|p| p.file_name == n).unwrap().id.0;
    let neutral = s.edit_view(id("srgb.jpg")).unwrap().settings;
    let render =
        |k| frame_rgb(&s.render_edit(k, &neutral, 600, 600, RenderQuality::Final, false).unwrap());
    let plain = render(id("srgb.jpg"));
    let tagged = render(id("adobe.jpg"));
    // Esperado: Adobe RGB → Rec. 2020 linear → sRGB.
    let mut e = vec![px.map(|v| v as f32 / 255.0)];
    InputColor::new(adobe.icc()).unwrap().unwrap().to_linear_rec2020(&mut e);
    OutputColor::builtin(ColorSpace::Srgb, false).unwrap().convert(&mut e);
    let expected = e[0].map(|v| v * 255.0);
    eprintln!("sem perfil {plain:?} · Adobe RGB {tagged:?} · esperado {expected:?}");
    for c in 0..3 {
        assert!((plain[c] - px[c] as f32).abs() < 3.0, "sRGB: {plain:?}");
        assert!((tagged[c] - expected[c]).abs() < 3.0, "Adobe RGB: {tagged:?} ≠ {expected:?}");
    }
    assert!(tagged[1] - tagged[0] > plain[1] - plain[0] + 5.0, "o verde Adobe RGB é mais saturado");
}

/// Temperatura do balanço da câmara (Kelvin) dos RAW reais: plausível (luz de dia/flash/
/// tungsténio) e a mesma no render — com `MOMENTO_SAMPLES_DIR`.
#[test]
fn as_shot_kelvin_of_real_raws() {
    let Some(dir) = std::env::var("MOMENTO_SAMPLES_DIR").ok().map(PathBuf::from) else {
        eprintln!("SALTADO: MOMENTO_SAMPLES_DIR não definido");
        return;
    };
    let mut seen = 0;
    for p in std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()) {
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !["nef", "cr2", "cr3", "arw", "raf", "orf", "rw2", "pef", "srw", "dng"]
            .contains(&ext.as_str())
        {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        let Ok(raw) = momento_raw_engine::RawFile::open(&data) else { continue };
        let k = raw
            .info()
            .as_shot_white
            .and_then(momento_image_processing::develop::color::kelvin_of_white);
        eprintln!("{:<40} {:?}", p.file_name().unwrap().to_string_lossy(), k.map(|k| k.round()));
        if let Some(k) = k {
            assert!((1800.0..=15000.0).contains(&k), "{p:?}: {k} K");
            seen += 1;
        }
    }
    assert!(seen >= 5, "poucos RAW com temperatura: {seen}");
}

/// Câmara sem perfil de cor na LibRaw (ex.: Canon EOS C50): simulada a partir de RAW reais
/// com as cores do sensor misturadas; a calibração pela preview embutida recupera as cores.
/// Com `MOMENTO_SAMPLES_DIR`.
#[test]
fn unknown_camera_colors_are_calibrated_from_the_embedded_preview() {
    use momento_image_processing::develop::calibrate::calibrate_from_preview;
    use momento_image_processing::develop::color::mul3;
    let Some(dir) = std::env::var("MOMENTO_SAMPLES_DIR").ok().map(PathBuf::from) else {
        eprintln!("SALTADO: MOMENTO_SAMPLES_DIR não definido");
        return;
    };
    let registry = momento_project_management::codecs::full_registry();
    let mut tested = 0;
    for name in ["Canon EOS R3.cr3", "Nikon D810.nef", "Sony ILCE-7M4 (A7M4).ARW"] {
        let path = dir.join(name);
        let Ok(data) = std::fs::read(&path) else { continue };
        let mut raw = momento_raw_engine::RawFile::open(&data).unwrap();
        let info = raw.info().clone();
        assert!(info.color_matrix_known, "{name}: câmara conhecida");
        let bm = raw.develop_linear(true).unwrap();
        let truth = momento_image_processing::develop::Source::from_linear16(
            bm.width,
            bm.height,
            &bm.rgb,
            info.orientation,
            info.width,
            info.height,
        );
        let t = truth.levels.last().unwrap();
        // "Câmara desconhecida" como a LibRaw a entrega sem perfil: cores do sensor
        // misturadas E sem balanço de brancos (verde ~2× mais forte, azul fraco).
        let mix = [[0.40, 0.15, 0.03], [0.30, 1.45, 0.20], [0.03, 0.18, 0.45]];
        let mut fake = t.clone();
        // … e sem o nível de preto subtraído (valor constante em todos os píxeis, como sem o
        // bloco de dados da câmara).
        let black = [0.012, 0.025, 0.010];
        for p in &mut fake.px {
            let m = mul3(&mix, *p);
            *p = [m[0] + black[0], m[1] + black[1], m[2] + black[2]];
        }
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase());
        let format = momento_image_codecs::detect(&data[..data.len().min(4096)], ext.as_deref())
            .unwrap()
            .format;
        let preview = momento_image_processing::apply_orientation(
            registry.decode(&data, format, Some(480)).unwrap().image,
            info.orientation,
        );
        let cal = calibrate_from_preview(&fake, &preview).expect("calibra");
        // Preview de câmara de cinema: recortada a 16:9 e com barras pretas em cima e em baixo.
        {
            let (pw, ph) = preview.dimensions();
            let ch = (pw as f32 * 9.0 / 16.0) as u32;
            let crop = image::imageops::crop_imm(&preview, 0, (ph - ch) / 2, pw, ch).to_image();
            let bar = ch / 8;
            let mut boxed = image::RgbImage::new(pw, ch + 2 * bar);
            image::imageops::replace(&mut boxed, &crop, 0, bar as i64);
            let c2 = calibrate_from_preview(&fake, &boxed).expect("calibra com barras e 16:9");
            eprintln!(
                "{name}: 16:9 com barras → enquadramento {:?}, alinhamento {:.3}, erro {:.4}",
                c2.framing, c2.alignment, c2.error_after
            );
            assert!(c2.framing.starts_with("recortado") && c2.alignment > 0.9, "{name}: {c2:?}");
        }
        eprintln!(
            "{name}: alinhamento {:.3} ({}), preto estimado {:?} (real {black:?})",
            cal.alignment, cal.framing, cal.offset
        );
        // Erro de cromaticidade médio face às cores verdadeiras, antes e depois.
        let chroma = |v: [f32; 3]| {
            let s = (v[0] + v[1] + v[2]).max(1e-6);
            v.map(|c| c / s)
        };
        // Erro mínimo possível: a própria preview face às cores verdadeiras.
        let pv = image::imageops::resize(
            &preview,
            t.w as u32,
            t.h as u32,
            image::imageops::FilterType::Triangle,
        );
        let (mut before, mut after, mut floor, mut n) = (0f32, 0f32, 0f32, 0f32);
        for (i, (f, tr)) in fake.px.iter().zip(&t.px).enumerate().step_by(37) {
            if tr.iter().sum::<f32>() < 0.02 {
                continue;
            }
            let q = pv.get_pixel((i % t.w) as u32, (i / t.w) as u32).0;
            let ql = mul3(
                &momento_image_processing::develop::color::SRGB_TO_REC2020,
                q.map(|c| momento_image_processing::develop::color::eotf(c as f32 / 255.0)),
            );
            let fo = [f[0] - cal.offset[0], f[1] - cal.offset[1], f[2] - cal.offset[2]]
                .map(|v| v.max(0.0));
            let (a, b, c, d) = (chroma(*f), chroma(mul3(&cal.matrix, fo)), chroma(*tr), chroma(ql));
            before += (0..3).map(|i| (a[i] - c[i]).abs()).sum::<f32>();
            after += (0..3).map(|i| (b[i] - c[i]).abs()).sum::<f32>();
            floor += (0..3).map(|i| (d[i] - c[i]).abs()).sum::<f32>();
            n += 1.0;
        }
        eprintln!("{name}: preview face às cores verdadeiras (mínimo) {:.4}", floor / n);
        eprintln!(
            "{name}: erro de cor sem calibração {:.4} → calibrado {:.4} ({} amostras)",
            before / n,
            after / n,
            cal.samples
        );
        // A preview tem o "estilo" da câmara (um pouco diferente das cores neutras da
        // LibRaw): há um erro mínimo que nenhuma calibração elimina.
        assert!(
            after < before * 0.8 && after < floor * 1.2,
            "{name}: a calibração tem de aproximar as cores verdadeiras"
        );
        tested += 1;
    }
    assert!(tested >= 2);
}

/// Upright em RAW reais (`MOMENTO_SAMPLES_DIR`): nunca piora as linhas, e é consistente — se a
/// fotografia for rodada X° (e inclinada em perspetiva), o Upright compensa essa diferença.
#[test]
fn upright_on_real_raws() {
    use momento_image_processing::develop::{self, upright, Geometry, Quality, Settings};
    let Some(dir) = std::env::var("MOMENTO_SAMPLES_DIR").ok().map(PathBuf::from) else {
        eprintln!("SALTADO: MOMENTO_SAMPLES_DIR não definido");
        return;
    };
    let mut checked = 0;
    for p in std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()) {
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !["nef", "cr2", "cr3", "arw", "raf", "orf", "rw2", "pef", "dng"].contains(&ext.as_str())
        {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        let Ok(mut raw) = momento_raw_engine::RawFile::open(&data) else { continue };
        let info = raw.info().clone();
        let Ok(bm) = raw.develop_linear(true) else { continue };
        let src = develop::Source::from_linear16(
            bm.width,
            bm.height,
            &bm.rgb,
            info.orientation,
            info.width,
            info.height,
        );
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let auto = develop::upright(&src, &Settings::default(), upright::UprightMode::Auto, None);
        eprintln!("{name:<40} auto: {auto:?}");
        if let Some(r) = &auto {
            assert!(r.rms_after <= r.rms_before + 1e-3, "{name}: piorou {r:?}");
            assert!(r.vertical.abs() <= 100.0 && r.horizontal.abs() <= 100.0);
        }
        // Consistência: rodar a fotografia 3° (render com endireitar) e pedir Nível à imagem
        // resultante → a diferença para a original tem de ser ~3°.
        let level = |s: &Settings| {
            let img = develop::render_rgb(&src, s, 1280, Quality::Fast);
            let (w, h) = (img.width() as usize, img.height() as usize);
            let gray: Vec<f32> = img
                .pixels()
                .map(|q| {
                    (0.2126 * q[0] as f32 + 0.7152 * q[1] as f32 + 0.0722 * q[2] as f32) / 255.0
                })
                .collect();
            let segs = upright::detect_segments(&gray, w, h);
            upright::solve_upright(&segs, w, h, None, upright::UprightMode::Level)
                .map(|r| (r.straighten, r.lines))
        };
        let (Some(a), Some(b)) = (
            level(&Settings::default()),
            level(&Settings {
                geometry: Geometry { straighten: 3.0, ..Default::default() },
                ..Default::default()
            }),
        ) else {
            eprintln!("{name:<40} sem linhas suficientes para o teste de consistência");
            continue;
        };
        eprintln!("{name:<40} nível {:.2}° ({:?}) → rodada 3°: {:.2}° ({:?})", a.0, a.1, b.0, b.1);
        // Só conta onde há linhas a sério (fotografias de natureza não têm).
        if a.1 .0 + a.1 .1 >= 8 && b.1 .0 + b.1 .1 >= 8 {
            assert!(((a.0 - b.0) - 3.0).abs() < 0.6, "{name}: {a:?} vs {b:?}");
            checked += 1;
        }
    }
    eprintln!("consistência verificada em {checked} fotografias");
    // As amostras públicas têm poucas fotografias com arquitetura (2: Olympus E-M10, Nikon D70).
    assert!(checked >= 2, "poucas fotografias com linhas: {checked}");
}

/// Perfis de objetiva (Lensfun) nos RAW reais (`MOMENTO_SAMPLES_DIR`): lista o que é
/// identificado; onde há perfil com distorção, corrigir não pode criar cantos vazios.
#[test]
fn lens_profiles_on_real_raws() {
    use momento_image_processing::develop::{geometry::GeoMap, Geometry, Lens};
    use momento_project_management::lensprofile::{profile_for, ProfileQuery};
    let Some(dir) = std::env::var("MOMENTO_SAMPLES_DIR").ok().map(PathBuf::from) else {
        eprintln!("SALTADO: MOMENTO_SAMPLES_DIR não definido");
        return;
    };
    let mut found = 0;
    for p in std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()) {
        let Ok(data) = std::fs::read(&p) else { continue };
        let Ok(raw) = momento_raw_engine::RawFile::open(&data) else { continue };
        let i = raw.info().clone();
        let q = ProfileQuery {
            camera_make: i.make.clone(),
            camera_model: i.model.clone(),
            lens: i.lens.clone().unwrap_or_default(),
            focal: i.focal_length.map(|f| f as f32),
            focal35: i.focal_length_35mm.map(|f| f as f32),
            aperture: i.aperture.map(|a| a as f32),
        };
        let prof = profile_for(&q);
        eprintln!(
            "{:<34} {:<22} {:<38} {:>5.1}mm f{:<4.1} → {}",
            p.file_name().unwrap().to_string_lossy(),
            format!("{} {}", i.make, i.model).chars().take(22).collect::<String>(),
            q.lens.chars().take(38).collect::<String>(),
            q.focal.unwrap_or(0.0),
            q.aperture.unwrap_or(0.0),
            prof.as_ref().map_or("—".into(), |p| format!(
                "{} (dist {} {:?}, tca {}, vinheta {:?})",
                p.name, p.dist_model, p.dist, p.tca_model, p.vig
            ))
        );
        if let Some(prof) = prof {
            found += 1;
            let lens = Lens { profile: prof, ..Default::default() };
            let m = GeoMap::new(
                i.width as usize,
                i.height as usize,
                &Geometry::default(),
                &lens,
                false,
            );
            for k in 0..=20 {
                let t = k as f32 / 20.0;
                for (u, v) in [(t, 0.0), (t, 1.0), (0.0, t), (1.0, t)] {
                    let q = m.map(u, v).0[1];
                    assert!(
                        (-2e-3..=1.002).contains(&q.0) && (-2e-3..=1.002).contains(&q.1),
                        "{p:?}: canto vazio em ({u},{v}) → {q:?}"
                    );
                }
            }
        }
    }
    eprintln!("{found} RAW com perfil");
}
