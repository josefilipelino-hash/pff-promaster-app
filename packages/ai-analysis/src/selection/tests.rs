use super::*;

/// Embedding sintético: base por “cena” + pequena variação (cos ≈ 0,95–0,99 na mesma cena).
fn emb(scene: u32, jitter: u32) -> Vec<f32> {
    let mut v: Vec<f32> = (0..64)
        .map(|i| {
            (((i as u32).wrapping_mul(2654435761u32) ^ scene.wrapping_mul(97)) % 1000) as f32
                / 1000.0
                - 0.5
        })
        .collect();
    for (i, x) in v.iter_mut().enumerate() {
        *x += (((i as u32 + jitter * 31) % 7) as f32 - 3.0) * 0.012 * jitter.min(3) as f32;
    }
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.into_iter().map(|x| x / n).collect()
}

fn face(x: f32, sharp: f32, closure: f32, smile: f32) -> FaceInfo {
    FaceInfo {
        x,
        y: 0.3,
        w: 0.15,
        h: 0.2,
        px: 180.0,
        blink_left: closure,
        blink_right: closure,
        smile,
        jaw_open: 0.05,
        sharpness: sharp,
        eye_sharpness: sharp,
        presence: 0.99,
        cut: false,
    }
}

fn photo(id: i64, ms: i64, scene: u32, jitter: u32, faces: Vec<FaceInfo>) -> PhotoInput {
    PhotoInput {
        id,
        capture_ms: Some(ms),
        camera: Some("Canon EOS R5".into()),
        stem_key: format!("/c/img_{id}"),
        file_seq: Some(id as u64),
        is_raw: false,
        metrics: TechnicalMetrics {
            sharpness: 2.2,
            focus_peak: 2.6,
            focus_center: 2.4,
            motion_blur: 0.05,
            brightness: 0.45,
            highlights_clipped: 0.002,
            shadows_crushed: 0.01,
            contrast: 0.6,
            noise: 2.5,
            tilt_degrees: None,
        },
        faces,
        phash: 0xF0F0_F0F0_0F0F_0F0F ^ scene as u64,
        embedding: Some(emb(scene, jitter)),
        user_flag: 0,
    }
}

/// O beijo: 14 fotografias em 2,6 s. A #7 é a melhor (rostos nítidos, olhos abertos,
/// sorriso); a #3 tem uma piscadela; a #10 está tremida/desfocada.
fn kiss() -> Vec<PhotoInput> {
    (0..14)
        .map(|i| {
            let id = 100 + i as i64;
            let sharp = if i == 7 {
                2.5
            } else if i == 10 {
                1.1
            } else {
                2.0 + (i % 3) as f32 * 0.08
            };
            let closure = if i == 3 { 0.85 } else { 0.18 };
            let smile = if i == 7 { 0.85 } else { 0.5 };
            let mut p = photo(
                id,
                1_000_000 + i as i64 * 200,
                1,
                (i % 3) as u32,
                vec![face(0.3, sharp, closure, smile), face(0.55, sharp, 0.15, smile)],
            );
            if i == 10 {
                p.metrics.motion_blur = 0.7;
            }
            p
        })
        .collect()
}

fn rec(recs: &[Recommendation], id: i64) -> &Recommendation {
    recs.iter().find(|r| r.id == id).unwrap()
}

