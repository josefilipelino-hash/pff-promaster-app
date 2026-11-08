//! Gera um conjunto de fotografias sintéticas para testes manuais e de desempenho.
//!
//! Uso: `cargo run -p momento-test-fixtures --release --bin generate-fixtures -- <pasta> [quantidade] [largura]`
//!
//! Conteúdo gerado em `<pasta>`:
//! - `cerimonia/`, `rececao/sub/…` — JPEG com EXIF variado (4 câmaras, orientações 1/3/6/8);
//! - `png/` e `tiff/` — PNG (com eXIf) e TIFF 8/16 bits;
//! - `duplicados/` — cópias exatas com outros nomes (devem ser detetadas);
//! - `invalidos/` — ficheiro truncado, ficheiro de texto com extensão .jpg, formato não suportado.

use momento_test_fixtures::*;
use rayon::prelude::*;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("indicar a pasta de destino"));
    let count: u32 = args.next().map(|s| s.parse().expect("quantidade")).unwrap_or(500);
    let width: u32 = args.next().map(|s| s.parse().expect("largura")).unwrap_or(3000);
    let height = width * 2 / 3;
    let started = std::time::Instant::now();

    let orientations = [1u16, 1, 1, 6, 1, 8, 1, 3];
    (0..count).into_par_iter().for_each(|i| {
        let o = orientations[(i as usize) % orientations.len()];
        let img = test_image(width, height, i, o);
        let dir =
            if i < count / 2 { "cerimonia".to_string() } else { format!("rececao/sub{}", i % 3) };
        let p = out.join(dir).join(format!("IMG_{:05}.jpg", i + 1));
        write(&p, &jpeg_bytes(&img, Some(&spec_for(i, o)), 88)).unwrap();
    });

    for i in 0..10u32 {
        let img = test_image(width / 2, height / 2, 1000 + i, 1);
        write(&out.join(format!("png/PNG_{i:03}.png")), &png_bytes(&img, Some(&spec_for(i, 1))))
            .unwrap();
        write(&out.join(format!("tiff/TIFF_{i:03}.tif")), &tiff_bytes(&img)).unwrap();
    }
    write(&out.join("tiff/TIFF16_000.tiff"), &tiff16_bytes(width / 2, height / 2)).unwrap();

    for i in 0..5u32 {
        let src = out.join("cerimonia").join(format!("IMG_{:05}.jpg", i + 1));
        if src.exists() {
            std::fs::create_dir_all(out.join("duplicados")).unwrap();
            std::fs::copy(&src, out.join(format!("duplicados/COPIA_{i}.jpg"))).unwrap();
        }
    }

    let good = jpeg_bytes(&test_image(800, 600, 7, 1), None, 85);
    write(&out.join("invalidos/truncado.jpg"), &good[..good.len() / 3]).unwrap();
    write(&out.join("invalidos/texto.jpg"), b"isto nao e uma imagem").unwrap();
    write(&out.join("invalidos/notas.txt"), b"notas do evento").unwrap();

    println!(
        "Geradas {} fotografias em {} ({:.1}s)",
        count + 29,
        out.display(),
        started.elapsed().as_secs_f32()
    );
}
