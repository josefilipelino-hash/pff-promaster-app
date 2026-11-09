//! Folha de contactos com os presets internos aplicados a uma fotografia (avaliação visual).
//! `cargo run --release -p momento-presets --example preset_sheet -- foto.jpg saida.jpg`
use image::{imageops, RgbImage};
use momento_image_processing::develop::{render_rgb, Quality, Settings, Source};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = image::open(&a[1]).expect("fotografia").to_rgb8();
    let src = Source::from_srgb(&img, img.width(), img.height());
    let presets = momento_presets::builtin::builtin();
    let tile = 360u32;
    let mut tiles = vec![(
        "Original".to_string(),
        render_rgb(&src, &Settings::default(), tile, Quality::Final),
    )];
    for p in &presets {
        let s = momento_presets::apply(&Settings::default(), &p.settings, &p.components, 100.0);
        tiles.push((p.name.clone(), render_rgb(&src, &s, tile, Quality::Final)));
    }
    let (tw, th) = (tiles[0].1.width(), tiles[0].1.height());
    let cols = 5u32;
    let rows = (tiles.len() as u32).div_ceil(cols);
    let mut sheet =
        RgbImage::from_pixel(cols * (tw + 6), rows * (th + 6), image::Rgb([20, 20, 20]));
    for (i, (name, t)) in tiles.iter().enumerate() {
        let (x, y) = ((i as u32 % cols) * (tw + 6), (i as u32 / cols) * (th + 6));
        imageops::replace(&mut sheet, t, x as i64, y as i64);
        println!("{i}: {name}");
    }
    sheet.save(&a[2]).unwrap();
}
