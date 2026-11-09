//! Fase 8 — edição localizada pelo caminho real: máscaras de IA e de céu guardadas no
//! projeto, ajustes locais, retrato, retoque, remoção e originais intactos.
//! O retrato real vem de `MOMENTO_FACES_DIR` (portrait.jpg); sem ele, só a parte sintética.

use image::{Rgb, RgbImage};
use momento_database::import::ImportMode;
use momento_image_processing::develop::*;
use momento_import_engine::ImportProgress;
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

fn frame(f: &[u8]) -> (usize, usize, Vec<[f32; 3]>) {
    let w = u32::from_le_bytes(f[0..4].try_into().unwrap()) as usize;
    let h = u32::from_le_bytes(f[4..8].try_into().unwrap()) as usize;
    (
        w,
        h,
        f[8 + 3 * 256 * 4..]
            .chunks_exact(4)
            .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
            .collect(),
    )
}

fn region_mean(f: &[u8], x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
    let (w, h, px) = frame(f);
    let (mut s, mut n) = (0.0, 0.0);
    for y in (y0 * h as f32) as usize..(y1 * h as f32) as usize {
        for x in (x0 * w as f32) as usize..(x1 * w as f32) as usize {
            s += px[y * w + x][1];
            n += 1.0;
        }
    }
    s / n
}

fn mask(kind: &str, raster: &str, ev: f32) -> LocalMask {
    LocalMask {
        id: kind.into(),
        name: kind.into(),
        components: vec![MaskComponent {
            shape: MaskShape::Raster { kind: kind.into(), raster: raster.into(), feather: 0.0 },
            ..Default::default()
        }],
        adjust: LocalAdjust { exposure: ev, ..Default::default() },
        ..Default::default()
    }
}

