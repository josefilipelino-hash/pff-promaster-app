//! Diagnóstico do Upright: grava antes/depois (JPEG lado a lado) de um RAW.
//! `upright_diag <raw> <saida.jpg> [modo]`
use momento_image_processing::develop::{self, upright::UprightMode, Geometry, Quality, Settings};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let mut raw = momento_raw_engine::RawFile::open(&data).unwrap();
    let info = raw.info().clone();
    let bm = raw.develop_linear(true).unwrap();
    let src = develop::Source::from_linear16(
        bm.width,
        bm.height,
        &bm.rgb,
        info.orientation,
        info.width,
        info.height,
    );
    let mode = UprightMode::parse(a.get(3).map(String::as_str).unwrap_or("auto")).unwrap();
    let r = develop::upright(&src, &Settings::default(), mode, None);
    println!("{r:?}");
    let before = develop::render_rgb(&src, &Settings::default(), 900, Quality::Final);
    let after = match &r {
        Some(r) => develop::render_rgb(
            &src,
            &Settings {
                geometry: Geometry {
                    vertical: r.vertical,
                    horizontal: r.horizontal,
                    straighten: r.straighten,
                    ..Default::default()
                },
                ..Default::default()
            },
            900,
            Quality::Final,
        ),
        None => before.clone(),
    };
    let mut out = image::RgbImage::new(
        before.width() + after.width() + 10,
        before.height().max(after.height()),
    );
    image::imageops::replace(&mut out, &before, 0, 0);
    image::imageops::replace(&mut out, &after, (before.width() + 10) as i64, 0);
    out.save(&a[2]).unwrap();
}
