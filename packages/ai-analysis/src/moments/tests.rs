use super::*;

fn idx(name: &str) -> u16 {
    LABELS.lines().position(|l| l == name).unwrap_or_else(|| panic!("classe {name}")) as u16
}

fn at(h: u32, m: u32) -> i64 {
    // 15/06/2026 em ms “locais” (só as diferenças contam).
    let day = 20_619i64 * 86_400_000;
    day + (h as i64 * 60 + m as i64) * 60_000
}

fn emb(scene: u32, k: u32) -> Vec<f32> {
    let v: Vec<f32> = (0..64u32)
        .map(|i| {
            ((i.wrapping_mul(2_654_435_761) ^ scene.wrapping_mul(97)) % 1000) as f32 / 1000.0 - 0.5
                + ((i + k) % 5) as f32 * 0.01
        })
        .collect();
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.into_iter().map(|x| x / n).collect()
}

fn face(x: f32, y: f32, w: f32) -> FaceBrief {
    FaceBrief { x, y, w, h: w * 1.3, px: w * 2048.0 }
}

struct Day {
    photos: Vec<MomentPhoto>,
    expect: Vec<(i64, Option<&'static str>)>,
}

#[allow(clippy::too_many_arguments)] // construtor de dados de teste: um argumento por pista
impl Day {
    fn add(
        &mut self,
        t: i64,
        scene: &[(&str, f32)],
        faces: Vec<FaceBrief>,
        roles: &[&str],
        bright: f32,
        emb_scene: u32,
        expect: Option<&'static str>,
    ) -> i64 {
        let id = self.photos.len() as i64 + 1;
        self.photos.push(MomentPhoto {
            id,
            time_ms: Some(t),
            gps: None,
            scene: scene.iter().map(|(n, p)| (idx(n), *p)).collect(),
            embedding: Some(emb(emb_scene, id as u32)),
            faces,
            roles: roles.iter().map(|s| s.to_string()).collect(),
            brightness: bright,
            iso: Some(if bright < 0.3 { 6400 } else { 400 }),
        });
        self.expect.push((id, expect));
        id
    }
}

/// Um dia de casamento sintético (pistas como as devolvidas pelos modelos).
fn wedding() -> Day {
    let mut d = Day { photos: Vec::new(), expect: Vec::new() };
    let one = || vec![face(0.4, 0.2, 0.12)];
    // Preparação.
    for k in 0..4 {
        d.add(
            at(10, k * 5),
            &[("four-poster", 0.2), ("lipstick", 0.1)],
            one(),
            &["bride"],
            0.55,
            1,
            Some("bride_prep"),
        );
        d.add(
            at(10, 40 + k * 3),
            &[("bow tie", 0.3), ("suit", 0.3)],
            one(),
            &["groom"],
            0.5,
            2,
            Some("groom_prep"),
        );
    }
    d.add(at(11, 0), &[("Loafer", 0.6)], vec![], &[], 0.6, 3, Some("shoes"));
    d.add(at(11, 2), &[("gown", 0.7)], vec![], &[], 0.6, 3, Some("dress"));
    d.add(at(11, 4), &[("envelope", 0.5), ("menu", 0.2)], vec![], &[], 0.6, 3, Some("invitations"));
    d.add(at(11, 6), &[("perfume", 0.6)], vec![], &[], 0.6, 3, Some("details"));
    // Chegada e cerimónia (13:00–13:45).
    d.add(at(12, 52), &[("limousine", 0.6)], one(), &[], 0.6, 4, Some("arrival"));
    for k in 0..12u32 {
        let t = at(13, k * 4);
        // Entrada: os primeiros ~15 % da cerimónia.
        let exp = if k <= 1 { Some("entrance") } else { Some("ceremony") };
        d.add(
            t,
            &[("church", 0.4), ("altar", 0.3)],
            vec![face(0.3, 0.3, 0.05), face(0.6, 0.3, 0.05), face(0.1, 0.5, 0.03)],
            &[],
            0.45,
            5,
            exp,
        );
    }
    // O beijo: dois rostos grandes a tocar-se.
    d.add(
        at(13, 45),
        &[("church", 0.3), ("altar", 0.2)],
        vec![face(0.40, 0.25, 0.14), face(0.52, 0.27, 0.13)],
        &["bride", "groom"],
        0.45,
        5,
        Some("kiss"),
    );
    // Saída: muitos rostos logo depois.
    let crowd = (0..6).map(|k| face(0.1 + k as f32 * 0.13, 0.35, 0.05)).collect::<Vec<_>>();
    d.add(at(13, 58), &[("bell cote", 0.05)], crowd.clone(), &[], 0.65, 6, Some("exit"));
    // Sessão do casal e grupos.
    for k in 0..3 {
        d.add(
            at(14, 30 + k * 3),
            &[("lakeside", 0.5)],
            vec![face(0.35, 0.3, 0.08), face(0.5, 0.31, 0.08)],
            &["bride", "groom"],
            0.6,
            7,
            Some("couple_session"),
        );
    }
    let group: Vec<FaceBrief> =
        (0..9).map(|k| face(0.05 + k as f32 * 0.1, 0.4 + (k % 2) as f32 * 0.06, 0.05)).collect();
    d.add(at(15, 10), &[("patio", 0.05)], group, &[], 0.6, 8, Some("groups"));
    d.add(
        at(16, 20),
        &[("goblet", 0.4), ("red wine", 0.3)],
        crowd.clone(),
        &[],
        0.6,
        9,
        Some("cocktail"),
    );
    d.add(
        at(19, 10),
        &[("plate", 0.5), ("restaurant", 0.3)],
        crowd.clone(),
        &[],
        0.5,
        10,
        Some("dinner"),
    );
    d.add(at(21, 0), &[("microphone", 0.5)], one(), &[], 0.4, 11, Some("speeches"));
    d.add(
        at(21, 30),
        &[("trifle", 0.4), ("confectionery", 0.3)],
        vec![],
        &[],
        0.45,
        12,
        Some("cake"),
    );
    d.add(
        at(22, 30),
        &[("stage", 0.4), ("spotlight", 0.3)],
        vec![face(0.4, 0.3, 0.07), face(0.55, 0.32, 0.07)],
        &["bride", "groom"],
        0.15,
        13,
        Some("first_dance"),
    );
    d.add(at(23, 20), &[("stage", 0.2)], crowd, &[], 0.18, 14, Some("party"));
    // Sem pistas suficientes: não se força.
    d.add(at(17, 40), &[("tabby", 0.3)], vec![], &[], 0.6, 15, None);
    d
}

#[test]
fn wedding_day_is_classified_and_unsure_photos_stay_unclassified() {
    let d = wedding();
    let res = classify(&d.photos, &HashMap::new(), &Cues::new());
    let mut wrong = Vec::new();
    for (id, exp) in &d.expect {
        let r = res.iter().find(|r| r.id == *id).unwrap();
        let got = r.label.map(|l| match l {
            MomentLabel::Kind(k) => k,
            MomentLabel::Custom(_) => "custom",
        });
        if got != *exp {
            wrong.push((*id, *exp, got, r.confidence, r.reasons.clone()));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    let unsure = res.iter().find(|r| r.id == d.photos.len() as i64).unwrap();
    assert!(
        unsure.label.is_none() && unsure.reasons[0].code == "noEvidence"
            || unsure.reasons[0].code == "lowConfidence"
    );
}

#[test]
fn without_ceremony_or_cues_nothing_is_forced() {
    // Fotografias só com rostos, sem cena reconhecível e sem cerimónia detetada.
    let photos: Vec<MomentPhoto> = (0..30)
        .map(|i| MomentPhoto {
            id: i,
            time_ms: Some(at(15, i as u32)),
            faces: vec![face(0.4, 0.3, 0.1)],
            brightness: 0.6,
            ..Default::default()
        })
        .collect();
    let res = classify(&photos, &HashMap::new(), &Cues::new());
    assert!(
        res.iter().all(|r| r.label.is_none()),
        "{:?}",
        res.iter().filter(|r| r.label.is_some()).collect::<Vec<_>>()
    );
}

#[test]
fn user_choices_teach_similar_photos_including_custom_moments() {
    let mut d = wedding();
    // Fogo de artifício (momento personalizado): o utilizador classifica 3; há mais 2 parecidas.
    let ids: Vec<i64> = (0..5)
        .map(|k| d.add(at(23, 50 + k), &[("tabby", 0.01)], vec![], &[], 0.1, 40, None))
        .collect();
    let mut user = HashMap::new();
    for id in &ids[..3] {
        user.insert(*id, Some(MomentLabel::Custom(900)));
    }
    // E corrige uma fotografia da cerimónia para “troca de alianças”: as outras não mudam
    // por isso (não são suficientemente parecidas), mas a decisão é respeitada.
    user.insert(20, Some(MomentLabel::Kind("ring_exchange")));
    let res = classify(&d.photos, &user, &Cues::new());
    assert!(
        res.iter().all(|r| !user.contains_key(&r.id)),
        "decisões do utilizador não são reclassificadas"
    );
    for id in &ids[3..] {
        let r = res.iter().find(|r| r.id == *id).unwrap();
        assert_eq!(r.label, Some(MomentLabel::Custom(900)), "{r:?}");
        assert!(r
            .reasons
            .iter()
            .any(|x| x.code == "similarToUserChoice" || x.code == "sceneMajorityUserChoice"));
    }
}

#[test]
fn gps_jump_splits_scenes() {
    let mk = |id: i64, m: u32, gps: (f64, f64)| MomentPhoto {
        id,
        time_ms: Some(at(14, m)),
        gps: Some(gps),
        ..Default::default()
    };
    let photos =
        vec![mk(1, 0, (38.79, -9.39)), mk(2, 1, (38.7901, -9.3901)), mk(3, 2, (38.75, -9.20))];
    let order = vec![0, 1, 2];
    assert_eq!(scenes(&order, &photos), vec![vec![0, 1], vec![2]]);
}

#[test]
fn performance_15000_photos() {
    let base = wedding();
    let mut photos = Vec::new();
    for rep in 0..(15_000 / base.photos.len() + 1) {
        for p in &base.photos {
            let mut q = p.clone();
            q.id = photos.len() as i64;
            q.time_ms = q.time_ms.map(|t| t + rep as i64 * 2000);
            photos.push(q);
        }
    }
    photos.truncate(15_000);
    let mut user = HashMap::new();
    for i in (0..15_000).step_by(500) {
        user.insert(i as i64, Some(MomentLabel::Kind("party")));
    }
    let t = std::time::Instant::now();
    let res = classify(&photos, &user, &Cues::new());
    let e = t.elapsed();
    eprintln!(
        "15 000 fotografias: momentos em {e:.2?}, {} classificadas",
        res.iter().filter(|r| r.label.is_some()).count()
    );
    assert!(e.as_secs_f32() < 20.0);
}
