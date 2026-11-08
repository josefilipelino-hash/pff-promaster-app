//! Fluxo completo da Fase 1 com centenas de fotografias sintéticas:
//! criar projeto → importar → miniaturas → classificar → fechar → reabrir → verificar.

use momento_core_types::AssetId;
use momento_database::import::ImportMode;
use momento_import_engine::ImportProgress;
use momento_project_management::paths::CacheLevel;
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Recorder {
    state: Mutex<(Vec<ImportProgress>, Vec<ImportSummary>, u64, u64)>,
    cv: Condvar,
}

impl SessionEvents for Recorder {
    fn import_progress(&self, p: &ImportProgress) {
        self.state.lock().unwrap().0.push(p.clone());
    }
    fn import_finished(&self, s: &ImportSummary) {
        self.state.lock().unwrap().1.push(s.clone());
        self.cv.notify_all();
    }
    fn thumbnails(&self, b: &ThumbBatch) {
        let mut st = self.state.lock().unwrap();
        st.2 += b.ready.len() as u64;
        st.3 += b.failed.len() as u64;
        self.cv.notify_all();
    }
}

impl Recorder {
    fn wait_thumbs(&self, expected: u64, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let mut st = self.state.lock().unwrap();
        while st.2 + st.3 < expected {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "miniaturas: {} prontas, {} falhadas de {expected}",
                st.2,
                st.3
            );
            st = self.cv.wait_timeout(st, left).unwrap().0;
        }
    }
}

fn generate(dir: &Path, n: u32, w: u32) {
    (0..n).into_par_iter().for_each(|i| {
        let o = [1u16, 1, 6, 1, 8, 3][(i % 6) as usize];
        let sub = format!("cartao{}/DCIM/{}", i % 2, 100 + i % 3);
        let p = dir.join(sub).join(format!("IMG_{i:05}.jpg"));
        write(&p, &jpeg_bytes(&test_image(w, w * 2 / 3, i, o), Some(&spec_for(i, o)), 85)).unwrap();
    });
    for i in 0..10u32 {
        write(
            &dir.join(format!("extra/PNG_{i}.png")),
            &png_bytes(&test_image(300, 200, 500 + i, 1), Some(&spec_for(i, 1))),
        )
        .unwrap();
        write(
            &dir.join(format!("extra/TIF_{i}.tif")),
            &tiff_bytes(&test_image(300, 200, 600 + i, 1)),
        )
        .unwrap();
    }
    write(&dir.join("extra/TIF16.tiff"), &tiff16_bytes(300, 200)).unwrap();
    // duplicados e inválidos
    std::fs::copy(dir.join("cartao0/DCIM/100/IMG_00000.jpg"), dir.join("extra/DUP_A.jpg")).unwrap();
    std::fs::copy(dir.join("cartao1/DCIM/101/IMG_00001.jpg"), dir.join("extra/DUP_B.jpg")).unwrap();
    let good = jpeg_bytes(&test_image(400, 300, 7, 1), None, 80);
    write(&dir.join("extra/truncado.jpg"), &good[..good.len() / 4]).unwrap();
    write(&dir.join("extra/texto.jpg"), b"nao e imagem").unwrap();
}

fn new_project(work: &Path, originals: &Path) -> NewProject {
    NewProject {
        name: "Casamento Ana & Pedro".into(),
        client_name: "Ana & Pedro".into(),
        event_type: "wedding".into(),
        event_date: Some("2026-06-15".into()),
        location: "Quinta da Regaleira, Sintra".into(),
        originals_dir: Some(originals.to_string_lossy().into_owned()),
        work_dir: work.to_string_lossy().into_owned(),
    }
}

