//! Validação contra os resultados de referência publicados pela Google para `portrait.jpg`
//! (bucket `mediapipe-assets`). Ignorado se os modelos ou a imagem não existirem.

use super::*;
use crate::mesh::face_roi;
use image::imageops;

fn setup() -> Option<(VisionModels, RgbImage)> {
    let dir = find_models_dir(&[])?;
    let img_path = std::env::var("MOMENTO_MP_PORTRAIT").ok()?;
    let img = image::open(img_path).ok()?.to_rgb8();
    Some((VisionModels::load(&dir).unwrap(), img))
}

#[test]
fn portrait_matches_mediapipe_reference() {
    let Some((m, img)) = setup() else {
        eprintln!("modelos ou MOMENTO_MP_PORTRAIT em falta: teste ignorado");
        return;
    };
    // Referência: portrait_expected_detection.pbtxt (bounding_box em píxeis, 820×1024).
    let dets = m.detect_faces(&img, false).unwrap();
    assert_eq!(dets.len(), 1, "{dets:?}");
    let d = dets[0];
    eprintln!("deteção: x={:.1} y={:.1} w={:.1} h={:.1} score={:.3}", d.x, d.y, d.w, d.h, d.score);
    assert!((d.x - 283.0).abs() < 8.0 && (d.y - 115.0).abs() < 8.0, "{d:?}");
    assert!((d.w - 234.0).abs() < 10.0 && (d.h - 234.0).abs() < 10.0, "{d:?}");
    let (w, h) = (img.width() as f32, img.height() as f32);
    let exp_kp = [(0.44416338, 0.17643969), (0.55514044, 0.17731678), (0.5046702, 0.2265771)];
    for (k, e) in exp_kp.iter().enumerate() {
        let (x, y) = (d.keypoints[k].0 / w, d.keypoints[k].1 / h);
        assert!((x - e.0).abs() < 0.01 && (y - e.1).abs() < 0.01, "ponto {k}: ({x}, {y}) vs {e:?}");
    }
    let _ = face_roi(&d);

    // Referência: portrait_expected_face_landmarks.pbtxt e portrait_expected_blendshapes.pbtxt.
    let faces = m.analyze_faces(&img, false).unwrap();
    assert_eq!(faces.len(), 1);
    let f = &faces[0];
    let exp_lm = [(0.49662378, 0.2506647), (0.4964889, 0.22542163)];
    for (k, e) in exp_lm.iter().enumerate() {
        let (x, y) = (f.landmarks[k].0 / w, f.landmarks[k].1 / h);
        eprintln!("ponto facial {k}: ({x:.4}, {y:.4}) esperado {e:?}");
        assert!((x - e.0).abs() < 0.01 && (y - e.1).abs() < 0.01, "ponto facial {k}");
    }
    let (bl, br) = f.eye_blink();
    eprintln!(
        "presença {:.3} · eyeBlink {:.3}/{:.3} (ref 0.263/0.297) · sorriso {:.3} (ref ~0.917)",
        f.presence,
        bl,
        br,
        f.smile()
    );
    assert!(f.presence > 0.9);
    assert!((bl - 0.2633514).abs() < 0.1 && (br - 0.29687604).abs() < 0.1, "olhos");
    assert!((f.smile() - 0.9167).abs() < 0.1, "sorriso");
}

