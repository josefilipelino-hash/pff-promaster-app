//! Protótipo da luz de retrato (avaliação): `portrait_proto <depth.onnx> <foto.jpg> <saida.jpg>`
//! Painel 3×2: original | Rembrandt | borboleta (suave) / lateral dura | contraluz | janela quente.
use momento_image_processing::develop::color::{eotf, oetf};
use momento_image_processing::develop::relight::{self, Portrait, PortraitLight};
use momento_image_processing::develop::source::Plane;
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

fn upsample(src: &[f32], sw: usize, sh: usize, w: usize, h: usize) -> Vec<f32> {
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let fx = ((x as f32 + 0.5) / w as f32 * sw as f32 - 0.5).clamp(0.0, sw as f32 - 1.0);
            let fy = ((y as f32 + 0.5) / h as f32 * sh as f32 - 0.5).clamp(0.0, sh as f32 - 1.0);
            let (x0, y0) = (fx as usize, fy as usize);
            let (x1, y1) = ((x0 + 1).min(sw - 1), (y0 + 1).min(sh - 1));
            let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
            let a = src[y0 * sw + x0] * (1.0 - tx) + src[y0 * sw + x1] * tx;
            let b = src[y1 * sw + x0] * (1.0 - tx) + src[y1 * sw + x1] * tx;
            a * (1.0 - ty) + b * ty
        })
        .collect()
}

fn to_rgb(p: &Plane, l: &relight::Light) -> image::RgbImage {
    image::RgbImage::from_fn(p.w as u32, p.h as u32, |x, y| {
        let i = y as usize * p.w + x as usize;
        let v: [f32; 3] = std::array::from_fn(|c| p.px[i][c] * l.gain[i][c] + l.add[i][c]);
        let sh = |x: f32| if x <= 0.8 { x } else { 0.8 + 0.2 * (1.0 - (-(x - 0.8) / 0.2).exp()) };
        image::Rgb(v.map(|c| (oetf(sh(c.max(0.0))) * 255.0).round().clamp(0.0, 255.0) as u8))
    })
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let srgb = image::open(&a[2]).unwrap().to_rgb8();
    let plane = Plane {
        w: srgb.width() as usize,
        h: srgb.height() as usize,
        px: srgb.pixels().map(|p| p.0.map(|c| eotf(c as f32 / 255.0))).collect(),
    };
    let (w, h) = (plane.w, plane.h);
    let dir = momento_face_analysis::find_models_dir(&[]).expect("modelos");
    let vm = momento_face_analysis::VisionModels::load(&dir).unwrap();
    let faces = vm.analyze_faces(&srgb, false).unwrap();
    let seg = momento_face_analysis::segment::Segmenter::load(&dir).unwrap();
    let cm = seg.classes(&srgb).unwrap().expect("segmentação");
    let up = |c: usize| upsample(&cm.probs[c], cm.w, cm.h, w, h);
    let person: Vec<f32> = up(0).iter().map(|b| (1.0 - b).clamp(0.0, 1.0)).collect();
    let face_skin = up(3);
    if let Ok(out) = std::env::var("MASK_OUT") {
        image::GrayImage::from_fn(w as u32, h as u32, |x, y| {
            image::Luma([(person[y as usize * w + x as usize] * 255.0) as u8])
        })
        .save(out)
        .unwrap();
        return;
    }
    let hair = up(1);
    eprintln!("{} rosto(s)", faces.len());
    let (d, ds) = depth(&a[1], &srgb);
    let surf = relight::surface(&d, ds, ds, &plane);
    let p = Portrait {
        faces: faces
            .iter()
            .map(|f| f.landmarks.iter().zip(&f.depth).map(|(l, z)| [l.0, l.1, *z]).collect())
            .collect(),
        person,
        face_skin,
        hair,
    };
    let looks = [
        PortraitLight {
            azimuth: -50.0,
            elevation: 35.0,
            intensity: 0.9,
            kelvin: 5200.0,
            softness: 0.25,
            background: 0.35,
        },
        PortraitLight {
            azimuth: 0.0,
            elevation: 45.0,
            intensity: 0.8,
            kelvin: 5600.0,
            softness: 0.75,
            background: 0.15,
        },
        PortraitLight {
            azimuth: 85.0,
            elevation: 10.0,
            intensity: 1.0,
            kelvin: 5000.0,
            softness: 0.1,
            background: 0.5,
        },
        PortraitLight {
            azimuth: 160.0,
            elevation: 20.0,
            intensity: 1.0,
            kelvin: 3400.0,
            softness: 0.3,
            background: 0.3,
        },
        PortraitLight {
            azimuth: -70.0,
            elevation: 15.0,
            intensity: 0.9,
            kelvin: 3200.0,
            softness: 0.55,
            background: 0.25,
        },
    ];
    let ident = relight::Light { gain: vec![[1.0; 3]; w * h], add: vec![[0.0; 3]; w * h] };
    let mut panels = vec![to_rgb(&plane, &ident)];
    for l in &looks {
        panels.push(to_rgb(&plane, &relight::portrait_light(&plane, &surf, &p, l)));
    }
    let (pw, ph) = (w as u32, h as u32);
    let mut out = image::RgbImage::new(pw * 3 + 20, ph * 2 + 10);
    for (k, im) in panels.iter().enumerate() {
        image::imageops::replace(
            &mut out,
            im,
            ((k % 3) as u32 * (pw + 10)) as i64,
            ((k / 3) as u32 * (ph + 10)) as i64,
        );
    }
    out.save(&a[3]).unwrap();
}