#[test]
fn create_import_rate_close_reopen_with_hundreds_of_photos() {
    const N: u32 = 600;
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let t = Instant::now();
    generate(&source, N, 1200);
    eprintln!("geração de {} ficheiros: {:.2?}", N + 25, t.elapsed());

    // 1. Criar projeto
    let paths = create_project(&new_project(&work, &tmp.path().join("originais"))).unwrap();
    assert_eq!(paths.root, work.join("2026-06-15_Ana_e_Pedro.momento"));
    assert!(
        create_project(&new_project(&work, &tmp.path().join("originais"))).is_err(),
        "não sobrescreve"
    );

    // 2. Abrir e importar a pasta (recursiva)
    let rec = Arc::new(Recorder::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    assert_eq!(s.info().unwrap().record.client_name, "Ana & Pedro");
    let t = Instant::now();
    s.start_import(&ImportRequest {
        paths: vec![source.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: true,
    })
    .unwrap();
    s.wait_import();
    let import_time = t.elapsed();
    let summary = rec.state.lock().unwrap().1.last().cloned().unwrap();
    let c = &summary.counts;
    eprintln!("importação: {import_time:.2?} — {c:?}");
    assert_eq!(c.imported, N as u64 + 22, "JPEG + 10 PNG + 11 TIFF + truncado (cabeçalho válido)");
    assert_eq!(c.duplicate, 2);
    assert_eq!(c.unsupported, 1);
    assert_eq!(c.error, 0);
    assert!(!rec.state.lock().unwrap().0.is_empty(), "eventos de progresso emitidos");

    // 3. Miniaturas em segundo plano
    let t = Instant::now();
    rec.wait_thumbs(N as u64 + 22, Duration::from_secs(300));
    eprintln!("miniaturas: {:.2?} após o fim da importação", t.elapsed());
    let failed = rec.state.lock().unwrap().3;
    assert_eq!(failed, 1, "apenas o JPEG truncado falha");

    let photos = s.list_photos().unwrap();
    assert_eq!(photos.len(), N as usize + 22);
    // Ordenadas por data de captura.
    let times: Vec<_> = photos.iter().filter_map(|p| p.capture_time.clone()).collect();
    assert!(times.windows(2).all(|w| w[0] <= w[1]));
    for p in &photos {
        if p.file_name != "truncado.jpg" {
            assert!(paths.cached(CacheLevel::Thumb, p.id).is_file(), "{}", p.file_name);
        }
    }
    // Orientação 6: o sensor gravou 800×1200; a imagem visual é 1200×800 com a barra
    // vermelha no topo.
    let rotated = photos.iter().find(|p| p.file_name == "IMG_00002.jpg").unwrap();
    assert_eq!((rotated.width, rotated.height), (1200, 800));
    let thumb = image::open(paths.cached(CacheLevel::Thumb, rotated.id)).unwrap().to_rgb8();
    assert_eq!(thumb.dimensions(), (640, 427));
    let top = thumb.get_pixel(320, 4).0;
    assert!(top[0] > 170 && top[2] < 90, "barra vermelha no topo: {top:?}");

    // Preview e resolução total a pedido.
    let pv = previews::ensure(&s.paths, s.registry(), rotated.id, CacheLevel::Preview).unwrap();
    assert!(pv.path.starts_with(&paths.root));
    let full = previews::ensure(&s.paths, s.registry(), rotated.id, CacheLevel::Full).unwrap();
    assert!(full.path.starts_with(&source), "JPEG: serve o original");
    let tif = photos.iter().find(|p| p.file_name == "TIF_0.tif").unwrap();
    let full_tif = previews::ensure(&s.paths, s.registry(), tif.id, CacheLevel::Full).unwrap();
    assert!(full_tif.path.starts_with(&paths.root), "TIFF: JPEG gerado em cache");

    // 4. Classificar
    let ids: Vec<AssetId> = photos.iter().map(|p| p.id).collect();
    s.set_stars(&ids[0..50], 3).unwrap();
    s.set_stars(&ids[10..20], 5).unwrap();
    s.set_flag(&ids[0..30], 1).unwrap();
    s.set_flag(&ids[100..110], -1).unwrap();
    s.set_color(&ids[200..205], Some("green")).unwrap();
    s.set_color(&ids[300..301], Some("purple")).unwrap();
    s.save_ui_state(r#"{"filter":"picks","thumbSize":240,"focusId":12}"#).unwrap();
    let detail = s.photo_details(rotated.id).unwrap().unwrap();
    assert_eq!(detail.camera_model.as_deref(), Some("SONY").and(Some("ILCE-7M4")));
    assert_eq!(detail.orientation, 6);

    // 5. Fechar
    s.close();
    drop(s);

    // 6. Reabrir e verificar que está tudo igual
    let s2 = ProjectSession::open(&paths.root, Arc::new(Recorder::default())).unwrap();
    let again = s2.list_photos().unwrap();
    assert_eq!(again.len(), photos.len());
    let by_id = |id: AssetId| again.iter().find(|p| p.id == id).unwrap();
    assert_eq!(by_id(ids[0]).stars, 3);
    assert_eq!(by_id(ids[15]).stars, 5);
    assert_eq!(by_id(ids[25]).flag, 1);
    assert_eq!(by_id(ids[105]).flag, -1);
    assert_eq!(by_id(ids[202]).color.as_deref(), Some("green"));
    assert_eq!(by_id(ids[300]).color.as_deref(), Some("purple"));
    assert_eq!(by_id(ids[400]).stars, 0);
    let info = s2.info().unwrap();
    assert_eq!(
        info.ui_state.as_deref(),
        Some(r#"{"filter":"picks","thumbSize":240,"focusId":12}"#)
    );
    assert_eq!(info.photo_count, N as u64 + 22);
    assert_eq!(info.record.status, "imported");
    assert_eq!(s2.thumbnails_remaining(), 0, "nenhuma miniatura por gerar ao reabrir");
    // Cópias de segurança do catálogo
    assert!(std::fs::read_dir(paths.backups()).unwrap().count() >= 1);
    s2.close();
}

#[test]
fn import_interrupted_by_close_resumes_on_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("fonte");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    generate(&source, 300, 1600);
    let paths = create_project(&new_project(&work, &tmp.path().join("orig"))).unwrap();

    let rec = Arc::new(Recorder::default());
    let s = ProjectSession::open(&paths.root, rec.clone()).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![source.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: true,
    })
    .unwrap();
    // Fecha assim que houver progresso (como fechar a aplicação a meio).
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if rec.state.lock().unwrap().0.iter().any(|p| p.processed > 0) || Instant::now() > deadline
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    s.close();
    let first = rec.state.lock().unwrap().1.last().cloned().unwrap();
    drop(s);
    eprintln!("após fecho: {:?}", first.counts);

    let rec2 = Arc::new(Recorder::default());
    let s2 = ProjectSession::open(&paths.root, rec2.clone()).unwrap();
    s2.wait_import();
    let total = s2.list_photos().unwrap().len();
    if first.counts.pending > 0 {
        assert!(first.suspended);
        let resumed = rec2.state.lock().unwrap().1.last().cloned().expect("importação retomada");
        assert_eq!(resumed.counts.pending, 0);
    }
    assert_eq!(total, 300 + 22);
    s2.close();
}

#[test]
fn copy_mode_puts_files_in_originals_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("cartao");
    let work = tmp.path().join("trabalho");
    let originals = tmp.path().join("Originais");
    std::fs::create_dir_all(&work).unwrap();
    generate(&source, 24, 400);
    let paths = create_project(&new_project(&work, &originals)).unwrap();
    let s = ProjectSession::open(&paths.root, Arc::new(Recorder::default())).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![source.to_string_lossy().into_owned()],
        mode: ImportMode::Copy,
        recursive: true,
    })
    .unwrap();
    s.wait_import();
    let photos = s.list_photos().unwrap();
    assert_eq!(photos.len(), 24 + 22);
    for p in &photos {
        let d = s.photo_details(p.id).unwrap().unwrap();
        assert!(PathBuf::from(&d.path).starts_with(&originals), "{}", d.path);
    }
    s.close();
}

#[test]
fn open_rejects_non_project_folders() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(ProjectSession::open(tmp.path(), Arc::new(Recorder::default())).is_err());
    let bad = NewProject {
        work_dir: tmp.path().join("nao-existe").to_string_lossy().into_owned(),
        ..new_project(tmp.path(), tmp.path())
    };
    assert!(create_project(&bad).is_err());
}