#[test]
fn embeddings_separate_similar_from_different() {
    let Some((m, img)) = setup() else { return };
    let a = m.embed(&img).unwrap();
    // Mesma fotografia ligeiramente recortada e mais escura (como numa rajada).
    let crop = imageops::crop_imm(&img, 20, 30, img.width() - 40, img.height() - 60).to_image();
    let darker = RgbImage::from_fn(crop.width(), crop.height(), |x, y| {
        let p = crop.get_pixel(x, y).0;
        image::Rgb([
            (p[0] as f32 * 0.85) as u8,
            (p[1] as f32 * 0.85) as u8,
            (p[2] as f32 * 0.85) as u8,
        ])
    });
    let b = m.embed(&darker).unwrap();
    // Imagem diferente: gradiente sintético.
    let other =
        RgbImage::from_fn(400, 300, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
    let c = m.embed(&other).unwrap();
    let cos = |p: &[f32], q: &[f32]| p.iter().zip(q).map(|(x, y)| x * y).sum::<f32>();
    eprintln!("semelhantes {:.3} · diferentes {:.3}", cos(&a, &b), cos(&a, &c));
    assert_eq!(a.len(), 1024);
    assert!(cos(&a, &b) > 0.85);
    assert!(cos(&a, &b) - cos(&a, &c) > 0.3);
}

/// Referências geradas com os runtimes oficiais (onnxruntime 1.23 para o SFace, TFLite para
/// o EfficientNet-Lite2) sobre as mesmas entradas: `MOMENTO_REF_DIR` com `sface_in.png`,
/// `sface_ref.npy`, `scene_in.png`, `scene_ref.npy`.
fn ref_dir() -> Option<std::path::PathBuf> {
    let d = std::path::PathBuf::from(std::env::var("MOMENTO_REF_DIR").ok()?);
    d.join("sface_ref.npy").is_file().then_some(d)
}

/// Lê um .npy float32 simples (cabeçalho v1).
fn read_npy(p: &std::path::Path) -> Vec<f32> {
    let b = std::fs::read(p).unwrap();
    let hlen = u16::from_le_bytes([b[8], b[9]]) as usize;
    b[10 + hlen..].chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

#[test]
fn identity_and_scene_match_official_runtimes() {
    let (Some(dir), Some(models)) = (ref_dir(), find_models_dir(&[])) else {
        eprintln!("MOMENTO_REF_DIR ou modelos em falta: teste ignorado");
        return;
    };
    let m = VisionModels::load(&models).unwrap();
    assert!(m.has_identity() && m.has_scene());
    // SFace: o recorte já tem 112×112; compara o vetor normalizado.
    let crop = image::open(dir.join("sface_in.png")).unwrap().to_rgb8();
    let v = crate::identity::embed(m.identity.as_ref().unwrap(), &crop).unwrap();
    let r = read_npy(&dir.join("sface_ref.npy"));
    let rn = r.iter().map(|x| x * x).sum::<f32>().sqrt();
    let cos: f32 = v.iter().zip(&r).map(|(a, b)| a * b / rn).sum();
    eprintln!("SFace tract vs onnxruntime: cos = {cos:.6}");
    assert!(cos > 0.9999, "{cos}");
    // Classificador: mesmas probabilidades que o TFLite.
    let img = image::open(dir.join("scene_in.png")).unwrap().to_rgb8();
    let p = m.classify_scene(&img).unwrap().unwrap();
    let r = read_npy(&dir.join("scene_ref.npy"));
    let max_diff = p.iter().zip(&r).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
    let labels = crate::scene::labels();
    let top: Vec<_> =
        crate::scene::top_k(&p, 3).into_iter().map(|(i, s)| (labels[i as usize], s)).collect();
    eprintln!("cena: {top:?} · diferença máxima {max_diff:.2e}");
    assert!(max_diff < 1e-3);
    assert_eq!(top[0].0, "suit");
}

#[test]
fn identity_separates_people_and_survives_changes() {
    let Some((m, img)) = setup() else { return };
    if !m.has_identity() {
        return;
    }
    let id = |im: &RgbImage| {
        let f = m.analyze_faces(im, true).unwrap();
        f.into_iter()
            .max_by(|a, b| a.detection.w.total_cmp(&b.detection.w))
            .and_then(|b| b.identity)
    };
    let cos = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
    let base = id(&img).unwrap();
    // Mesma pessoa: espelhada, mais escura, mais pequena e rodada 8°.
    let flipped = image::imageops::flip_horizontal(&img);
    let darker = RgbImage::from_fn(img.width(), img.height(), |x, y| {
        let p = img.get_pixel(x, y).0;
        image::Rgb([
            (p[0] as f32 * 0.6) as u8,
            (p[1] as f32 * 0.6) as u8,
            (p[2] as f32 * 0.6) as u8,
        ])
    });
    let small = image::imageops::resize(&img, 410, 512, image::imageops::FilterType::Triangle);
    let same = [
        cos(&base, &id(&flipped).unwrap()),
        cos(&base, &id(&darker).unwrap()),
        cos(&base, &id(&small).unwrap()),
    ];
    eprintln!("mesma pessoa: {same:?}");
    // Pessoas diferentes: outras fotografias da pasta MOMENTO_FACES_DIR, se existir.
    let mut diff = Vec::new();
    if let Ok(dir) = std::env::var("MOMENTO_FACES_DIR") {
        for name in [
            "lena.jpg",
            "messi5.jpg",
            "Tom_Cruise_avp_2014_4.jpg",
            "2008_001009.jpg",
            "2009_004587.jpg",
            "dogs.jpg",
        ] {
            if let Ok(o) = image::open(std::path::Path::new(&dir).join(name)) {
                if let Some(v) = id(&o.to_rgb8()) {
                    diff.push(cos(&base, &v));
                }
            }
        }
    }
    eprintln!("pessoas diferentes: {diff:?}");
    // Limiar recomendado para o SFace (OpenCV Zoo): cos ≥ 0,363 → mesma pessoa.
    assert!(same.iter().all(|&s| s > 0.55), "{same:?}");
    assert!(diff.iter().all(|&s| s < 0.3), "{diff:?}");
}
