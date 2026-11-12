//! Diagnóstico do perfil de objetiva: antes/depois (JPEG lado a lado) de um RAW.
//! `lens_diag <raw> <saida.jpg>`
use momento_image_processing::develop::{self, Lens, Quality, Settings};
use momento_project_management::lensprofile::{profile_for, ProfileQuery};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let mut raw = momento_raw_engine::RawFile::open(&data).unwrap();
    let i = raw.info().clone();
    let q = ProfileQuery {
        camera_make: i.make.clone(),
        camera_model: i.model.clone(),
        lens: i.lens.clone().unwrap_or_default(),
        focal: i.focal_length.map(|f| f as f32),
        focal35: i.focal_length_35mm.map(|f| f as f32),
        aperture: i.aperture.map(|a| a as f32),
    };
    let p = profile_for(&q).expect("sem perfil");
    println!("{p:?}");
    let bm = raw.develop_linear(true).unwrap();
    let src = develop::Source::from_linear16(
        bm.width,
        bm.height,
        &bm.rgb,
        i.orientation,
        i.width,
        i.height,
    );
    let before = develop::render_rgb(&src, &Settings::default(), 900, Quality::Final);
    let after = develop::render_rgb(
        &src,
        &Settings { lens: Lens { profile: p, ..Default::default() }, ..Default::default() },
        900,
        Quality::Final,
    );
    let mut out = image::RgbImage::new(
        before.width() + after.width() + 10,
        before.height().max(after.height()),
    );
    image::imageops::replace(&mut out, &before, 0, 0);
    image::imageops::replace(&mut out, &after, (before.width() + 10) as i64, 0);
    out.save(&a[2]).unwrap();
}
