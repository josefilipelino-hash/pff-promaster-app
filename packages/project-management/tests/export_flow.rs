//! Fase 9 — exportação pelo caminho real da aplicação: plano "Exportar casamento"
//! (várias receitas numa operação), originais intactos, edições aplicadas, GPS removido,
//! nomes sem colisões, fila com pausa/retoma e cancelamento.
//! Com `MOMENTO_SAMPLES_DIR`, exporta também um RAW real em resolução total.

use momento_database::import::ImportMode;
use momento_export_engine::presets::{builtin, wedding_plan};
use momento_export_engine::{Format, Recipe, Size, SizeMode};
use momento_import_engine::ImportProgress;
use momento_project_management::export::{ExportProgress, ExportRequest, JobState};
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Rec {
    exports: Mutex<Vec<ExportProgress>>,
}
impl SessionEvents for Rec {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &ThumbBatch) {}
    fn export_progress(&self, p: &ExportProgress) {
        self.exports.lock().unwrap().push(p.clone());
    }
}

fn hash(p: &Path) -> u64 {
    let d = std::fs::read(p).unwrap();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&d, &mut h);
    std::hash::Hasher::finish(&h)
}

fn wait_job(s: &ProjectSession, id: u64) -> ExportProgress {
    let t = Instant::now();
    loop {
        let p = s.export_jobs().into_iter().find(|p| p.id == id).unwrap();
        if matches!(p.state, JobState::Done | JobState::Cancelled | JobState::Failed) {
            return p;
        }
        assert!(t.elapsed() < Duration::from_secs(600), "exportação não terminou: {p:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
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

fn raw_sample() -> Option<PathBuf> {
    let d = PathBuf::from(std::env::var("MOMENTO_SAMPLES_DIR").ok()?);
    let p = d.join("Canon EOS 7D.cr2");
    p.is_file().then_some(p)
}

#[test]
fn wedding_export_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    let dest = tmp.path().join("entrega");
    std::fs::create_dir_all(&work).unwrap();
    for i in 0..3 {
        let spec = ExifSpec { gps: Some((38.79, -9.39)), ..spec_for(i, 1) };
        write(
            &src.join(format!("IMG_{i}.jpg")),
            &jpeg_bytes(&test_image(1800, 1200, i, 1), Some(&spec), 92),
        )
        .unwrap();
    }
    let originals: Vec<(PathBuf, u64)> = files(&src)
        .into_iter()
        .map(|p| {
            let h = hash(&p);
            (p, h)
        })
        .collect();
    let paths = create_project(&NewProject {
        name: "Casamento Ana & Pedro".into(),
        client_name: "Ana e Pedro".into(),
        event_type: "wedding".into(),
        event_date: Some("2026-06-15".into()),
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
    let keys: Vec<i64> = photos.iter().map(|p| p.id.0).collect();
    let edited = photos.iter().find(|p| p.file_name == "IMG_0.jpg").unwrap().id.0;
    // Edição: +1 EV numa fotografia.
    let mut st = s.edit_view(edited).unwrap().settings;
    st.light.exposure = 1.0;
    s.commit_edit(edited, "exposure", &st).unwrap();

    // Estimativa do plano completo.
    let plan = wedding_plan();
    let req = ExportRequest {
        keys: keys.clone(),
        recipes: plan.recipes.clone(),
        destination: dest.to_string_lossy().into_owned(),
        name: plan.name.clone(),
        ..Default::default()
    };
    let est = s.estimate_export(&req).unwrap();
    assert_eq!((est.photos, est.files), (3, 15));
    assert!(est.enough_space && est.free_bytes.unwrap() > 0 && est.errors.is_empty(), "{est:?}");

    // Uma operação → 5 pastas.
    let id = s.start_export(req.clone()).unwrap();
    let done = wait_job(&s, id);
    assert_eq!(done.state, JobState::Done, "{:?}", done.errors);
    assert_eq!((done.files_done, done.files_failed), (15, 0));
    let out = files(&dest);
    assert_eq!(out.len(), 15);
    for folder in ["Entrega/AltaResolucao", "Galeria", "Instagram", "Impressao", "Arquivo"] {
        let n = out.iter().filter(|p| p.parent().unwrap() == dest.join(folder)).count();
        assert_eq!(n, 3, "{folder}");
    }
    let real: u64 = out.iter().map(|p| std::fs::metadata(p).unwrap().len()).sum();
    assert_eq!(done.bytes_written, real);
    // Imagens sintéticas comprimem muito mais do que fotografias: a estimativa (feita para
    // fotografias reais) pode sobrestimar — nunca subestimar, para proteger o espaço.
    assert!(est.bytes >= real, "estimativa {} < real {real}", est.bytes);
    assert_eq!(done.bytes_estimated, real, "no fim, a estimativa é o real");
    assert!(rec.exports.lock().unwrap().iter().any(|p| p.state == JobState::Running));

    // Conteúdo: resolução total, Instagram 4:5, TIFF 16 bits Adobe RGB, GPS removido.
    let hi = dest.join("Entrega/AltaResolucao/IMG_0.jpg");
    let img = image::open(&hi).unwrap().to_rgb8();
    assert_eq!(img.dimensions(), (1800, 1200));
    let ig = image::open(dest.join("Instagram/IMG_1.jpg")).unwrap();
    assert_eq!((ig.width(), ig.height()), (1080, 1350));
    let tif = image::open(dest.join("Arquivo/IMG_2.tif")).unwrap();
    assert_eq!(tif.color(), image::ColorType::Rgb16);
    let ex = exif::Reader::new()
        .read_from_container(&mut std::io::BufReader::new(std::fs::File::open(&hi).unwrap()))
        .unwrap();
    assert!(ex.get_field(exif::Tag::Model, exif::In::PRIMARY).is_some(), "câmara mantida");
    assert!(ex.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY).is_none(), "GPS removido");
    let ex = exif::Reader::new().read_from_container(&mut std::io::BufReader::new(
        std::fs::File::open(dest.join("Arquivo/IMG_0.tif")).unwrap(),
    ));
    // TIFF: metadados básicos nas etiquetas e XMP (o arquivo mantém o GPS no catálogo, não no TIFF).
    assert!(ex.is_ok());
    // A edição foi aplicada: IMG_0 (+1 EV) mais clara que o original.
    let orig = image::open(&originals[0].0).unwrap().to_rgb8();
    let mean = |i: &image::RgbImage| {
        i.pixels().map(|p| p[1] as f64).sum::<f64>() / (i.width() * i.height()) as f64
    };
    assert!(mean(&img) > mean(&orig) + 20.0, "exposição exportada");

    // Segunda exportação para o mesmo sítio: renomeia em vez de sobrescrever.
    let gal = builtin().into_iter().find(|r| r.id == "builtin:galeria").unwrap();
    let id = s
        .start_export(ExportRequest {
            recipes: vec![gal.clone()],
            keys: vec![edited],
            ..req.clone()
        })
        .unwrap();
    assert_eq!(wait_job(&s, id).files_done, 1);
    assert!(dest.join("Galeria/IMG_0-2.jpg").is_file());

    // Pausa e retoma; cancelamento.
    let many: Vec<Recipe> = (0..6)
        .map(|i| Recipe {
            name: format!("r{i}"),
            format: Format::Png,
            size: Size { mode: SizeMode::LongEdge, a: 1500.0, ..Default::default() },
            naming: momento_export_engine::Naming {
                subfolder: format!("p{i}"),
                ..Default::default()
            },
            ..Default::default()
        })
        .collect();
    let id = s.start_export(ExportRequest { recipes: many.clone(), ..req.clone() }).unwrap();
    assert!(s.pause_export(id, true));
    std::thread::sleep(Duration::from_millis(600));
    let p = s.export_jobs().into_iter().find(|p| p.id == id).unwrap();
    assert!(matches!(p.state, JobState::Paused | JobState::Queued), "{:?}", p.state);
    let before = p.files_done;
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(
        s.export_jobs().into_iter().find(|p| p.id == id).unwrap().files_done,
        before,
        "pausada não avança"
    );
    assert!(s.pause_export(id, false));
    assert_eq!(wait_job(&s, id).files_done, 18);
    let id = s.start_export(ExportRequest { recipes: many, ..req.clone() }).unwrap();
    s.cancel_export(id);
    let c = wait_job(&s, id);
    assert_eq!(c.state, JobState::Cancelled);
    assert!(c.files_done < 18);
    s.clear_finished_exports();
    assert!(s.export_jobs().is_empty());

    // Erros claros: receita indisponível e ICC inexistente.
    let bad = Recipe { name: "DNG".into(), format: Format::Dng, ..Default::default() };
    let e =
        s.estimate_export(&ExportRequest { recipes: vec![bad.clone()], ..req.clone() }).unwrap();
    assert_eq!(e.errors.len(), 1);
    let id = s.start_export(ExportRequest { recipes: vec![bad], ..req.clone() }).unwrap();
    let f = wait_job(&s, id);
    assert_eq!(f.state, JobState::Failed);
    assert!(!f.errors.is_empty());

    // Originais nunca tocados.
    for (p, h) in &originals {
        assert_eq!(hash(p), *h, "{p:?}");
    }
    s.close();
}

/// RAW real: a exportação usa os dados do sensor em resolução total (não a preview
/// embutida nem a fonte reduzida do editor).
#[test]
fn raw_export_uses_full_resolution_original() {
    let Some(raw) = raw_sample() else {
        eprintln!("MOMENTO_SAMPLES_DIR sem 'Canon EOS 7D.cr2': teste ignorado");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    let dest = tmp.path().join("saida");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    let copy = src.join(raw.file_name().unwrap());
    std::fs::copy(&raw, &copy).unwrap();
    let before = hash(&copy);
    let paths = create_project(&NewProject {
        name: "RAW".into(),
        client_name: "-".into(),
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
    let p = &s.list_photos().unwrap()[0];
    let recipe = Recipe {
        name: "Total".into(),
        format: Format::Tiff,
        depth: momento_export_engine::BitDepth::B16,
        compression: momento_export_engine::Compression::Zip,
        ..Default::default()
    };
    let jpeg = builtin().into_iter().find(|r| r.id == "builtin:alta-resolucao").unwrap();
    let req = ExportRequest {
        keys: vec![p.id.0],
        recipes: vec![recipe, jpeg],
        destination: dest.to_string_lossy().into_owned(),
        name: "RAW".into(),
        ..Default::default()
    };
    let est = s.estimate_export(&req).unwrap();
    let t = Instant::now();
    let id = s.start_export(req).unwrap();
    let done = wait_job(&s, id);
    eprintln!("RAW {}×{} → TIFF 16 bits: {:.1?}", p.width, p.height, t.elapsed());
    assert_eq!(done.state, JobState::Done, "{:?}", done.errors);
    let out = files(&dest);
    let tif = out.iter().find(|p| p.extension().unwrap() == "tif").unwrap();
    // Estimativa de tamanho numa fotografia real (TIFF 16 bits ZIP + JPEG q95).
    let real = done.bytes_written as f64;
    eprintln!("estimado {} MB, real {:.1} MB", est.bytes / 1_000_000, real / 1e6);
    assert!(
        est.bytes as f64 / real > 0.5 && (est.bytes as f64) / real < 2.0,
        "estimativa {} vs {real}",
        est.bytes
    );
    let img = image::open(tif).unwrap();
    assert_eq!(img.color(), image::ColorType::Rgb16);
    // 7D: 5184×3456 (sensor); a preview embutida e a fonte do editor são menores.
    assert!(img.width().max(img.height()) >= 5000, "{}×{}", img.width(), img.height());
    assert!(
        (img.width() as i64 - p.width as i64).abs() <= 64
            && (img.height() as i64 - p.height as i64).abs() <= 64,
        "{}×{} vs catálogo {}×{}",
        img.width(),
        img.height(),
        p.width,
        p.height
    );
    // Gradações finas de 16 bits (não 8 bits ampliados).
    let v = img.to_rgb16();
    let distinct: std::collections::HashSet<u16> = v.pixels().take(200_000).map(|p| p[1]).collect();
    assert!(distinct.len() > 2000, "níveis distintos: {}", distinct.len());
    assert_eq!(hash(&copy), before, "RAW original intacto");
    s.close();
}
