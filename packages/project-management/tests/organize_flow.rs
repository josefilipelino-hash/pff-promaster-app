//! Fase 4 — organização do casamento pelo caminho real da aplicação:
//! importar → analisar (rostos + identidade + cena) → pessoas agrupadas → identificar
//! noiva/noivo → pesquisas (“noiva”, “noivos juntos”) → sugestão de sincronização entre
//! câmaras (Canon correta, Sony +3min42s) → cronologia reconstruída → momentos (arrastar,
//! personalizados, aprendizagem com exemplos) → fechar/reabrir.
//!
//! Precisa de fotografias reais com rostos (`MOMENTO_FACES_DIR`, com `portrait.jpg` e
//! `lena.jpg`); sem elas, o teste é ignorado. Nada é copiado para o repositório.

use image::{imageops, Rgb, RgbImage};
use momento_core_types::AssetId;
use momento_database::clock::CameraOffset;
use momento_database::import::ImportMode;
use momento_import_engine::ImportProgress;
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

struct Quiet;
impl SessionEvents for Quiet {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &ThumbBatch) {}
}

fn faces_dir() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var("MOMENTO_FACES_DIR").ok()?);
    (d.join("portrait.jpg").is_file() && d.join("lena.jpg").is_file()).then_some(d)
}

/// Fundo com textura (cada “cena” tem a sua cor) e pessoas coladas.
fn canvas(scene: u32, people: &[(&RgbImage, u32, u32, u32)], jitter: u32) -> RgbImage {
    let hue = [(200u8, 180u8, 160u8), (90, 120, 160), (140, 170, 110), (180, 110, 120)]
        [scene as usize % 4];
    let mut img = RgbImage::from_fn(1600, 1000, |x, y| {
        let n = ((x / 9 + y / 7 + scene * 13 + jitter) % 11) as u8 * 3;
        Rgb([hue.0.saturating_add(n), hue.1.saturating_add(n), hue.2.saturating_add(n)])
    });
    for (p, x, y, h) in people {
        let w = p.width() * h / p.height();
        let r = imageops::resize(*p, w, *h, imageops::FilterType::Triangle);
        imageops::overlay(&mut img, &r, *x as i64 + jitter as i64 * 3, *y as i64);
    }
    img
}

