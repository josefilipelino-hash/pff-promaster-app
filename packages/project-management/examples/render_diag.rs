//! Diagnóstico: revela um RAW pelo motor de edição (neutro e com Auto) e mostra a média
//! sRGB do resultado — para comparar plataformas. Uso: render_diag <ficheiro.raw>
use momento_image_processing::develop::{self, Quality, RenderOptions, Settings, Source};

fn mean(r: &develop::Rendered) -> [f64; 3] {
    let n = (r.rgba.len() / 4) as f64;
    let mut s = [0f64; 3];
    for p in r.rgba.chunks_exact(4) {
        for i in 0..3 {
            s[i] += p[i] as f64;
        }
    }
    s.map(|v| v / n)
}

fn main() {
    let path = std::env::args().nth(1).expect("uso: render_diag <ficheiro>");
    let data = std::fs::read(&path).unwrap();
    let mut raw = momento_raw_engine::RawFile::open(&data).unwrap();
    let info = raw.info().clone();
    let bm = raw.develop_linear(true).unwrap();
    let mut src = Source::from_linear16(
        bm.width,
        bm.height,
        &bm.rgb,
        info.orientation,
        info.width,
        info.height,
    );
    src.as_shot_k = info.as_shot_white.and_then(develop::color::kelvin_of_white);
    println!(
        "{} · Kelvin do disparo {:?} · base {:.2} EV",
        info.model,
        src.as_shot_k.map(|k| k.round()),
        src.baseline_ev
    );
    let neutral = Settings::default();
    let o = RenderOptions {
        max_width: 800,
        max_height: 800,
        quality: Quality::Final,
        uncropped: false,
    };
    let r = develop::render(&src, &neutral, &o);
    println!("neutro: sRGB médio {:?}", mean(&r).map(|v| v.round()));
    let auto = develop::auto::auto_edit(&src, &neutral, &Default::default(), &Default::default());
    println!(
        "auto: balanço {:.1}/{:.1} exposição {:.2}",
        auto.0.wb.temperature, auto.0.wb.tint, auto.0.light.exposure
    );
    let r = develop::render(&src, &auto.0, &o);
    println!("auto: sRGB médio {:?}", mean(&r).map(|v| v.round()));
}
