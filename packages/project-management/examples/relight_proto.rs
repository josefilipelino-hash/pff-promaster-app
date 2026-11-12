//! Protótipo de iluminação (para avaliação): `relight_proto <modelo.onnx> <foto (RAW/JPEG)> <saida.jpg>`
//! Painel 3×2: original | foco de luz | contraluz / pôr do sol | meio-dia | hora azul.
use momento_image_processing::develop::color::{eotf, oetf};
use momento_image_processing::develop::relight::{self, Backlight, Spot, TimeOfDay};
use momento_image_processing::develop::source::Plane;
use momento_image_processing::develop::{self, Quality, Settings};
use std::time::Instant;
use tract_onnx::prelude::*;

fn depth(model: &str, img: &image::RgbImage) -> (Vec<f32>, usize) {
    let size = 518usize;
    let mut m = tract_onnx::onnx()
        .with_ignore_value_info(true)
        .with_ignore_output_shapes(true)
        .model_for_path(model)
        .unwrap();
    for i in 0..m.outputs.len() {
        m.set_output_fact(i, InferenceFact::default()).unwrap();
    }
    let run = m
        .with_input_fact(0, f32::fact([1, 3, size, size]).into())
        .unwrap()
        .into_optimized()
        .unwrap()
        .into_runnable()
        .unwrap();
    let r = image::imageops::resize(
        img,
        size as u32,
        size as u32,
        image::imageops::FilterType::Triangle,
    );
    let (mean, std) = ([0.485f32, 0.456, 0.406], [0.229f32, 0.224, 0.225]);
    let mut x = vec![0f32; 3 * size * size];
    for (i, p) in r.pixels().enumerate() {
        for c in 0..3 {
            x[c * size * size + i] = (p[c] as f32 / 255.0 - mean[c]) / std[c];
        }
    }
    let out = run.run(tvec!(Tensor::from_shape(&[1, 3, size, size], &x).unwrap().into())).unwrap();
    (out[0].to_plain_array_view::<f32>().unwrap().iter().copied().collect(), size)
}

fn to_rgb(p: &Plane, gain: &[[f32; 3]], add: &[[f32; 3]]) -> image::RgbImage {
    image::RgbImage::from_fn(p.w as u32, p.h as u32, |x, y| {
        let i = y as usize * p.w + x as usize;
        let q = p.px[i];
        let v: [f32; 3] = std::array::from_fn(|c| q[c] * gain[i][c] + add[i][c]);
        // Ombro suave acima de 0,8 (sem cortes duros nos realces novos).
        let sh = |x: f32| if x <= 0.8 { x } else { 0.8 + 0.2 * (1.0 - (-(x - 0.8) / 0.2).exp()) };
        image::Rgb(v.map(|c| (oetf(sh(c.max(0.0))) * 255.0).round().clamp(0.0, 255.0) as u8))
    })
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let path = &a[2];
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    let srgb = if ["jpg", "jpeg", "png"].contains(&ext.as_str()) {
        let im = image::open(path).unwrap().to_rgb8();
        let s = 1100.0 / im.width().max(im.height()) as f32;
        image::imageops::resize(
            &im,
            (im.width() as f32 * s) as u32,
            (im.height() as f32 * s) as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        let data = std::fs::read(path).unwrap();
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
        develop::render_rgb(&src, &Settings::default(), 1100, Quality::Final)
    };
    let plane = Plane {
        w: srgb.width() as usize,
        h: srgb.height() as usize,
        px: srgb.pixels().map(|p| p.0.map(|c| eotf(c as f32 / 255.0))).collect(),
    };
    let t = Instant::now();
    let (d, ds) = depth(&a[1], &srgb);
    let surf = relight::surface(&d, ds, ds, &plane);
    eprintln!("profundidade + superfície: {:.1?}", t.elapsed());
    // Foco no que está mais perto (centro dos 4 % mais próximos).
    let mut idx: Vec<usize> = (0..plane.px.len()).collect();
    idx.sort_by(|&x, &y| surf.near[y].total_cmp(&surf.near[x]));
    let top = &idx[..idx.len() / 25];
    let (u, v) = top
        .iter()
        .fold((0f32, 0f32), |s, &i| (s.0 + (i % plane.w) as f32, s.1 + (i / plane.w) as f32));
    let (u, v) =
        (u / top.len() as f32 / plane.w as f32, v / top.len() as f32 / plane.h as f32 - 0.08);
    let ones = vec![[1f32; 3]; plane.px.len()];
    let zeros = vec![[0f32; 3]; plane.px.len()];
    // Foco: ponto dado (u, v) ou o centro do que está mais perto.
    let (u, v) = match (a.get(4), a.get(5)) {
        (Some(x), Some(y)) => (x.parse().unwrap(), y.parse().unwrap()),
        _ => (u, v),
    };
    let t = Instant::now();
    let spot = relight::spot(
        &surf,
        &plane,
        &Spot { u, v, radius: 0.22, intensity: 1.8, kelvin: 3400.0, ambient_cut: 0.6 },
    );
    let rim =
        relight::backlight(&surf, &plane, &Backlight { intensity: 1.0, kelvin: 3200.0, side: 0.7 });
    let dusk = relight::time_of_day(&surf, &plane, &TimeOfDay { t: 0.93, amount: 0.9, side: -1.0 });
    let noon = relight::time_of_day(&surf, &plane, &TimeOfDay { t: 0.5, amount: 0.9, side: 1.0 });
    let blue = relight::time_of_day(&surf, &plane, &TimeOfDay { t: 1.15, amount: 0.9, side: 1.0 });
    eprintln!("luzes: {:.1?}", t.elapsed());
    let panels = [
        to_rgb(&plane, &ones, &zeros),
        to_rgb(&plane, &spot.gain, &spot.add),
        to_rgb(&plane, &rim.gain, &rim.add),
        to_rgb(&plane, &dusk.gain, &dusk.add),
        to_rgb(&plane, &noon.gain, &noon.add),
        to_rgb(&plane, &blue.gain, &blue.add),
    ];
    let (w, h) = (plane.w as u32, plane.h as u32);
    let mut out = image::RgbImage::new(w * 3 + 20, h * 2 + 10);
    for (k, p) in panels.iter().enumerate() {
        image::imageops::replace(
            &mut out,
            p,
            ((k % 3) as u32 * (w + 10)) as i64,
            ((k / 3) as u32 * (h + 10)) as i64,
        );
    }
    out.save(&a[3]).unwrap();
}
