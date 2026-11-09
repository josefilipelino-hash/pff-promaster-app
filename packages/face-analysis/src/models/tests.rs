//! Validação contra os resultados de referência publicados pela Google para `portrait.jpg`
//! (bucket `mediapipe-assets`). Ignorado se os modelos ou a imagem não existirem.

use super::*;
use crate::mesh::face_roi;

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
