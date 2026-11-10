//! Fase 10 — galeria pelo caminho real da aplicação: definições no catálogo, escolha
//! (favoritas, ocultas), capítulos pelos momentos, geração a partir dos originais, PIN,
//! substituição segura, favoritas do cliente de volta à aplicação, originais intactos.

use momento_core_types::AssetId;
use momento_database::import::ImportMode;
use momento_gallery::site::read_manifest;
use momento_gallery::{Downloads, Protection, Selection};
use momento_import_engine::ImportProgress;
use momento_project_management::export::JobState;
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

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

fn wait(s: &ProjectSession, id: u64) -> momento_project_management::export::ExportProgress {
    let t = Instant::now();
    loop {
        let p = s.export_jobs().into_iter().find(|p| p.id == id).unwrap();
        if matches!(p.state, JobState::Done | JobState::Failed | JobState::Cancelled) {
            return p;
        }
        assert!(t.elapsed() < Duration::from_secs(300));
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn wedding_gallery_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    let dest = tmp.path().join("publicar");
    std::fs::create_dir_all(&work).unwrap();
    for i in 0..6 {
        write(
            &src.join(format!("IMG_{i}.jpg")),
            &jpeg_bytes(&test_image(1200, 800, i, 1), Some(&spec_for(i, 1)), 90),
        )
        .unwrap();
    }
    let originals: Vec<(std::path::PathBuf, u64)> = std::fs::read_dir(&src)
        .unwrap()
        .flatten()
        .map(|e| {
            let h = hash(&e.path());
            (e.path(), h)
        })
        .collect();
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
    let rows = s.list_photos().unwrap();
    let key = |n: &str| rows.iter().find(|r| r.file_name == n).unwrap().id.0;
    let asset = |n: &str| rows.iter().find(|r| r.file_name == n).unwrap().asset;
    // Momentos: 0–1 preparação, 2–3 cerimónia, 4 festa; 5 rejeitada.
    let ms = s.list_moments().unwrap();
    let mid = |k: &str| ms.iter().find(|m| m.kind.as_deref() == Some(k)).unwrap().id;
    s.assign_moment(&[asset("IMG_0.jpg"), asset("IMG_1.jpg")], Some(mid("bride_prep"))).unwrap();
    s.assign_moment(&[asset("IMG_2.jpg")], Some(mid("ceremony"))).unwrap();
    s.assign_moment(&[asset("IMG_3.jpg")], Some(mid("kiss"))).unwrap();
    s.assign_moment(&[asset("IMG_4.jpg")], Some(mid("party"))).unwrap();
    s.set_flag(&[asset("IMG_5.jpg")], -1).unwrap();

    // Definições iniciais vêm do projeto.
    let mut cfg = s.gallery_config().unwrap();
    assert_eq!(
        (cfg.settings.couple.as_str(), cfg.settings.location.as_str()),
        ("Ana & Pedro", "Sintra")
    );
    cfg.settings.selection = Selection::NotRejected;
    cfg.settings.hero = Some(key("IMG_2.jpg"));
    cfg.settings.cover = Some(key("IMG_2.jpg"));
    cfg.settings.photographer = "Estúdio Lino".into();
    cfg.destination = dest.to_string_lossy().into_owned();
    s.save_gallery_config(&cfg).unwrap();
    // Ocultar uma fotografia.
    let cfg = s.set_gallery_hidden(&[key("IMG_1.jpg")], true).unwrap();
    assert_eq!(cfg.settings.hidden, [key("IMG_1.jpg")]);
    let names: HashMap<i64, String> = HashMap::new();
    let pv = s.gallery_preview(&cfg, &names).unwrap();
    assert_eq!((pv.photos, pv.hidden), (4, 1), "6 − rejeitada − oculta");
    let titles: Vec<&str> = pv.chapters.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["Preparação", "Cerimónia", "Festa"]);
    assert!(pv.problems.is_empty() && !pv.exists && !pv.blocked, "{pv:?}");

    // Gerar.
    let id = s.start_gallery(cfg.clone(), names.clone(), false).unwrap();
    let done = wait(&s, id);
    assert_eq!(done.state, JobState::Done, "{:?}", done.errors);
    let folder = dest.join("ana-e-pedro-galeria");
    assert_eq!(done.index.as_deref(), Some(folder.join("index.html").to_string_lossy().as_ref()));
    let m = read_manifest(&folder, None).unwrap();
    assert_eq!(m["photos"].as_array().unwrap().len(), 4);
    let all: Vec<&str> =
        m["photos"].as_array().unwrap().iter().map(|p| p["n"].as_str().unwrap()).collect();
    assert!(!all.contains(&"IMG_1") && !all.contains(&"IMG_5"), "oculta e rejeitada fora: {all:?}");
    assert!(m["hero"]["src"].as_str().unwrap().starts_with("img/w/"));
    // Imagens a partir dos originais: tamanhos certos.
    let p0 = &m["photos"][0];
    assert_eq!(
        (p0["w"].as_u64(), p0["h"].as_u64()),
        (Some(1200), Some(800)),
        "não amplia 1200 px para 2048"
    );
    assert_eq!(p0["tw"].as_u64(), Some(900));
    let d = image::open(folder.join(p0["d"].as_str().unwrap())).unwrap();
    assert_eq!(d.width(), 1200, "download em resolução total");
    let zip = folder.join(m["zips"][0]["href"].as_str().unwrap());
    assert_eq!(zip::ZipArchive::new(std::fs::File::open(&zip).unwrap()).unwrap().len(), 4);
    // Copyright nas imagens, sem GPS.
    let ex = exif::Reader::new()
        .read_from_container(&mut std::io::BufReader::new(
            std::fs::File::open(folder.join(p0["s"].as_str().unwrap())).unwrap(),
        ))
        .unwrap();
    assert!(ex.get_field(exif::Tag::Copyright, exif::In::PRIMARY).is_some());
    assert!(ex.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY).is_none());

    // Voltar a gerar: pede confirmação; com confirmação substitui; com PIN.
    assert!(s.gallery_preview(&cfg, &names).unwrap().exists);
    let e = s.start_gallery(cfg.clone(), names.clone(), false).unwrap_err();
    assert!(e.to_string().contains("gallery.problem.exists"));
    let mut pin = cfg.clone();
    pin.settings.protection = Protection::Pin;
    pin.settings.secret = "2468".into();
    pin.settings.downloads = Downloads::Individual;
    let done = wait(&s, s.start_gallery(pin, names.clone(), true).unwrap());
    assert_eq!(done.state, JobState::Done, "{:?}", done.errors);
    assert!(read_manifest(&folder, None).is_err(), "cifrada");
    let m = read_manifest(&folder, Some("2468")).unwrap();
    assert_eq!(m["photos"].as_array().unwrap().len(), 4);
    assert!(m["zips"].as_array().unwrap().is_empty());
    assert!(!folder.join("download").exists(), "a geração anterior foi substituída");

    // Uma pasta que não é do Momento nunca é tocada.
    let mut other = cfg.clone();
    other.settings.couple = "Rita & Rui".into();
    let foreign = dest.join("rita-e-rui-galeria");
    std::fs::create_dir_all(&foreign).unwrap();
    std::fs::write(foreign.join("importante.txt"), "não apagar").unwrap();
    assert!(s.gallery_preview(&other, &names).unwrap().blocked);
    assert!(s.start_gallery(other, names.clone(), true).is_err());
    assert!(foreign.join("importante.txt").is_file());

    // Favoritas do cliente de volta à aplicação (etiqueta roxa).
    let r = s.import_client_favorites("IMG_0\nIMG_3.jpg, desconhecida").unwrap();
    assert_eq!(r.matched, 2);
    assert_eq!(r.unmatched, ["desconhecida"]);
    let rows = s.list_photos().unwrap();
    let purple: Vec<&str> = rows
        .iter()
        .filter(|r| r.color.as_deref() == Some("purple"))
        .map(|r| r.file_name.as_str())
        .collect();
    assert_eq!(purple.len(), 2);
    let _ = AssetId(0);

    for (p, h) in &originals {
        assert_eq!(hash(p), *h, "{p:?}");
    }
    s.close();
}
