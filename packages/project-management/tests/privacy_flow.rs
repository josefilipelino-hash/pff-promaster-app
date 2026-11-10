//! Auditoria final — ações de privacidade e de limpeza, pelo caminho real da aplicação:
//! limpar a cache, apagar dados de pessoas, cópia virtual no lixo, restaurar um catálogo
//! (cache invalidada) e eliminar o projeto (para o Lixo, sem tocar nos originais).

use momento_database::import::ImportMode;
use momento_import_engine::ImportProgress;
use momento_project_management::paths::CacheLevel;
use momento_project_management::thumbnails::ThumbBatch;
use momento_project_management::*;
use momento_test_fixtures::*;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Rec;
impl SessionEvents for Rec {
    fn import_progress(&self, _: &ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &ThumbBatch) {}
}

fn wait_thumbs(s: &ProjectSession) {
    let t = Instant::now();
    while s.thumbnails_remaining() > 0 {
        assert!(t.elapsed() < Duration::from_secs(60), "miniaturas não terminaram");
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn thumbs_on_disk(root: &Path) -> usize {
    walkdir::WalkDir::new(root.join("cache/thumbs"))
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file() && e.file_name().to_string_lossy().ends_with(".jpg"))
        .count()
}

#[test]
fn privacy_and_cleanup_actions() {
    let tmp = tempfile::tempdir().unwrap();
    // Lixo do sistema dentro da pasta temporária (Linux: XDG); nada vai para o Lixo real.
    std::env::set_var("XDG_DATA_HOME", tmp.path().join("xdg"));
    let src = tmp.path().join("cartao");
    let work = tmp.path().join("trabalho");
    std::fs::create_dir_all(&work).unwrap();
    let originals: Vec<_> = (0..3)
        .map(|i| {
            let p = src.join(format!("IMG_{i}.jpg"));
            write(&p, &jpeg_bytes(&test_image(900, 600, i, 1), Some(&spec_for(i, 1)), 90)).unwrap();
            p
        })
        .collect();
    let hashes: Vec<_> = originals.iter().map(|p| std::fs::read(p).unwrap()).collect();
    let paths = create_project(&NewProject {
        name: "Privacidade".into(),
        client_name: "Cliente".into(),
        event_type: "wedding".into(),
        event_date: None,
        location: String::new(),
        originals_dir: None,
        work_dir: work.to_string_lossy().into_owned(),
    })
    .unwrap();
    let s = ProjectSession::open(&paths.root, Arc::new(Rec)).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![src.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    wait_thumbs(&s);
    assert_eq!(thumbs_on_disk(&paths.root), 3);

    // Limpar cache: miniaturas apagadas e geradas de novo; catálogo intacto.
    let c = s.clear_cache().unwrap();
    assert!(c.files >= 3 && c.bytes > 0, "{c:?}");
    wait_thumbs(&s);
    assert_eq!(thumbs_on_disk(&paths.root), 3, "miniaturas regeneradas");
    assert_eq!(s.list_photos().unwrap().len(), 3);

    // Apagar dados de pessoas: sem pessoas.
    s.clear_people().unwrap();
    assert!(s.list_persons().unwrap().is_empty());

    // Cópia virtual enviada para o lixo: só a cópia sai; o ficheiro fica.
    let master = s.list_photos().unwrap()[0].id.0;
    let vc = s.create_virtual_copy(master).unwrap();
    assert_eq!(s.list_photos().unwrap().len(), 4);
    let r = s.move_to_trash(&[vc], true).unwrap();
    assert_eq!((r.removed, r.trashed), (1, 0), "{r:?}");
    assert_eq!(s.list_photos().unwrap().len(), 3);
    assert!(originals.iter().all(|p| p.is_file()), "uma cópia virtual nunca leva o original");

    // Restaurar uma versão do catálogo: a cache (indexada por id) é invalidada.
    let b = s.create_backup().unwrap();
    let thumb0 = paths.cached(CacheLevel::Thumb, s.list_photos().unwrap()[0].asset);
    assert!(thumb0.is_file());
    s.close();
    drop(s);
    momento_project_management::safety::replace_catalog(&paths, Path::new(&b.file)).unwrap();
    assert!(!thumb0.exists(), "cache antiga apagada depois do restauro");
    let s = ProjectSession::open(&paths.root, Arc::new(Rec)).unwrap();
    wait_thumbs(&s);
    assert_eq!(thumbs_on_disk(&paths.root), 3, "miniaturas regeneradas depois do restauro");

    // Eliminar o projeto aberto: recusado (em uso).
    assert!(momento_project_management::safety::delete_project(&paths.root).is_err());
    s.close();
    drop(s);
    // Fechado: vai para o Lixo; os originais ficam intactos.
    momento_project_management::safety::delete_project(&paths.root).unwrap();
    assert!(!paths.root.exists(), "pasta do projeto movida para o Lixo");
    for (p, h) in originals.iter().zip(&hashes) {
        assert_eq!(&std::fs::read(p).unwrap(), h, "{p:?}");
    }

    // Um projeto com originais lá dentro nunca é eliminado.
    let inside = work.join("Dentro");
    std::fs::create_dir_all(&inside).unwrap();
    let paths2 = create_project(&NewProject {
        name: "Dentro".into(),
        client_name: "Cliente".into(),
        event_type: "wedding".into(),
        event_date: None,
        location: String::new(),
        originals_dir: None,
        work_dir: inside.to_string_lossy().into_owned(),
    })
    .unwrap();
    let own = paths2.root.join("fotos");
    write(&own.join("IMG_9.jpg"), &jpeg_bytes(&test_image(300, 200, 9, 1), None, 90)).unwrap();
    let s2 = ProjectSession::open(&paths2.root, Arc::new(Rec)).unwrap();
    s2.start_import(&ImportRequest {
        paths: vec![own.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s2.wait_import();
    s2.close();
    drop(s2);
    let e = momento_project_management::safety::delete_project(&paths2.root).unwrap_err();
    assert!(e.to_string().contains("originais dentro"), "{e}");
    assert!(own.join("IMG_9.jpg").is_file() && paths2.root.is_dir());
}
