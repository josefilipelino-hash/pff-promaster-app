//! Avaliação visual do retrato: deteta os rostos de uma fotografia e grava o rosto sem e com
//! retoque (pele, olheiras, brilho, dentes, olhos), lado a lado. Nada é escrito no original.
//! `cargo run --release -p momento-project-management --example portrait_eval -- foto.jpg saida.jpg [pele olheiras brilho dentes olhos]`
use image::{imageops, RgbImage};
use momento_database::import::ImportMode;
use momento_image_processing::develop::{render_rgb, Portrait, Quality, Settings, Source};
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
    let v = |i: usize, d: f32| a.get(i).and_then(|x| x.parse().ok()).unwrap_or(d);
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("fotos");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(&a[1], dir.join("foto.jpg")).unwrap();
    let paths = create_project(&NewProject {
        name: "Retrato".into(),
        client_name: "-".into(),
        event_type: "wedding".into(),
        event_date: None,
        location: String::new(),
        originals_dir: None,
        work_dir: tmp.path().to_string_lossy().into_owned(),
    })
    .unwrap();
    let s = ProjectSession::open(&paths.root, Arc::new(Quiet)).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![dir.to_string_lossy().into_owned()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    let key = s.list_photos().unwrap()[0].id.0;
    let faces = s.detect_portrait(key).unwrap();
    println!("{} rosto(s)", faces.len());
    let img = image::open(&a[1]).unwrap().to_rgb8();
    let src = Source::from_srgb(&img, img.width(), img.height());
    let p = Portrait {
        skin: v(3, 35.0),
        dark_circles: v(4, 40.0),
        shine: v(5, 40.0),
        teeth: v(6, 40.0),
        eyes: v(7, 30.0),
        faces: faces.clone(),
    };
    let before = render_rgb(&src, &Settings::default(), 2000, Quality::Final);
    let after =
        render_rgb(&src, &Settings { portrait: p, ..Default::default() }, 2000, Quality::Final);
    let f = &faces[0];
    let bb = f.oval.iter().fold([1f32, 1.0, 0.0, 0.0], |b, p| {
        [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
    });
    let (w, h) = (before.width() as f32, before.height() as f32);
    let (x0, y0) = (((bb[0] - 0.02) * w) as u32, ((bb[1] - 0.02) * h) as u32);
    let (cw, ch) = (((bb[2] - bb[0] + 0.04) * w) as u32, ((bb[3] - bb[1] + 0.04) * h) as u32);
    let crop = |i: &RgbImage| imageops::crop_imm(i, x0, y0, cw, ch).to_image();
    let (cb, ca) = (crop(&before), crop(&after));
    let mut out = RgbImage::new(cw * 2 + 8, ch);
    imageops::replace(&mut out, &cb, 0, 0);
    imageops::replace(&mut out, &ca, cw as i64 + 8, 0);
    out.save(&a[2]).unwrap();
    s.close();
}
