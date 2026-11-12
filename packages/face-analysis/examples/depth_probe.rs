//! Verifica se o modelo de profundidade (Depth Anything V2 Small, ONNX) corre no tract.
//! `depth_probe <model.onnx> <imagem>` → grava `<imagem>.depth.png` e mostra o tempo.
use std::time::Instant;
use tract_onnx::prelude::*;

fn main() -> TractResult<()> {
    let a: Vec<String> = std::env::args().collect();
    let size = 518usize;
    let t = Instant::now();
    let mut m = tract_onnx::onnx()
        .with_ignore_value_info(true)
        .with_ignore_output_shapes(true)
        .model_for_path(&a[1])?;
    // As saídas declaram dimensões simbólicas (batch_size, height/14…): inferidas de novo.
    for i in 0..m.outputs.len() {
        m.set_output_fact(i, InferenceFact::default())?;
    }
    let model = m
        .with_input_fact(0, f32::fact([1, 3, size, size]).into())?
        .into_optimized()?
        .into_runnable()?;
    println!("carregado e otimizado em {:.1?}", t.elapsed());
    let img = image::open(&a[2]).unwrap().to_rgb8();
    let r = image::imageops::resize(
        &img,
        size as u32,
        size as u32,
        image::imageops::FilterType::Triangle,
    );
    let mean = [0.485f32, 0.456, 0.406];
    let std = [0.229f32, 0.224, 0.225];
    let mut x = vec![0f32; 3 * size * size];
    for (i, p) in r.pixels().enumerate() {
        for c in 0..3 {
            x[c * size * size + i] = (p[c] as f32 / 255.0 - mean[c]) / std[c];
        }
    }
    let input = Tensor::from_shape(&[1, 3, size, size], &x)?;
    let t = Instant::now();
    let out = model.run(tvec!(input.into()))?;
    println!("inferência {:.1?}; saída {:?}", t.elapsed(), out[0].shape());
    let d = out[0].to_plain_array_view::<f32>()?;
    let v: Vec<f32> = d.iter().copied().collect();
    let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |a, &x| (a.0.min(x), a.1.max(x)));
    let n = (v.len() as f64).sqrt() as u32;
    let im = image::GrayImage::from_fn(n, n, |x, y| {
        image::Luma([(((v[(y * n + x) as usize] - lo) / (hi - lo)) * 255.0) as u8])
    });
    im.save(format!("{}.depth.png", a[2])).unwrap();
    println!("profundidade relativa {lo:.2}..{hi:.2}");
    Ok(())
}