#[test]
fn local_editing_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    // Céu azul com um edifício e relva.
    let sky = RgbImage::from_fn(900, 600, |x, y| {
        if y < 250 && !(380..520).contains(&x) {
            Rgb([110, 160, 225])
        } else if y < 250 {
            Rgb([90, 80, 70])
        } else {
            let t = ((x / 6 + y / 6) % 2) as u8 * 50;
            Rgb([60 + t, 90 + t, 50])
        }
    });
    write(&src.join("ceu.jpg"), &jpeg_bytes(&sky, None, 92)).unwrap();
    let portrait = std::env::var("MOMENTO_FACES_DIR")
        .ok()
        .map(|d| PathBuf::from(d).join("portrait.jpg"))
        .filter(|p| p.is_file());
    if let Some(p) = &portrait {
        std::fs::copy(p, src.join("retrato.jpg")).unwrap();
    }
    let originals: Vec<(PathBuf, u64)> =
        std::fs::read_dir(&src).unwrap().flatten().map(|e| (e.path(), hash(&e.path()))).collect();
    let paths = create_project(&NewProject {
        name: "Local".into(),
        client_name: "Teste".into(),
        event_type: "wedding".into(),
        event_date: None,
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
    let photos = s.list_photos().unwrap();
    let key = |n: &str| photos.iter().find(|p| p.file_name == n).unwrap().id.0;
    let render = |k: i64, st: &Settings| {
        s.render_edit(k, st, 600, 600, RenderQuality::Final, false).unwrap()
    };

    // Céu: máscara guardada no projeto e escurecida localmente.
    let k = key("ceu.jpg");
    let info = s.create_mask_raster(k, "sky", None).unwrap();
    assert!((0.3..0.42).contains(&info.coverage), "céu (teórico 0,35): {info:?}");
    assert!(paths.root.join("masks").join(format!("{}.png", info.raster)).is_file());
    let mut st = s.edit_view(k).unwrap().settings;
    let before = render(k, &st);
    st.masks.push(mask("sky", &info.raster, -1.0));
    s.commit_edit(k, "masks", &st).unwrap();
    let after = render(k, &s.edit_view(k).unwrap().settings);
    assert!(
        region_mean(&after, 0.05, 0.05, 0.3, 0.3)
            < region_mean(&before, 0.05, 0.05, 0.3, 0.3) - 15.0,
        "céu mais escuro"
    );
    assert!(
        (region_mean(&after, 0.1, 0.7, 0.4, 0.95) - region_mean(&before, 0.1, 0.7, 0.4, 0.95))
            .abs()
            < 1.0,
        "relva igual"
    );
    // Mapeamento vista ↔ fotografia com recorte e endireitar.
    let mut g = st.clone();
    g.geometry.crop = Crop { x: 0.2, y: 0.1, w: 0.6, h: 0.7 };
    g.geometry.straighten = 5.0;
    let srcp = s.map_points(k, &g, &[[0.5, 0.5], [0.2, 0.8]], true, false).unwrap();
    let back = s
        .map_points(k, &g, &srcp.iter().map(|p| p.unwrap()).collect::<Vec<_>>(), false, false)
        .unwrap();
    assert!((back[1].unwrap()[0] - 0.2).abs() < 1e-3 && (back[1].unwrap()[1] - 0.8).abs() < 1e-3);
    // Origem automática do heal: no mesmo tipo de zona (céu).
    let [sx, sy] = s.suggest_heal_source(k, 0.2, 0.2, 0.02).unwrap();
    assert!(sy < 0.4 && !(0.42..=0.58).contains(&sx), "origem no céu: {sx},{sy}");
    // Repor: tudo removido, igual ao original.
    s.reset_edit(k).unwrap();
    assert_eq!(render(k, &s.edit_view(k).unwrap().settings), before);

    if portrait.is_some() {
        let k = key("retrato.jpg");
        let base = s.edit_view(k).unwrap().settings;
        let b = render(k, &base);
        // Máscaras de IA: pessoa, fundo, rosto, cabelo, roupa, pele, sujeito e objeto.
        let mut cov = std::collections::HashMap::new();
        for kind in ["person", "background", "face", "hair", "clothes", "skin", "subject"] {
            let i = s.create_mask_raster(k, kind, None).unwrap();
            cov.insert(kind, i.clone());
        }
        let obj = s.create_mask_raster(k, "object", Some([0.5, 0.75])).unwrap();
        assert!(s.create_mask_raster(k, "sky", None).unwrap().coverage < 0.01, "interior sem céu");
        let c = |k: &str| cov[k].coverage;
        eprintln!("cobertura: {:?}", cov.iter().map(|(k, v)| (*k, v.coverage)).collect::<Vec<_>>());
        assert!(
            (0.3..0.7).contains(&c("person")) && (c("person") + c("background") - 1.0).abs() < 0.05
        );
        assert!(c("face") < c("skin") && c("hair") < 0.08 && c("clothes") > 0.15);
        assert!((c("subject") - c("person")).abs() < 0.02, "sujeito = pessoa");
        assert!(obj.coverage > 0.15);
        // Exposição só na pessoa: o fundo (bandeira à esquerda) não muda.
        let mut st = base.clone();
        st.masks.push(mask("person", &cov["person"].raster, 1.0));
        let r = render(k, &st);
        assert!(region_mean(&r, 0.4, 0.55, 0.6, 0.7) > region_mean(&b, 0.4, 0.55, 0.6, 0.7) + 5.0);
        assert!(
            (region_mean(&r, 0.0, 0.2, 0.06, 0.4) - region_mean(&b, 0.0, 0.2, 0.06, 0.4)).abs()
                < 2.0
        );
        // Retrato: uma face com regiões; só a cara muda.
        let faces = s.detect_portrait(k).unwrap();
        assert_eq!(faces.len(), 1);
        let f = &faces[0];
        let bb = f.oval.iter().fold([1f32, 1.0, 0.0, 0.0], |a, p| {
            [a[0].min(p[0]), a[1].min(p[1]), a[2].max(p[0]), a[3].max(p[1])]
        });
        assert!(bb[0] > 0.3 && bb[2] < 0.7 && bb[1] > 0.0 && bb[3] < 0.45, "rosto {bb:?}");
        let mut pt = base.clone();
        pt.portrait = Portrait {
            skin: 50.0,
            dark_circles: 40.0,
            shine: 40.0,
            teeth: 50.0,
            eyes: 30.0,
            faces: faces.clone(),
        };
        s.commit_edit(k, "portrait", &pt).unwrap();
        let r2 = render(k, &s.edit_view(k).unwrap().settings);
        assert_ne!(r2, b);
        assert_eq!(
            region_mean(&r2, 0.0, 0.6, 0.2, 1.0),
            region_mean(&b, 0.0, 0.6, 0.2, 1.0),
            "fora do rosto igual"
        );
        // Remover o retrato depois: volta exatamente ao original.
        let mut none = s.edit_view(k).unwrap().settings;
        none.portrait = Portrait::default();
        s.commit_edit(k, "portrait", &none).unwrap();
        assert_eq!(render(k, &s.edit_view(k).unwrap().settings), b);
    }
    for (p, h) in &originals {
        assert_eq!(&hash(p), h, "original alterado: {p:?}");
    }
    s.close();
}
