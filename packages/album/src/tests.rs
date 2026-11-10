//! Montagem automática, verificação, composição e exportação.

use crate::auto::{build, AutoOptions};
use crate::export::{export, ExportKind};
use crate::layouts;
use crate::model::*;
use crate::preflight::{check, PhotoInfo, Severity};
use crate::spec::*;
use momento_image_processing::develop::LinearImage;
use std::collections::HashMap;
use std::sync::Arc;

fn photo(key: i64, moment: &str, hour: u32, min: u32, landscape: bool, score: f32) -> PhotoInfo {
    PhotoInfo {
        key,
        width: if landscape { 6000 } else { 4000 },
        height: if landscape { 4000 } else { 6000 },
        time: Some(format!("2026-06-15T{hour:02}:{min:02}:00")),
        moment: Some(moment.into()),
        flag: 1,
        stars: 3,
        score,
        faces: vec![[0.42, 0.25, 0.16, 0.2]],
        persons: vec![1],
        group: None,
    }
}

fn wedding() -> Vec<PhotoInfo> {
    let mut v = Vec::new();
    let plan = [
        ("bride_prep", 10, 8),
        ("details", 11, 4),
        ("ceremony", 15, 8),
        ("kiss", 16, 2),
        ("couple_session", 17, 6),
        ("groups", 18, 4),
        ("cocktail", 19, 5),
        ("dinner", 20, 5),
        ("first_dance", 22, 3),
        ("party", 23, 8),
    ];
    let mut k = 0;
    for (m, h, n) in plan {
        for i in 0..n {
            k += 1;
            v.push(photo(k, m, h, i * 4, k % 3 != 0, 0.4 + (k % 7) as f32 / 20.0));
        }
    }
    v
}

fn spec() -> AlbumSpec {
    AlbumSpec {
        width_mm: 300.0,
        height_mm: 300.0,
        pages: 24,
        dpi: 300,
        confirmed: true,
        ..Default::default()
    }
}

