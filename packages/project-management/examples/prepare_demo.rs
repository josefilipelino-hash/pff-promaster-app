//! Cria um projeto e importa uma pasta (sem analisar), e regista-o na lista de recentes de
//! uma biblioteca. Usado para verificar a aplicação real num ecrã virtual:
//! `cargo run --release -p momento-project-management --example prepare_demo -- <fotos> <trabalho> <library.db>`
use momento_database::import::ImportMode;
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
        name: "Casamento Ana & Pedro".into(),
        client_name: "Ana & Pedro".into(),
        event_type: "wedding".into(),
        event_date: Some("2026-06-15".into()),
        location: "Sintra".into(),
        originals_dir: None,
        work_dir: a[2].clone(),
    })
    .unwrap();
    let s = ProjectSession::open(&paths.root, Arc::new(Quiet)).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![a[1].clone()],
        mode: ImportMode::Reference,
        recursive: true,
    })
    .unwrap();
    s.wait_import();
    while s.thumbnails_remaining() > 0 {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let lib = momento_database::library::Library::open(std::path::Path::new(&a[3])).unwrap();
    lib.touch(&s.recent_entry().unwrap()).unwrap();
    println!("{} ({} fotografias)", paths.root.display(), s.list_photos().unwrap().len());
    s.close();
}
