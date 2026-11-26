//! Prepara um projeto para testar a aplicação real: importa uma pasta (por referência),
//! aplica a edição automática à primeira fotografia e regista o projeto nos recentes.
//! Uso: `real_setup <fotos> <pasta-de-trabalho> <library.db>`
use momento_database::import::ImportMode;
use momento_database::library::{Library, RecentProject};
use momento_image_processing::develop::auto::AutoOptions;
use momento_project_management::*;
use std::sync::Arc;

struct Quiet;
impl SessionEvents for Quiet {
    fn import_progress(&self, _: &momento_import_engine::ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &thumbnails::ThumbBatch) {}
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let paths = create_project(&NewProject {
        name: "Teste editor".into(),
        client_name: "Teste".into(),
        event_type: "wedding".into(),
        event_date: None,
        location: String::new(),
        originals_dir: None,
        work_dir: a[2].clone(),
    })
    .unwrap();
    let s = ProjectSession::open(&paths.root, Arc::new(Quiet)).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![a[1].clone()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    let photos = s.list_photos().unwrap();
    s.auto_edit_photo(photos[0].id.0, &AutoOptions::default()).unwrap();
    std::fs::create_dir_all(std::path::Path::new(&a[3]).parent().unwrap()).unwrap();
    Library::open(std::path::Path::new(&a[3]))
        .unwrap()
        .touch(&RecentProject {
            path: paths.root.to_string_lossy().into_owned(),
            name: "Teste editor".into(),
            client_name: "Teste".into(),
            event_type: "wedding".into(),
            event_date: None,
            location: String::new(),
            photo_count: photos.len() as u64,
            last_opened: 0,
            available: true,
        })
        .unwrap();
    println!("{} fotografias em {}", photos.len(), paths.root.display());
}