fn spec(model: (&str, &str), secs: u32) -> ExifSpec {
    let mut s = spec_for(0, 1);
    s.make = Some(model.0.into());
    s.model = Some(model.1.into());
    s.serial = None;
    let t = 14 * 3600 + secs;
    s.date_time_original =
        Some(format!("2026:06:15 {:02}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60));
    s.sub_sec = None;
    s
}

const CANON: (&str, &str) = ("Canon", "EOS R5");
const SONY: (&str, &str) = ("SONY", "ILCE-7M4");

fn generate(dir: &Path, faces: &Path) {
    let a = image::open(faces.join("portrait.jpg")).unwrap().to_rgb8(); // “noiva”
    let b = image::open(faces.join("lena.jpg")).unwrap().to_rgb8(); // “noivo”
    let mut n = 0;
    let mut put = |cam: (&str, &str), secs: u32, img: &RgbImage| {
        n += 1;
        write(
            &dir.join(format!("{}_{n:03}.jpg", cam.0)),
            &jpeg_bytes(img, Some(&spec(cam, secs)), 90),
        )
        .unwrap();
    };
    // 6 momentos, 2 min entre eles. Em cada um: a Canon (hora certa) e a Sony
    // (relógio adiantado 3min42s) fotografam o mesmo instante.
    for m in 0..6u32 {
        let t = m * 120;
        let people: Vec<(&RgbImage, u32, u32, u32)> = match m % 3 {
            0 => vec![(&a, 500, 150, 700)],
            1 => vec![(&b, 550, 200, 600)],
            _ => vec![(&a, 250, 150, 700), (&b, 900, 200, 600)],
        };
        for k in 0..3 {
            put(CANON, t + k * 4, &canvas(m, &people, k));
        }
        put(SONY, t + 2 + 222, &canvas(m, &people, 5));
    }
}

fn id_of(s: &ProjectSession, name: &str) -> AssetId {
    s.list_photos().unwrap().into_iter().find(|p| p.file_name == name).unwrap().id
}

#[test]
fn people_moments_and_camera_sync() {
    let Some(faces) = faces_dir() else {
        eprintln!("MOMENTO_FACES_DIR em falta: teste ignorado");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    generate(&src, &faces);
    let paths = create_project(&NewProject {
        name: "Casamento".into(),
        client_name: "Ana & Pedro".into(),
        event_type: "wedding".into(),
        event_date: Some("2026-06-15".into()),
        location: "Sintra".into(),
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
    assert_eq!(s.list_photos().unwrap().len(), 24);
    let t = Instant::now();
    s.start_analysis(false).unwrap();
    s.wait_analysis();
    eprintln!("análise de 24 fotografias (com identidade e cena): {:.2?}", t.elapsed());

    // ---- Pessoas: duas pessoas agrupadas automaticamente.
    let persons = s.list_persons().unwrap();
    eprintln!(
        "pessoas: {:?}",
        persons.iter().map(|p| (p.id, p.photo_count, p.face_count)).collect::<Vec<_>>()
    );
    assert_eq!(persons.len(), 2, "{persons:?}");
    let pp = s.photo_persons().unwrap();
    let photos_of = |pid: i64| pp.iter().filter(|x| x.1 == pid).map(|x| x.0).collect::<Vec<_>>();
    // Cada pessoa aparece em 16 fotografias (8 sozinha + 8 juntas).
    assert!(persons.iter().all(|p| p.photo_count == 16), "{persons:?}");
    // Identificar: a pessoa que está em Canon_001 (momento 0, só a “noiva”) é a noiva.
    let first = id_of(&s, "Canon_001.jpg");
    let bride = pp.iter().find(|x| x.0 == first).unwrap().1;
    let groom = persons.iter().find(|p| p.id != bride).unwrap().id;
    s.update_person(bride, Some("Ana"), Some("bride"), false).unwrap();
    s.update_person(groom, Some("Pedro"), Some("groom"), false).unwrap();
    let pp = s.photo_persons().unwrap();
    let together: Vec<AssetId> =
        photos_of(bride).into_iter().filter(|a| pp.contains(&(*a, groom))).collect();
    assert_eq!(together.len(), 8, "noivos juntos = momentos 2 e 5 (4 fotografias cada)");
    // Reagrupar não desfaz a identificação.
    s.recluster_people().unwrap();
    assert_eq!(s.list_persons().unwrap().iter().filter(|p| p.role.is_some()).count(), 2);

    // ---- Sincronização: a Sony está adiantada 3min42s.
    let cams = s.list_cameras().unwrap();
    assert_eq!(cams.len(), 2);
    let sugg = s.suggest_offsets("Canon EOS R5").unwrap();
    eprintln!("sugestões: {sugg:?}");
    let sony = sugg.iter().find(|x| x.camera == "SONY ILCE-7M4").expect("sugestão para a Sony");
    // Precisão: a do par de fotografias mais parecido (aqui, até 8 s ao lado).
    assert!((sony.offset_ms + 222_000).abs() <= 8000, "{sony:?}");
    // Antes: a Sony do momento 0 (14:03:44) aparece depois da Canon do momento 0.
    let order = |s: &ProjectSession| {
        s.list_photos().unwrap().into_iter().map(|p| p.file_name).collect::<Vec<_>>()
    };
    let before = order(&s);
    let summary = s
        .set_camera_offsets(&[
            CameraOffset { key: "Canon EOS R5".into(), offset_ms: 0, reference: true },
            CameraOffset {
                key: "SONY ILCE-7M4".into(),
                offset_ms: sony.offset_ms,
                reference: false,
            },
        ])
        .unwrap();
    let after = order(&s);
    assert_ne!(before, after);
    // Depois: cada fotografia da Sony fica junto das Canon do mesmo momento.
    let pos = |v: &[String], n: &str| v.iter().position(|x| x == n).unwrap();
    assert!(pos(&after, "SONY_004.jpg") < pos(&after, "Canon_005.jpg"), "{after:?}");
    assert!(pos(&after, "SONY_004.jpg") > pos(&after, "Canon_001.jpg"));
    eprintln!("cronologia reconstruída: {summary:?}");
    // As sequências passam a juntar as duas câmaras do mesmo instante.
    let groups = s.list_groups().unwrap();
    assert!(
        groups.iter().any(|g| g.members.contains(&id_of(&s, "SONY_004.jpg"))
            && g.members.contains(&id_of(&s, "Canon_001.jpg"))),
        "{groups:?}"
    );

    // ---- Momentos: sem pistas de cena de casamento, nada é forçado.
    let moments = s.list_moments().unwrap();
    assert_eq!(moments.iter().filter(|m| !m.custom).count(), 25);
    let am = s.asset_moments().unwrap();
    assert!(am.is_empty(), "retratos sem pistas de momento ficam por classificar: {am:?}");
    // O utilizador cria “Sessão no jardim” e arrasta 3 fotografias do momento 2 (juntos).
    let custom = s.create_moment("Sessão no jardim", None).unwrap();
    let m2: Vec<AssetId> =
        ["Canon_009.jpg", "Canon_010.jpg", "Canon_011.jpg"].iter().map(|n| id_of(&s, n)).collect();
    s.assign_moment(&m2, Some(custom)).unwrap();
    // A Sony do mesmo instante é parecida → aprende com o exemplo.
    let am = s.asset_moments().unwrap();
    let sony_m2 = am
        .iter()
        .find(|m| m.asset == id_of(&s, "SONY_012.jpg"))
        .expect("classificada pelo exemplo");
    assert_eq!((sony_m2.moment_id, sony_m2.source.as_str()), (Some(custom), "auto"));
    assert!(sony_m2.reasons.as_deref().unwrap_or("").contains("UserChoice"));
    // “Por classificar” explícito e devolver à classificação automática.
    s.assign_moment(&[id_of(&s, "SONY_012.jpg")], None).unwrap();
    assert_eq!(
        s.asset_moments()
            .unwrap()
            .iter()
            .find(|m| m.asset == id_of(&s, "SONY_012.jpg"))
            .unwrap()
            .source,
        "user"
    );
    s.reset_moments(&[id_of(&s, "SONY_012.jpg")]).unwrap();
    assert_eq!(
        s.asset_moments()
            .unwrap()
            .iter()
            .find(|m| m.asset == id_of(&s, "SONY_012.jpg"))
            .unwrap()
            .moment_id,
        Some(custom)
    );

    // ---- Reabrir: tudo persistido; a análise não volta a correr.
    s.close();
    drop(s);
    let s2 = ProjectSession::open(&paths.root, Arc::new(Quiet)).unwrap();
    assert_eq!(s2.list_persons().unwrap().iter().filter(|p| p.name.is_some()).count(), 2);
    assert_eq!(
        s2.list_cameras().unwrap().iter().find(|c| c.key == "SONY ILCE-7M4").unwrap().offset_ms,
        sony.offset_ms
    );
    assert!(s2
        .list_moments()
        .unwrap()
        .iter()
        .any(|m| m.name.as_deref() == Some("Sessão no jardim") && m.count >= 4));
    assert_eq!(order(&s2), after);
    s2.clear_people().unwrap();
    assert!(s2.list_persons().unwrap().is_empty(), "dados de pessoas apagados a pedido");
    s2.close();
}
