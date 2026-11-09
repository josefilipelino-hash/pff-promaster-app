//! Máscara de céu (heurística) sobre uma imagem: grava a imagem com o céu a vermelho.
//! `cargo run --release -p momento-image-processing --example sky_eval -- foto.jpg saida.jpg`
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = image::open(&a[1]).unwrap().thumbnail(1024, 1024).to_rgb8();
    let m = momento_image_processing::develop::sky::sky_mask(&img);
    let mut out = img.clone();
    for (i, p) in out.pixels_mut().enumerate() {
        let k = m[i] * 0.6;
        p[0] = (p[0] as f32 + (217.0 - p[0] as f32) * k) as u8;
        p[1] = (p[1] as f32 * (1.0 - k)) as u8;
        p[2] = (p[2] as f32 * (1.0 - k)) as u8;
    }
    out.save(&a[2]).unwrap();
    println!("céu: {:.1} %", m.iter().sum::<f32>() / m.len() as f32 * 100.0);
}
