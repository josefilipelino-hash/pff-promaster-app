//! Fase 15 — um casamento completo, como num trabalho real, pelo caminho da aplicação:
//!
//! cartão (DCIM, duas câmaras, RAW + JPEG, rajada, cópia repetida, ficheiros corrompidos,
//! nomes Unicode e longos) → importação com cópia verificada + disco de segurança → RAW →
//! miniaturas → análise → seleção → momentos → edição → auto edit → preset → revisão
//! (desfazer/refazer) → **reinício da aplicação** → exportação → galeria → álbum.
//!
//! No fim: originais intactos (byte a byte), cópias de trabalho e de segurança iguais aos
//! originais, catálogo válido, nada perdido com o reinício, exportações corretas.
//!
//! Material real opcional: `MOMENTO_SAMPLES_DIR` (RAW de várias marcas),
//! `MOMENTO_FACES_DIR` (fotografias reais com pessoas). Os modelos de análise vêm de
//! `models/` (sem modelos, a análise é saltada e isso é dito na saída).

use image::{Rgb, RgbImage};
use momento_album::export::ExportKind;
use momento_album::AlbumSpec;
use momento_database::assets::ThumbState;
use momento_database::import::ImportMode;
use momento_database::library::Library;
use momento_export_engine::presets::wedding_plan;
use momento_gallery::Selection;
use momento_image_processing::develop::auto::{AutoOptions, Intensity};
use momento_import_engine::ImportProgress;
use momento_project_management::export::{ExportRequest, JobState};
use momento_project_management::paths::CacheLevel;
use momento_project_management::preset_store::PresetStore;
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Rec {
    summaries: Mutex<Vec<ImportSummary>>,
}
impl SessionEvents for Rec {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, s: &ImportSummary) {
        self.summaries.lock().unwrap().push(s.clone());
    }
    fn thumbnails(&self, _: &ThumbBatch) {}
}

fn blake(p: &Path) -> String {
    blake3::hash(&std::fs::read(p).unwrap()).to_hex().to_string()
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    v.sort();
    v
}