#[test]
fn automatic_album_tells_the_story_in_order() {
    let mut photos = wedding();
    // A melhor do casal, horizontal: abertura.
    photos
        .iter_mut()
        .find(|p| p.moment.as_deref() == Some("couple_session") && p.width > p.height)
        .unwrap()
        .score = 0.99;
    // Rajada: duas iguais no mesmo segundo → só uma.
    let mut dup = photos[0].clone();
    dup.key = 900;
    dup.score = 0.1;
    photos.push(dup);
    // Grupo de sequência: só a melhor.
    photos[2].group = Some(7);
    photos[3].group = Some(7);
    // Baixa resolução: excluída.
    photos.push(PhotoInfo { key: 901, width: 640, height: 427, ..photos[5].clone() });
    // Rejeitada: excluída.
    photos.push(PhotoInfo { key: 902, flag: -1, ..photos[6].clone() });
    let s = spec();
    let r = build("Ana & Pedro", &s, &photos, &AutoOptions::default());
    let a = &r.album;
    assert_eq!(a.spreads.len(), 12, "páginas da especificação");
    assert_eq!(r.skipped_low_res, 1);
    assert!(r.skipped_duplicates >= 2, "{}", r.skipped_duplicates);
    let used = a.photos();
    assert!(!used.contains(&900) && !used.contains(&901) && !used.contains(&902));
    // Sem repetições e no máximo 3 por página.
    let mut all: Vec<i64> =
        a.spreads.iter().flat_map(|s| s.slots.iter().filter_map(|x| x.photo)).collect();
    let n = all.len();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), n, "sem repetições");
    for sp in &a.spreads {
        for page in 0..2 {
            let x0 = page as f32 * 300.0;
            let c = sp
                .slots
                .iter()
                .filter(|sl| {
                    sl.photo.is_some() && sl.x + sl.w / 2.0 >= x0 && sl.x + sl.w / 2.0 < x0 + 300.0
                })
                .count();
            assert!(c <= 3, "{} fotografias numa página ({})", c, sp.template);
        }
    }
    // Abertura: a melhor fotografia do casal, sozinha.
    let first = &a.spreads[0];
    assert_eq!(first.slots.len(), 1);
    let best = photos.iter().max_by(|a, b| a.score.total_cmp(&b.score)).unwrap().key;
    assert_eq!(first.slots[0].photo, Some(best));
    // Narrativa: os momentos aparecem pela ordem do casamento.
    let order = [
        "bride_prep",
        "details",
        "ceremony",
        "kiss",
        "couple_session",
        "groups",
        "cocktail",
        "dinner",
        "first_dance",
        "party",
    ];
    let by_key: HashMap<i64, &PhotoInfo> = photos.iter().map(|p| (p.key, p)).collect();
    let seq: Vec<usize> = used[1..]
        .iter()
        .map(|k| order.iter().position(|m| Some(*m) == by_key[k].moment.as_deref()).unwrap())
        .collect();
    // Capítulos agrupados: preparação (0–1) < cerimónia (2–3) < casal (4) < família (5) < receção (6–7) < festa (8–9).
    let chapter = |i: usize| [0, 0, 1, 1, 2, 3, 4, 4, 5, 5][i];
    assert!(seq.windows(2).all(|w| chapter(w[0]) <= chapter(w[1])), "{seq:?}");
    assert!(a.notes.first().is_some_and(|n| n.starts_with("abertura")));
    // O álbum gerado passa a verificação sem erros.
    let infos: HashMap<i64, PhotoInfo> = photos.iter().map(|p| (p.key, p.clone())).collect();
    let errors: Vec<_> =
        check(a, &infos).into_iter().filter(|i| i.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn no_faces_on_the_fold() {
    // Panorâmica com um rosto ao centro: a montagem não deixa o rosto na dobra.
    let mut p = photo(1, "couple_session", 17, 0, true, 0.9);
    p.width = 9000;
    p.height = 3000;
    p.faces = vec![[0.47, 0.3, 0.06, 0.15]];
    let s = spec();
    let r = build("x", &s, &[p.clone()], &AutoOptions::default());
    let infos = HashMap::from([(1, p)]);
    assert!(!check(&r.album, &infos).iter().any(|i| i.code == "album.issue.faceInGutter"));
}

#[test]
fn preflight_finds_the_problems() {
    let s = AlbumSpec { confirmed: false, ..spec() };
    let mut a = Album::new("t", s);
    let infos = HashMap::from([
        (1, PhotoInfo { key: 1, width: 1200, height: 800, ..Default::default() }),
        (
            2,
            PhotoInfo {
                key: 2,
                width: 6000,
                height: 4000,
                faces: vec![[0.45, 0.3, 0.1, 0.2]],
                ..Default::default()
            },
        ),
    ]);
    a.spreads[0].slots = vec![
        // 1200 px em 30 cm: ~100 DPI.
        Slot {
            id: "a".into(),
            x: 20.0,
            y: 20.0,
            w: 300.0,
            h: 200.0,
            photo: Some(1),
            ..Default::default()
        },
        // Atravessa a dobra com o rosto ao meio.
        Slot {
            id: "b".into(),
            x: 150.0,
            y: 50.0,
            w: 300.0,
            h: 200.0,
            photo: Some(2),
            ..Default::default()
        },
        // Borda a 2 mm do corte: nem sangra nem está a salvo.
        Slot {
            id: "c".into(),
            x: 2.0,
            y: 250.0,
            w: 50.0,
            h: 30.0,
            photo: Some(2),
            ..Default::default()
        },
        Slot {
            id: "d".into(),
            x: 400.0,
            y: 250.0,
            w: 50.0,
            h: 30.0,
            photo: None,
            ..Default::default()
        },
    ];
    let issues = check(&a, &infos);
    let has = |code: &str, slot: Option<&str>| {
        issues.iter().any(|i| i.code == code && i.slot.as_deref() == slot)
    };
    assert!(has("album.issue.unconfirmed", None));
    assert!(issues.iter().any(|i| i.code == "album.issue.lowDpi"
        && i.severity == Severity::Error
        && i.slot.as_deref() == Some("a")));
    assert!(has("album.issue.faceInGutter", Some("b")));
    assert!(has("album.issue.edgeNearTrim", Some("c")));
    assert!(has("album.issue.empty", Some("d")));
    assert!(has("album.issue.repeated", None));
    // Especificação inválida.
    let bad = AlbumSpec { pages: 21, ..spec() };
    assert!(bad.problems().contains(&"album.problem.pagesEven"));
    // Formatos e orientação.
    let f = AlbumSpec::default().with_format("20x30", Orientation::Portrait);
    assert_eq!((f.width_mm, f.height_mm, f.orientation()), (200.0, 300.0, Orientation::Portrait));
    // Preset de laboratório: aplica e confirma.
    let mut s2 = AlbumSpec::default();
    let lab = LabPreset {
        name: "Lab X".into(),
        dpi: 400,
        bleed_mm: 5.0,
        safe_mm: 12.0,
        gutter_mm: 20.0,
        ..Default::default()
    };
    lab.apply(&mut s2);
    assert!(s2.confirmed && s2.bleed_mm == 5.0 && s2.dpi == 400 && s2.lab == "Lab X");
}

fn solid(c: [f32; 3], w: u32, h: u32) -> Arc<LinearImage> {
    Arc::new(LinearImage { width: w, height: h, px: vec![c; (w * h) as usize] })
}

#[test]
fn compose_and_export_files() {
    let s = AlbumSpec {
        width_mm: 200.0,
        height_mm: 200.0,
        pages: 4,
        dpi: 100,
        bleed_mm: 3.0,
        confirmed: true,
        ..Default::default()
    };
    let mut a = Album::new("Ana & Pedro", s.clone());
    a.spreads[0] = layouts::apply(&s, "B|F", &[(1, 1.5), (2, 1.5)], 12.0, 4.0).unwrap();
    a.spreads[1] = layouts::apply(&s, "P", &[(3, 2.0)], 12.0, 4.0).unwrap();
    let provider = |k: i64, _: u32| -> Option<Arc<LinearImage>> {
        Some(match k {
            1 => solid([0.8, 0.0, 0.0], 1500, 1000),
            2 => solid([0.0, 0.6, 0.0], 1500, 1000),
            _ => solid([0.0, 0.0, 0.7], 3000, 1500),
        })
    };
    let ppm = s.px_per_mm();
    let img = crate::compose::compose(&s, &a.spreads[0], &provider, ppm, false);
    let (w, h) = crate::compose::canvas_size(&s, ppm);
    assert_eq!((img.width, img.height), (w, h));
    assert_eq!(w, (406.0 * ppm).round() as u32, "spread com sangria");
    let at = |x_mm: f32, y_mm: f32| {
        img.px[(((y_mm + 3.0) * ppm) as usize) * w as usize + ((x_mm + 3.0) * ppm) as usize]
    };
    assert!(at(-2.0, 100.0)[0] > 0.7, "página esquerda a sangrar até à sangria");
    assert!(at(300.0, 100.0)[1] > 0.5, "moldura verde na página direita");
    assert!(at(395.0, 5.0)[0] > 0.9 && at(395.0, 5.0)[1] > 0.9, "margem branca");
    // Ficheiros.
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let n = export(
        &a,
        ExportKind::Jpeg,
        &provider,
        &mut |name, b| {
            files.push((name.into(), b));
            Ok(())
        },
        &|_, _| true,
    )
    .unwrap();
    assert_eq!(n, 2);
    assert_eq!(files[0].0, "ana-e-pedro-spread-01.jpg");
    let dec = image::load_from_memory(&files[0].1).unwrap();
    assert_eq!((dec.width(), dec.height()), (w, h));
    // Por páginas (o laboratório pede assim): 4 ficheiros com sangria nos 4 lados.
    let mut pa = a.clone();
    pa.spec.export_unit = ExportUnit::Pages;
    let mut pages = Vec::new();
    export(
        &pa,
        ExportKind::Tiff,
        &provider,
        &mut |name, b| {
            pages.push((name.to_string(), b));
            Ok(())
        },
        &|_, _| true,
    )
    .unwrap();
    assert_eq!(pages.len(), 4);
    assert_eq!(pages[3].0, "ana-e-pedro-pagina-004.tif");
    let p = image::load_from_memory(&pages[0].1).unwrap();
    assert_eq!(p.width(), (206.0 * ppm).round() as u32);
    // PDF de impressão: 2 páginas com TrimBox para dentro da sangria.
    let mut pdf = Vec::new();
    export(
        &a,
        ExportKind::PrintPdf,
        &provider,
        &mut |_, b| {
            pdf = b;
            Ok(())
        },
        &|_, _| true,
    )
    .unwrap();
    let t = String::from_utf8_lossy(&pdf);
    assert!(t.starts_with("%PDF-1.6") && t.contains("/Count 2"));
    let bleed_pt = 3.0 / 25.4 * 72.0;
    assert!(t.contains(&format!("/TrimBox [{bleed_pt:.3} {bleed_pt:.3}")), "TrimBox");
    assert!(t.contains("/ICCBased"));
    // Prova: mais leve.
    let mut proof = Vec::new();
    export(
        &a,
        ExportKind::ProofPdf,
        &provider,
        &mut |_, b| {
            proof = b;
            Ok(())
        },
        &|_, _| true,
    )
    .unwrap();
    // Prova: resolução reduzida (120 DPI em vez de 100 aqui não serve de comparação → ver largura).
    let width_of = |b: &[u8]| {
        let t = String::from_utf8_lossy(b);
        let i = t.find("/Width ").unwrap() + 7;
        t[i..].split_whitespace().next().unwrap().parse::<u32>().unwrap()
    };
    assert_eq!(width_of(&pdf), w);
    assert_eq!(width_of(&proof), ((406.0f32 * 120.0 / 25.4).round()) as u32, "prova a 120 DPI");
    // Cancelar.
    assert!(export(&a, ExportKind::Jpeg, &provider, &mut |_, _| Ok(()), &|d, _| d == 0).is_err());
}

#[test]
fn small_photos_are_never_enlarged_beyond_the_lab_dpi() {
    // Fotografias que passam o filtro (1/3 de página) mas não enchem uma página: o
    // automático reduz as molduras em vez de as imprimir ampliadas.
    let photos: Vec<PhotoInfo> = (0..10)
        .map(|i| PhotoInfo {
            width: 1600,
            height: 1067,
            faces: vec![],
            ..photo(
                i + 1,
                ["ceremony", "party"][(i / 5) as usize],
                15 + (i / 5) as u32 * 6,
                (i % 5) as u32 * 7,
                true,
                0.5 + i as f32 / 100.0,
            )
        })
        .collect();
    let r =
        build("Pequenas", &spec(), &photos, &AutoOptions { max_per_page: 1, ..Default::default() });
    assert_eq!(r.used, 10);
    let infos: HashMap<i64, PhotoInfo> = photos.iter().map(|p| (p.key, p.clone())).collect();
    let low: Vec<_> =
        check(&r.album, &infos).into_iter().filter(|i| i.code == "album.issue.lowDpi").collect();
    assert!(low.is_empty(), "{low:?}");
    // E fotografias pequenas demais nem para 1/3 de página ficam de fora.
    let tiny = vec![PhotoInfo { width: 800, height: 533, ..photo(99, "party", 23, 0, true, 0.9) }];
    let r = build("Minúsculas", &spec(), &tiny, &AutoOptions::default());
    assert_eq!((r.used, r.skipped_low_res), (0, 1));
}