#[test]
fn kiss_sequence_is_grouped_and_best_is_explained() {
    let mut photos = kiss();
    // Fotografias de outro momento, 2 minutos depois.
    photos.push(photo(500, 1_120_000, 9, 0, vec![face(0.4, 2.3, 0.1, 0.6)]));
    photos.push(photo(501, 1_300_000, 12, 0, vec![]));
    let (groups, recs) = select(&photos, Settings::default());
    let seq: Vec<&Group> = groups.iter().filter(|g| g.kind == GroupKind::Sequence).collect();
    assert_eq!(seq.len(), 1, "{groups:?}");
    assert_eq!(seq[0].members.len(), 14);
    assert_eq!(seq[0].members[0], 107, "a #7 é a melhor");
    let best = rec(&recs, 107);
    assert_eq!(best.label, Label::Best);
    assert_eq!(best.summary, "bestInSequence");
    let codes: Vec<&str> = best.reasons.iter().map(|r| r.code.as_str()).collect();
    assert!(codes.contains(&"sharpestFace"), "{codes:?}");
    // 1–3 melhores no modo equilibrado.
    let n_best = recs.iter().filter(|r| r.group.is_some() && r.label == Label::Best).count();
    assert!((1..=3).contains(&n_best), "{n_best}");
    // Piscadela e tremida: nunca melhores nem alternativas.
    assert_eq!(rec(&recs, 103).label, Label::Reject, "{:?}", rec(&recs, 103));
    assert!(rec(&recs, 103).reasons.iter().any(|r| r.code == "eyesClosed"));
    assert_eq!(rec(&recs, 110).label, Label::Reject);
    assert!(rec(&recs, 110)
        .reasons
        .iter()
        .any(|r| r.code == "motionBlur" || r.code == "outOfFocus"));
    // Isoladas: avaliadas pela qualidade absoluta.
    assert_eq!(rec(&recs, 500).group, None);
    assert_eq!(rec(&recs, 501).group, None);
}

#[test]
fn moments_far_apart_are_not_grouped_but_two_cameras_at_once_are() {
    let mut a = photo(1, 0, 3, 0, vec![face(0.4, 2.2, 0.1, 0.5)]);
    let mut b = photo(2, 900, 3, 2, vec![face(0.42, 2.2, 0.1, 0.5)]);
    b.camera = Some("Sony ILCE-7M4".into());
    let mut c = photo(3, 60_000, 3, 1, vec![face(0.4, 2.2, 0.1, 0.5)]);
    c.phash ^= 0x0000_FFFF_0000_FFFF; // outra fotografia (não é uma cópia)
    a.file_seq = Some(1);
    let (groups, _) = select(&[a, b, c], Settings::default());
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].members.len(), 2, "segundo fotógrafo no mesmo instante");
}

#[test]
fn raw_jpeg_pairs_and_near_duplicates() {
    let mut raw = photo(1, 0, 5, 0, vec![]);
    raw.is_raw = true;
    raw.stem_key = "/c/img_0001".into();
    let mut jpg = photo(2, 0, 5, 0, vec![]);
    jpg.stem_key = "/c/img_0001".into();
    // Cópia exportada noutra altura: mesmo pHash, embedding praticamente igual.
    let a = photo(10, 500_000, 7, 0, vec![]);
    let mut b = photo(11, 900_000, 7, 0, vec![]);
    b.metrics.sharpness = 2.1;
    let (groups, recs) = select(&[raw, jpg, a, b], Settings::default());
    assert!(groups.iter().any(|g| g.kind == GroupKind::RawJpeg && g.members == vec![1, 2]));
    assert_eq!(rec(&recs, 2).label, Label::Duplicate);
    assert_eq!(rec(&recs, 2).summary, "rawJpegPair");
    assert!(groups.iter().any(|g| g.kind == GroupKind::Duplicate && g.members.len() == 2));
    assert_eq!(
        recs.iter().filter(|r| (r.id == 10 || r.id == 11) && r.label == Label::Duplicate).count(),
        1
    );
}

#[test]
fn modes_change_how_many_are_kept() {
    let photos = kiss();
    let kept = |mode| {
        select(&photos, Settings { mode, target: None })
            .1
            .iter()
            .filter(|r| matches!(r.label, Label::Best | Label::Alternative))
            .count()
    };
    let (c, b, s) = (kept(Mode::Conservative), kept(Mode::Balanced), kept(Mode::Strict));
    assert!(c >= b && b >= s, "conservadora {c} ≥ equilibrada {b} ≥ rigorosa {s}");
    assert!(c > s);
    assert!(s >= 1);
}

