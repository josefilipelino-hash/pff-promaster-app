//! Diagnóstico: revela um RAW (linear, meia resolução) e mostra a média de cada canal e o
//! branco do disparo — para comparar plataformas (ex.: Windows vs Linux).
//! Uso: raw_diag <ficheiro.raw>
fn main() {
    let path = std::env::args().nth(1).expect("uso: raw_diag <ficheiro>");
    let data = std::fs::read(&path).expect("ler o ficheiro");
    let mut raw = momento_raw_engine::RawFile::open(&data).expect("abrir RAW");
    let info = raw.info().clone();
    println!(
        "{} {} · {}×{} · descodificador {}",
        info.make, info.model, info.width, info.height, info.decoder
    );
    println!("branco do disparo (sRGB): {:?}", info.as_shot_white);
    let bm = raw.develop_linear(true).expect("revelar");
    let n = (bm.rgb.len() / 3) as f64;
    let mut s = [0f64; 3];
    for c in bm.rgb.chunks_exact(3) {
        for i in 0..3 {
            s[i] += c[i] as f64;
        }
    }
    let m = s.map(|v| v / n / 65535.0);
    println!(
        "{}×{} · média linear R {:.4} G {:.4} B {:.4} · G/R {:.3} G/B {:.3}",
        bm.width,
        bm.height,
        m[0],
        m[1],
        m[2],
        m[1] / m[0],
        m[1] / m[2]
    );
}
