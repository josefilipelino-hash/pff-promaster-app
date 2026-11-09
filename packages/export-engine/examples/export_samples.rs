//! Gera um ficheiro de cada formato/profundidade/espaço a partir de uma fotografia, para
//! validação com descodificadores independentes (`scripts/validate-exports.py`).
//! `cargo run --release -p momento-export-engine --example export_samples -- foto.jpg pasta/`
//! Escreve também `manifest.json` (o que cada ficheiro deve conter) e `reference.f32`
//! (a imagem esperada em Rec. 2020 linear). Nada é escrito no original.

use momento_color_management::ColorSpace;
use momento_export_engine::*;
use momento_image_processing::develop::color::{eotf, mul3, SRGB_TO_REC2020};
use momento_image_processing::develop::LinearImage;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = image::open(&a[1]).unwrap().to_rgb8();
    let img = image::imageops::resize(
        &img,
        1200,
        1200 * img.height() / img.width(),
        image::imageops::FilterType::Lanczos3,
    );
    let (w, h) = img.dimensions();
    let px: Vec<[f32; 3]> = img
        .pixels()
        .map(|p| mul3(&SRGB_TO_REC2020, [p[0], p[1], p[2]].map(|c| eotf(c as f32 / 255.0))))
        .collect();
    let lin = LinearImage { width: w, height: h, px };
    let out = std::path::PathBuf::from(&a[2]);
    std::fs::create_dir_all(&out).unwrap();
    let raw: Vec<u8> = lin.px.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(out.join("reference.f32"), raw).unwrap();
    let photo = PhotoMeta {
        file_stem: "amostra".into(),
        capture_time: Some("2026-06-15T17:42:05".into()),
        make: Some("Canon".into()),
        model: Some("EOS R5".into()),
        gps: Some((38.79, -9.39)),
        ..Default::default()
    };
    use BitDepth::*;
    use Compression as C;
    use Format::*;
    let cases: Vec<(Format, BitDepth, C, ColorSpace)> = vec![
        (Jpeg, B8, C::Lossy, ColorSpace::Srgb),
        (Jpeg, B8, C::Lossy, ColorSpace::AdobeRgb),
        (Png, B8, C::Fast, ColorSpace::Srgb),
        (Png, B16, C::Best, ColorSpace::DisplayP3),
        (Tiff, B8, C::Lzw, ColorSpace::Srgb),
        (Tiff, B16, C::Zip, ColorSpace::ProPhoto),
        (Tiff, F32, C::None, ColorSpace::Rec2020),
        (BigTiff, B16, C::Zip, ColorSpace::AdobeRgb),
        (Webp, B8, C::Lossy, ColorSpace::Srgb),
        (Webp, B8, C::Lossless, ColorSpace::DisplayP3),
        (Avif, B8, C::Lossy, ColorSpace::Srgb),
        (Avif, B10, C::Lossy, ColorSpace::DisplayP3),
        (Avif, B12, C::Lossy, ColorSpace::Rec2020),
        (JpegXl, B8, C::Lossless, ColorSpace::Srgb),
        (JpegXl, B16, C::Lossless, ColorSpace::DisplayP3),
        (JpegXl, B16, C::Lossless, ColorSpace::AdobeRgb),
        (Jpeg2000, B8, C::Lossless, ColorSpace::Srgb),
        (Jpeg2000, B12, C::Lossy, ColorSpace::Srgb),
        (Jpeg2000, B16, C::Lossy, ColorSpace::Srgb),
        (Jpeg2000, B8, C::Lossy, ColorSpace::Srgb),
        (Exr, F16, C::Zip, ColorSpace::Rec2020),
        (Exr, F32, C::Piz, ColorSpace::Srgb),
        (Psd, B8, C::Rle, ColorSpace::Srgb),
        (Psd, B16, C::None, ColorSpace::AdobeRgb),
        (Pdf, B8, C::Lossy, ColorSpace::Srgb),
    ];
    let mut manifest = Vec::new();
    for (f, d, c, s) in cases {
        let r = Recipe {
            format: f,
            depth: d,
            compression: c,
            color: s,
            quality: 92,
            metadata: Metadata {
                copyright: "© 2026 Estúdio".into(),
                creator: "José Lino".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let p = Prepared::new(&r).unwrap();
        let t = std::time::Instant::now();
        let o = p.produce(&lin, &photo).unwrap();
        let name = format!("{:?}-{:?}-{:?}-{}.{}", f, d, c, s.id(), p.extension()).to_lowercase();
        std::fs::write(out.join(&name), &o.bytes).unwrap();
        println!("{name}: {} KB em {:.0?}", o.bytes.len() / 1024, t.elapsed());
        manifest.push(serde_json::json!({
            "file": name, "format": f, "depth": d, "compression": c, "space": s.id(),
            "width": o.width, "height": o.height, "profile": p.color_description(),
        }));
    }
    std::fs::write(out.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap())
        .unwrap();
    // Tempo de reamostragem de uma imagem de 45 MP (8256×5504) para 2048 px.
    let (bw, bh) = (8256usize, 5504usize);
    let big: Vec<[f32; 3]> = (0..bw * bh).map(|i| [(i % 251) as f32 / 251.0, 0.3, 0.5]).collect();
    let size = Size { mode: SizeMode::LongEdge, a: 2048.0, ..Default::default() };
    let plan = process::plan(&size, bw as u32, bh as u32);
    let t = std::time::Instant::now();
    let r = process::resample(&big, bw, bh, &plan);
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    println!("reamostragem 45 MP → {}×{}: {:.0?} ({cores} núcleos)", plan.w, plan.h, t.elapsed());
    assert_eq!(r.len(), (plan.w * plan.h) as usize);
}
