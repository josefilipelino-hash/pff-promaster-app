//! Verifica se "Repor" deixa a fotografia como não editada. Uso: `reset_check <projeto.momento>`
use momento_project_management::*;
use std::sync::Arc;
struct Quiet;
impl SessionEvents for Quiet {
    fn import_progress(&self, _: &momento_import_engine::ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &thumbnails::ThumbBatch) {}
}
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let s = ProjectSession::open(std::path::Path::new(&p), Arc::new(Quiet)).unwrap();
    let show = |s: &Arc<ProjectSession>| {
        for r in s.list_photos().unwrap() {
            println!("  {} editada={}", r.file_name, r.edited);
        }
    };
    println!("antes:");
    show(&s);
    let k = s.list_photos().unwrap()[0].id.0;
    s.reset_edit(k).unwrap();
    println!("depois de Repor a primeira:");
    show(&s);
}