fn wait_until(what: &str, secs: u64, f: impl Fn() -> bool) {
    let t = Instant::now();
    while !f() {
        assert!(t.elapsed() < Duration::from_secs(secs), "tempo esgotado: {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn wait_job(s: &ProjectSession, id: u64) -> momento_project_management::export::ExportProgress {
    let t = Instant::now();
    loop {
        let p = s.export_jobs().into_iter().find(|p| p.id == id).unwrap();
        if matches!(p.state, JobState::Done | JobState::Cancelled | JobState::Failed) {
            return p;
        }
        assert!(t.elapsed() < Duration::from_secs(900), "trabalho não terminou: {p:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Fotografia "de casamento" sintética com textura (para a análise ter nitidez real).
fn shot(seed: u32, blur: bool) -> RgbImage {
    let base = test_image(1600, 1067, seed, 1);
    let mut s = 0x9e37_79b9_7f4a_7c15u64 ^ seed as u64;
    let mut img = RgbImage::from_fn(1600, 1067, |x, y| {
        let p = base.get_pixel(x, y);
        // Padrão fino (detalhe) + ruído leve.
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let n = ((s >> 59) as i32) - 16;
        let d = if (x / 3 + y / 3) % 2 == 0 { 18 } else { -18 };
        Rgb(p.0.map(|c| (c as i32 + d + n).clamp(0, 255) as u8))
    });
    if blur {
        img = image::imageops::blur(&img, 6.0);
    }
    img
}

fn exif_at(cam: (&str, &str, &str), serial: &str, hh: u32, mm: u32, ss: u32, sub: u32) -> ExifSpec {
    ExifSpec {
        make: Some(cam.0.into()),
        model: Some(cam.1.into()),
        serial: Some(serial.into()),
        lens: Some(cam.2.into()),
        orientation: Some(1),
        date_time_original: Some(format!("2026:06:20 {hh:02}:{mm:02}:{ss:02}")),
        sub_sec: Some(format!("{sub:02}")),
        offset_time: Some("+01:00".into()),
        exposure_time: Some((1, 250)),
        f_number: Some((28, 10)),
        iso: Some(800),
        focal_length: Some((50, 1)),
        focal_length_35mm: Some(50),
        gps: Some((38.80, -9.38)),
    }
}

struct Card {
    root: PathBuf,
    /// Momento planeado de cada ficheiro JPEG (pela hora).
    moment_of: HashMap<String, &'static str>,
    burst: Vec<String>,
    sharp_in_burst: String,
    corrupt: Vec<String>,
    #[allow(dead_code)]
    duplicate_copy: String,
    unicode: String,
    long: String,
    raws: Vec<String>,
}

/// Cartão de memória como uma câmara o deixa (mais o segundo fotógrafo noutra pasta).
fn make_card(root: &Path) -> Card {
    let a = root.join("DCIM/100CANON");
    let b = root.join("DCIM/101SONY_");
    let canon = ("Canon", "Canon EOS R5", "RF28-70mm F2L USM");
    let sony = ("SONY", "ILCE-7M4", "FE 35mm F1.4 GM");
    let mut moment_of = HashMap::new();
    let mut n = 0u32;
    let mut put = |dir: &Path, cam, serial: &str, hh, mm, ss, sub, blur, moment: &'static str| {
        n += 1;
        let name = format!("IMG_{:04}.JPG", n);
        write(
            &dir.join(&name),
            &jpeg_bytes(&shot(n, blur), Some(&exif_at(cam, serial, hh, mm, ss, sub)), 90),
        )
        .unwrap();
        moment_of.insert(name.clone(), moment);
        name
    };
    // Preparação (10:00–10:40), cerimónia (15:00–15:40), casal (17:30), receção (19:30), festa (22:30).
    for i in 0..5 {
        put(&a, canon, "R5-0001", 10, i * 8, 0, 0, false, "bride_prep");
    }
    for i in 0..4 {
        put(&b, sony, "A7-0002", 10, 5 + i * 9, 30, 0, false, "groom_prep");
    }
    for i in 0..6 {
        put(&a, canon, "R5-0001", 15, i * 6, 10, 0, false, "ceremony");
    }
    // Rajada no beijo: 5 fotografias em 0,4 s; a terceira é a nítida, as outras tremidas.
    let mut burst = Vec::new();
    for k in 0..5 {
        burst.push(put(&a, canon, "R5-0001", 15, 38, 0, k * 10, k != 2, "kiss"));
    }
    let sharp_in_burst = burst[2].clone();
    for i in 0..4 {
        put(&b, sony, "A7-0002", 17, 30 + i * 5, 0, 0, false, "couple_session");
    }
    for i in 0..4 {
        put(&a, canon, "R5-0001", 19, 30 + i * 4, 0, 0, false, "reception");
    }
    for i in 0..4 {
        put(&b, sony, "A7-0002", 22, 30 + i * 6, 0, 0, false, "party");
    }
    // A mesma fotografia duas vezes no cartão (cópia feita pelo utilizador).
    let duplicate_copy = "IMG_0001 (cópia).JPG".to_string();
    std::fs::copy(a.join("IMG_0001.JPG"), a.join(&duplicate_copy)).unwrap();
    // Nomes Unicode e muito longos (240 bytes no nome, perto do limite de 255).
    let unicode = "Inês & João — brinde 💍 ação.jpg".to_string();
    write(
        &b.join(&unicode),
        &jpeg_bytes(&shot(900, false), Some(&exif_at(sony, "A7-0002", 20, 15, 0, 0)), 90),
    )
    .unwrap();
    moment_of.insert(unicode.clone(), "reception");
    let long = format!(
        "{}.jpg",
        "fotografia_do_casamento_".repeat(9).chars().take(232).collect::<String>()
    );
    write(
        &b.join(&long),
        &jpeg_bytes(&shot(901, false), Some(&exif_at(sony, "A7-0002", 20, 20, 0, 0)), 90),
    )
    .unwrap();
    moment_of.insert(long.clone(), "reception");
    // Ficheiros corrompidos: JPEG truncado e RAW com lixo.
    let ok = std::fs::read(a.join("IMG_0002.JPG")).unwrap();
    std::fs::write(a.join("IMG_9001.JPG"), &ok[..ok.len() / 3]).unwrap();
    let mut junk = b"II*\0\x08\0\0\0".to_vec();
    junk.extend((0..200_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8));
    std::fs::write(a.join("IMG_9002.CR3"), junk).unwrap();
    let corrupt = vec!["IMG_9001.JPG".to_string(), "IMG_9002.CR3".to_string()];
    // RAW reais (várias marcas) se houver amostras.
    let mut raws = Vec::new();
    if let Ok(d) = std::env::var("MOMENTO_SAMPLES_DIR") {
        let r = root.join("DCIM/102RAW__");
        for name in [
            "Canon EOS R3.cr3",
            "Nikon D810.nef",
            "Sony ILCE-7M4 (A7M4).ARW",
            "Fujifilm X-E1.raf",
            "Olympus E-M10.orf",
            "Leica T (Typ 701).dng",
        ] {
            let p = Path::new(&d).join(name);
            if p.is_file() {
                std::fs::create_dir_all(&r).unwrap();
                std::fs::copy(&p, r.join(name)).unwrap();
                raws.push(name.to_string());
            }
        }
    }
    // Pessoas reais se houver (MOMENTO_FACES_DIR).
    if let Ok(d) = std::env::var("MOMENTO_FACES_DIR") {
        let f = root.join("DCIM/103FACES");
        std::fs::create_dir_all(&f).unwrap();
        for e in std::fs::read_dir(d).unwrap().flatten().take(8) {
            std::fs::copy(e.path(), f.join(e.file_name())).unwrap();
        }
    }
    Card {
        root: root.to_path_buf(),
        moment_of,
        burst,
        sharp_in_burst,
        corrupt,
        duplicate_copy,
        unicode,
        long,
        raws,
    }
}

#[test]
fn real_wedding_end_to_end() {
    let t0 = Instant::now();
    let tmp = tempfile::tempdir().unwrap();
    let card = make_card(&tmp.path().join("CARTÃO SD"));
    let work = tmp.path().join("Trabalho Fotográfico 2026");
    let backup = tmp.path().join("Disco de Segurança ✓");
    let delivery = tmp.path().join("Entregas — Inês & João");
    let originals = tmp.path().join("Arquivo de Originais/2026/Inês & João");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(&backup).unwrap();
    let card_files = files(&card.root);
    let card_hashes: HashMap<PathBuf, String> =
        card_files.iter().map(|p| (p.clone(), blake(p))).collect();
    let n_card = card_files.len();
    eprintln!("cartão: {n_card} ficheiros ({} RAW reais)", card.raws.len());

    // ── Projeto ───────────────────────────────────────────────────────────────────────
    let paths = create_project(&NewProject {
        name: "Casamento Inês & João".into(),
        client_name: "Inês & João".into(),
        event_type: "wedding".into(),
        event_date: Some("2026-06-20".into()),
        location: "Quinta da Regaleira, Sintra".into(),
        originals_dir: Some(originals.to_string_lossy().into_owned()),
        work_dir: work.to_string_lossy().into_owned(),
    })
    .unwrap();
    let rec = Arc::new(Rec::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();

    // ── Importação: cópia verificada + disco de segurança ─────────────────────────────
    s.start_import_with(
        &ImportRequest {
            paths: vec![card.root.to_string_lossy().into_owned()],
            mode: ImportMode::Copy,
            recursive: true,
        },
        &ImportSafety { verify: true, backup_dir: Some(backup.to_string_lossy().into_owned()) },
    )
    .unwrap();
    s.wait_import();
    let sum = rec.summaries.lock().unwrap().last().cloned().expect("importação terminou");
    eprintln!("importação: {:?}", sum.counts);
    assert!(sum.error.is_none() && !sum.cancelled, "{sum:?}");
    // A cópia repetida é reconhecida pelo conteúdo e não entra duas vezes.
    assert_eq!(sum.counts.duplicate, 1, "{:?}", sum.issues);
    // O RAW com lixo é recusado com motivo; o JPEG truncado entra (decodificável em parte) ou é recusado — nunca um pânico.
    assert!(sum.issues.iter().any(|i| i.source_path.ends_with("IMG_9002.CR3")), "{:?}", sum.issues);
    let photos = s.list_photos().unwrap();
    let imported = sum.counts.imported as usize;
    assert_eq!(photos.len(), imported);
    assert!(imported >= n_card - 1 - card.corrupt.len(), "{imported} de {n_card}");
    let by_name = |n: &str| {
        photos.iter().find(|p| p.file_name == n).unwrap_or_else(|| panic!("{n} em falta"))
    };
    by_name(&card.unicode);
    by_name(&card.long);

    // Cartão intacto; cópias de trabalho e de segurança idênticas aos originais.
    for (p, h) in &card_hashes {
        assert_eq!(&blake(p), h, "original alterado: {}", p.display());
    }
    let index: HashMap<String, String> = card_hashes
        .iter()
        .map(|(p, h)| (h.clone(), p.file_name().unwrap().to_string_lossy().into_owned()))
        .collect();
    for dir in [originals.clone(), backup.clone()] {
        let copies: Vec<PathBuf> = files(&dir)
            .into_iter()
            .filter(|p| !p.file_name().unwrap().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(
            copies.len() >= imported,
            "{}: {} cópias para {imported}",
            dir.display(),
            copies.len()
        );
        for c in &copies {
            assert!(
                index.contains_key(&blake(c)),
                "cópia diferente de qualquer original: {}",
                c.display()
            );
        }
    }

    // ── RAW e miniaturas ──────────────────────────────────────────────────────────────
    wait_until("miniaturas", 600, || s.thumbnails_remaining() == 0);
    let photos = s.list_photos().unwrap();
    let failed: Vec<&str> = photos
        .iter()
        .filter(|p| p.thumb == ThumbState::Failed)
        .map(|p| p.file_name.as_str())
        .collect();
    assert!(
        failed.iter().all(|f| card.corrupt.iter().any(|c| c == f)),
        "miniaturas falhadas: {failed:?}"
    );
    let not_ready: Vec<(&str, ThumbState)> = photos
        .iter()
        .filter(|p| !card.corrupt.contains(&p.file_name) && p.thumb != ThumbState::Ready)
        .map(|p| (p.file_name.as_str(), p.thumb))
        .collect();
    assert!(not_ready.is_empty(), "miniaturas por fazer depois de a fila dizer 0: {not_ready:?}");
    for r in &card.raws {
        let p = by_name(r);
        let prev = s.serve_image(p.id.0, CacheLevel::Preview).unwrap();
        let img = image::open(&prev.path).unwrap();
        assert!(img.width() >= 1000, "{r}: preview {}×{}", img.width(), img.height());
        // O editor revela o RAW (render rápido pelo motor de revelação).
        let v = s.edit_view(p.id.0).unwrap();
        let f = s.render_edit(p.id.0, &v.settings, 1024, 1024, RenderQuality::Fast, false).unwrap();
        let (w, h) = (
            u32::from_le_bytes(f[0..4].try_into().unwrap()),
            u32::from_le_bytes(f[4..8].try_into().unwrap()),
        );
        assert!(w.max(h) == 1024 && w.min(h) > 300, "{r}: render {w}×{h}");
    }

    // ── Análise e seleção ─────────────────────────────────────────────────────────────
    let st = s.analysis_status().unwrap();
    let analysed = st.models_available;
    if analysed {
        let n = s.start_analysis(false).unwrap();
        s.wait_analysis();
        let st = s.analysis_status().unwrap();
        eprintln!("análise: {n} pedidas → {:?}", st.counts);
        assert_eq!(st.counts.pending, 0);
        assert!(st.counts.failed as usize <= card.corrupt.len(), "{:?}", st.counts);
        let groups = s.list_groups().unwrap();
        let sharp = by_name(&card.sharp_in_burst).id;
        let g = groups.iter().find(|g| g.members.contains(&sharp)).expect("rajada agrupada");
        assert!(g.members.len() >= 4, "{g:?}");
        assert_eq!(g.members[0], sharp, "a nítida da rajada é a melhor");
        let recs = s.list_recommendations().unwrap();
        let best: Vec<_> = recs.iter().filter(|r| r.label == "best").map(|r| r.asset).collect();
        assert!(!best.is_empty());
        assert!(s.accept_recommendations(&best).unwrap() > 0);
    } else {
        eprintln!("AVISO: modelos ausentes — análise não executada");
    }

    // ── Momentos ──────────────────────────────────────────────────────────────────────
    let ms = s.list_moments().unwrap();
    let mid = |k: &str| {
        ms.iter().find(|m| m.kind.as_deref() == Some(k)).unwrap_or_else(|| panic!("momento {k}")).id
    };
    for (name, k) in &card.moment_of {
        if let Some(p) = photos.iter().find(|p| &p.file_name == name) {
            s.assign_moment(&[p.asset], Some(mid(k))).unwrap();
        }
    }
    let ms = s.list_moments().unwrap();
    assert_eq!(ms.iter().find(|m| m.kind.as_deref() == Some("ceremony")).unwrap().count, 6);

    // ── Culling manual (revisão) ──────────────────────────────────────────────────────
    let good: Vec<_> = photos
        .iter()
        .filter(|p| {
            !card.corrupt.contains(&p.file_name)
                && !(card.burst.contains(&p.file_name) && p.file_name != card.sharp_in_burst)
        })
        .map(|p| p.asset)
        .collect();
    s.set_flag(&good, 1).unwrap();
    let blurred: Vec<_> = photos
        .iter()
        .filter(|p| card.burst.contains(&p.file_name) && p.file_name != card.sharp_in_burst)
        .map(|p| p.asset)
        .collect();
    s.set_flag(&blurred, -1).unwrap();
    s.set_stars(&[by_name(&card.sharp_in_burst).asset], 5).unwrap();

    // ── Edição, auto edit, preset ─────────────────────────────────────────────────────
    let hero = by_name("IMG_0012.JPG").id.0;
    let mut st = s.edit_view(hero).unwrap().settings;
    st.light.exposure = 0.7;
    s.commit_edit(hero, "exposure", &st).unwrap();
    st.light.contrast = 20.0;
    s.commit_edit(hero, "contrast", &st).unwrap();
    s.undo_edit(hero).unwrap();
    assert_eq!(s.edit_view(hero).unwrap().settings.light.contrast, 0.0, "desfazer");
    s.redo_edit(hero).unwrap();
    assert_eq!(s.edit_view(hero).unwrap().settings.light.contrast, 20.0, "refazer");

    let prep: Vec<i64> = photos
        .iter()
        .filter(|p| card.moment_of.get(&p.file_name) == Some(&"bride_prep"))
        .map(|p| p.id.0)
        .collect();
    s.start_auto_edit(
        prep.clone(),
        AutoOptions { intensity: Intensity::Natural, ..Default::default() },
    )
    .unwrap();
    s.wait_auto_edit();
    for k in &prep {
        assert!(s.edit_view(*k).unwrap().state.can_undo, "auto edit gravado em {k}");
    }
    let lib = Library::open(&paths.root.join("lib-presets.sqlite")).unwrap();
    let store = PresetStore { lib: &lib, lut_dir: paths.root.join("luts") };
    let film = store.get("builtin:film-warm").unwrap();
    let party: Vec<i64> = photos
        .iter()
        .filter(|p| card.moment_of.get(&p.file_name) == Some(&"party"))
        .map(|p| p.id.0)
        .collect();
    assert_eq!(
        s.apply_preset(&party, &film.name, &film.settings, &film.components, 100.0).unwrap(),
        party.len()
    );
    let party_settings = s.edit_view(party[0]).unwrap().settings;

    // ── Reinício da aplicação ─────────────────────────────────────────────────────────
    let before = s.list_photos().unwrap();
    s.close();
    drop(s);
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    let after = s.list_photos().unwrap();
    assert_eq!(before.len(), after.len());
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(
            (a.id, a.stars, a.flag, a.edited),
            (b.id, b.stars, b.flag, b.edited),
            "{}",
            a.file_name
        );
    }
    assert_eq!(s.edit_view(hero).unwrap().settings.light.exposure, 0.7);
    assert_eq!(s.edit_view(party[0]).unwrap().settings, party_settings);
    assert_eq!(
        s.list_moments()
            .unwrap()
            .iter()
            .find(|m| m.kind.as_deref() == Some("ceremony"))
            .unwrap()
            .count,
        6
    );
    let v = s.validate_catalog().unwrap();
    assert!(v.ok(), "{v:?}");

    // ── Exportação: plano "Exportar casamento" das favoritas ──────────────────────────
    let picks: Vec<i64> = after.iter().filter(|p| p.flag == 1).map(|p| p.id.0).collect();
    let plan = wedding_plan();
    let req = ExportRequest {
        keys: picks.clone(),
        recipes: plan.recipes.clone(),
        destination: delivery.to_string_lossy().into_owned(),
        name: plan.name.clone(),
        ..Default::default()
    };
    let est = s.estimate_export(&req).unwrap();
    assert!(est.errors.is_empty() && est.enough_space, "{est:?}");
    let done = wait_job(&s, s.start_export(req).unwrap());
    assert_eq!(done.state, JobState::Done, "{:?}", done.errors);
    assert_eq!(done.files_failed, 0, "{:?}", done.errors);
    assert_eq!(done.files_done as usize, picks.len() * plan.recipes.len());
    let out = files(&delivery);
    assert_eq!(out.len(), picks.len() * plan.recipes.len());
    // Resolução total = dimensões da fotografia; edição aplicada (+0,7 EV mais clara).
    let hero_row = after.iter().find(|p| p.id.0 == hero).unwrap();
    let hi = delivery.join("Entrega/AltaResolucao/IMG_0012.jpg");
    let img = image::open(&hi).unwrap().to_rgb8();
    assert_eq!(img.dimensions(), (hero_row.width, hero_row.height));
    let orig =
        image::open(files(&originals).into_iter().find(|p| p.ends_with("IMG_0012.JPG")).unwrap())
            .unwrap()
            .to_rgb8();
    let mean = |i: &RgbImage| {
        i.pixels().map(|p| p[1] as f64).sum::<f64>() / (i.width() * i.height()) as f64
    };
    assert!(
        mean(&img) > mean(&orig) + 8.0,
        "edição na exportação: {} vs {}",
        mean(&img),
        mean(&orig)
    );
    // Nomes Unicode/longos exportados sem colisões nem truncagens perigosas.
    assert!(out.iter().any(|p| p
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("Inês & João")));

    // ── Galeria ───────────────────────────────────────────────────────────────────────
    let mut cfg = s.gallery_config().unwrap();
    cfg.settings.selection = Selection::Picks;
    cfg.destination = delivery.join("Galeria do cliente").to_string_lossy().into_owned();
    s.save_gallery_config(&cfg).unwrap();
    let names = HashMap::new();
    let pv = s.gallery_preview(&cfg, &names).unwrap();
    assert_eq!(pv.photos as usize, picks.len(), "{pv:?}");
    let g = wait_job(&s, s.start_gallery(cfg, names, false).unwrap());
    assert_eq!(g.state, JobState::Done, "{:?}", g.errors);
    let index = PathBuf::from(g.index.unwrap());
    assert!(index.is_file());
    let m = momento_gallery::site::read_manifest(index.parent().unwrap(), None).unwrap();
    assert_eq!(m["photos"].as_array().unwrap().len(), picks.len());
    let html = std::fs::read_to_string(&index).unwrap();
    assert!(!html.contains(&*work.to_string_lossy()), "caminhos locais na galeria");

    // ── Álbum ─────────────────────────────────────────────────────────────────────────
    let spec = AlbumSpec {
        width_mm: 300.0,
        height_mm: 300.0,
        pages: 20,
        dpi: 150,
        format: "30x30".into(),
        confirmed: true,
        ..Default::default()
    };
    let a = s.create_album("Inês & João", spec).unwrap();
    let r = s.auto_album(&a.id, Default::default()).unwrap();
    assert!(r.used > 10, "{} fotografias no álbum", r.used);
    let used: Vec<i64> = r.album.photos();
    assert!(!used.iter().any(|k| blurred.iter().any(|b| b.0 == *k)), "tremidas fora do álbum");
    let issues = s.album_preflight(&r.album).unwrap();
    assert!(
        !issues.iter().any(|i| i.severity == momento_album::preflight::Severity::Error),
        "{issues:?}"
    );
    let album_dir = delivery.join("Álbum");
    let j = wait_job(
        &s,
        s.start_album_export(&a.id, ExportKind::PrintPdf, album_dir.to_str().unwrap()).unwrap(),
    );
    assert_eq!(j.state, JobState::Done, "{:?}", j.errors);
    let pdf = std::fs::read(PathBuf::from(j.index.unwrap())).unwrap();
    assert!(String::from_utf8_lossy(&pdf).contains("/TrimBox"));

    // ── Fim: nada perdido, nada estragado ─────────────────────────────────────────────
    for (p, h) in &card_hashes {
        assert_eq!(&blake(p), h, "original alterado no fim: {}", p.display());
    }
    let v = s.validate_catalog().unwrap();
    assert!(v.ok(), "{v:?}");
    let leftovers: Vec<PathBuf> = files(&work)
        .into_iter()
        .chain(files(&delivery))
        .filter(|p| p.extension().is_some_and(|e| e == "tmp" || e == "part"))
        .collect();
    assert!(leftovers.is_empty(), "temporários: {leftovers:?}");
    s.close();
    eprintln!(
        "casamento completo em {:.1?} ({} fotografias, {} exportadas)",
        t0.elapsed(),
        after.len(),
        picks.len()
    );
}