#[test]
fn target_count_is_respected_and_favours_diversity() {
    // 50 momentos × 4 fotografias.
    let mut photos = Vec::new();
    for g in 0..50 {
        for k in 0..4 {
            let id = (g * 10 + k) as i64;
            photos.push(photo(
                id,
                g as i64 * 120_000 + k as i64 * 300,
                g as u32 + 1,
                k as u32,
                vec![face(0.4, 2.0 + k as f32 * 0.1, 0.1, 0.5)],
            ));
        }
    }
    photos[0].user_flag = 1; // favorita do utilizador conta para o objetivo
    let (_, recs) = select(&photos, Settings { mode: Mode::Balanced, target: Some(60) });
    let kept = recs.iter().filter(|r| matches!(r.label, Label::Best | Label::Alternative)).count();
    assert!((58..=60).contains(&kept), "{kept}");
    // Diversidade: cada um dos 50 momentos tem pelo menos uma fotografia mantida.
    let mut groups_kept = std::collections::HashSet::new();
    for r in recs.iter().filter(|r| matches!(r.label, Label::Best | Label::Alternative)) {
        groups_kept.insert(r.group);
    }
    assert!(groups_kept.len() >= 49, "{}", groups_kept.len());
    let (_, few) = select(&photos, Settings { mode: Mode::Balanced, target: Some(20) });
    let kept = few.iter().filter(|r| matches!(r.label, Label::Best | Label::Alternative)).count();
    assert!(kept <= 20, "{kept}");
    assert!(few.iter().any(|r| r.summary == "outsideTarget"));
}

#[test]
fn relative_blink_within_a_person_track() {
    // A mesma pessoa: fecho ~0,2 em 5 fotografias e 0,55 numa (abaixo do limiar absoluto).
    let photos: Vec<PhotoInput> = (0..6)
        .map(|i| {
            photo(
                i,
                i * 250,
                4,
                (i % 2) as u32,
                vec![face(0.4, 2.3, if i == 4 { 0.55 } else { 0.2 }, 0.3)],
            )
        })
        .collect();
    let (_, recs) = select(&photos, Settings::default());
    let r4 = rec(&recs, 4);
    assert!(r4.reasons.iter().any(|r| r.code == "blink"), "{r4:?}");
    assert_ne!(r4.label, Label::Best);
}

#[test]
fn laughing_squint_is_not_closed_eyes() {
    let laughing = face(0.4, 2.3, 0.62, 0.95);
    assert!(laughing.closure() < 0.6, "{}", laughing.closure());
    let closed = face(0.4, 2.3, 0.9, 0.2);
    assert!(closed.closure() >= 0.6);
    let tiny = FaceInfo { px: 30.0, ..closed };
    assert!(!tiny.eyes_reliable(), "rosto pequeno: olhos indeterminados");
}

#[test]
fn performance_with_15000_photos() {
    // 15 000 fotografias: ~3000 momentos de 1–9 fotografias, várias isoladas.
    let mut photos = Vec::with_capacity(15_000);
    let mut t = 0i64;
    let mut g = 0u32;
    while photos.len() < 15_000 {
        let size = 1 + (g % 9) as usize;
        for k in 0..size {
            let id = photos.len() as i64;
            let faces = if g % 3 == 0 {
                vec![]
            } else {
                vec![
                    face(0.3, 1.8 + (k % 4) as f32 * 0.2, if k == 2 { 0.8 } else { 0.15 }, 0.5),
                    face(0.55, 2.0, 0.1, 0.4),
                ]
            };
            photos.push(photo(id, t, g + 1, (k % 3) as u32, faces));
            t += 250;
        }
        t += 45_000;
        g += 1;
    }
    let start = std::time::Instant::now();
    let (groups, recs) = select(&photos, Settings { mode: Mode::Balanced, target: Some(800) });
    let el = start.elapsed();
    eprintln!("15 000 fotografias: {} grupos, seleção em {el:.2?}", groups.len());
    assert_eq!(recs.len(), 15_000);
    assert!(groups.len() > 2_000);
    let kept = recs.iter().filter(|r| matches!(r.label, Label::Best | Label::Alternative)).count();
    assert!(kept <= 800);
    assert!(el.as_secs_f32() < 20.0, "{el:?}");
}
