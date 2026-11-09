//! Diagnóstico visual: `cargo run -p momento-face-analysis --release --example faces_report -- <pasta_saida> <imagens...>`
//! Desenha as caixas (verde = olhos abertos, vermelho = algum olho fechado) e imprime os valores.

use image::{Rgb, RgbImage};
use momento_face_analysis::{find_models_dir, VisionModels};
use std::time::Instant;

fn rect(img: &mut RgbImage, x: f32, y: f32, w: f32, h: f32, c: Rgb<u8>) {
    let (iw, ih) = (img.width() as i64, img.height() as i64);
    for t in 0..3i64 {
        for xx in (x as i64)..=((x + w) as i64) {
            for yy in [y as i64 + t, (y + h) as i64 - t] {
                if xx >= 0 && yy >= 0 && xx < iw && yy < ih {
                    img.put_pixel(xx as u32, yy as u32, c)
                }
            }
        }
        for yy in (y as i64)..=((y + h) as i64) {
            for xx in [x as i64 + t, (x + w) as i64 - t] {
                if xx >= 0 && yy >= 0 && xx < iw && yy < ih {
                    img.put_pixel(xx as u32, yy as u32, c)
                }
            }
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = std::path::PathBuf::from(args.next().expect("pasta de saída"));
    std::fs::create_dir_all(&out).unwrap();
    let m = VisionModels::load(&find_models_dir(&[]).expect("modelos")).unwrap();
    for path in args {
        let Ok(img) = image::open(&path) else { continue };
        let mut img = img.to_rgb8();
        if img.width().max(img.height()) > 1600 {
            img = image::imageops::thumbnail(&img, 1600, 1600 * img.height() / img.width().max(1));
        }
        let t = Instant::now();
        let faces = m.analyze_faces(&img, true).unwrap();
        let ms = t.elapsed().as_millis();
        let name = std::path::Path::new(&path).file_name().unwrap().to_string_lossy().into_owned();
        println!("{name} ({}×{}, {ms} ms): {} rosto(s)", img.width(), img.height(), faces.len());
        for f in &faces {
            let (l, r) = f.eye_blink();
            let d = f.detection;
            println!("   [{:4.0},{:4.0} {:3.0}px] det {:.2} · olhos fechados E/D {:.2}/{:.2} · sorriso {:.2}", d.x, d.y, d.w, d.score, l, r, f.smile());
            let closed = l > 0.5 || r > 0.5;
            rect(
                &mut img,
                d.x,
                d.y,
                d.w,
                d.h,
                if closed { Rgb([255, 40, 40]) } else { Rgb([40, 255, 40]) },
            );
        }
        img.save(out.join(format!("{name}.faces.jpg"))).unwrap();
    }
}
