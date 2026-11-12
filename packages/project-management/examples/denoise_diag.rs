//! Diagnóstico do AI Denoise num RAW real: recorte 1:1 com o original, o nível único antigo
//! e o mapa de ruído novo, lado a lado. `denoise_diag <raw> <saida.png>`
use momento_image_processing::develop::{color::oetf, noise, Source};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&a[1]).unwrap();
    let mut raw = momento_raw_engine::RawFile::open(&data).unwrap();
    let info = raw.info().clone();
    let bm = raw.develop_linear(false).unwrap();
    let src = Source::from_linear16(
        bm.width,
        bm.height,
        &bm.rgb,
        info.orientation,
        info.width,
        info.height,
    );
    let p = &src.levels[0];
    let dn = momento_face_analysis::denoise::Denoiser::load(
        &momento_face_analysis::find_models_dir(&[]).unwrap(),
    )
    .unwrap()
    .unwrap();
    let (cw, ch) = (420usize, 420usize);
    let (x0, y0) = (p.w / 2 - cw / 2, p.h * 2 / 5 - ch / 2);
    // Exposição para o recorte ficar visível (só para ver).
    let crop: Vec<[f32; 3]> = (0..cw * ch).map(|i| p.at(x0 + i % cw, y0 + i / cw)).collect();
    let mean = crop.iter().map(|q| (q[0] + q[1] + q[2]) / 3.0).sum::<f32>() / crop.len() as f32;
    let k = 0.18 / mean.max(1e-4);
    let plane = momento_image_processing::develop::source::Plane {
        w: cw,
        h: ch,
        px: crop.iter().map(|q| q.map(|c| (c * k).clamp(0.0, 1.0))).collect(),
    };
    let disp: Vec<[f32; 3]> = plane.px.iter().map(|q| q.map(oetf)).collect();
    let s_src = Source::from_plane(plane.clone(), cw as u32, ch as u32, 0.0);
    let (lum, chroma) = momento_image_processing::develop::auto::measure_noise(&s_src, 1.0);
    let old = (lum.max(chroma * 0.5).max(1.0) / 255.0).clamp(1.0 / 255.0, 50.0 / 255.0);
    let mut single = disp.clone();
    dn.denoise(&mut single, cw, ch, old, &|_, _| true).unwrap();
    let plan = noise::denoise_plan(&plane, 50.0).unwrap();
    let mut mapped = disp.clone();
    if plan.shuffled {
        dn.denoise_map_shuffled(&mut mapped, cw, ch, &plan.map, &|_, _| true).unwrap();
    } else {
        dn.denoise_map(&mut mapped, cw, ch, &plan.map, &|_, _| true).unwrap();
    }
    let map = plan.map.clone();
    println!("correlacionado: {}", plan.shuffled);
    println!(
        "nível único {:.2} · mapa médio {:.2} (×255)",
        old * 255.0,
        map.iter().sum::<f32>() / map.len() as f32 * 255.0
    );
    let mut out = image::RgbImage::new((cw * 3 + 20) as u32, ch as u32);
    for (k, img) in [&disp, &single, &mapped].iter().enumerate() {
        for (i, q) in img.iter().enumerate() {
            out.put_pixel(
                (k * (cw + 10) + i % cw) as u32,
                (i / cw) as u32,
                image::Rgb(q.map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8)),
            );
        }
    }
    out.save(&a[2]).unwrap();
}
