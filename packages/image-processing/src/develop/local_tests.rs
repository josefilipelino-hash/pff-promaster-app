//! Fase 8 — ajustes locais, retoque e retrato medidos no render.

use super::*;
use image::{Rgb, RgbImage};

fn scene() -> Source {
    // Degradê horizontal com textura fina e uma faixa colorida.
    let img = RgbImage::from_fn(600, 400, |x, y| {
        let n = (((x * 7919) ^ (y * 104_729)) % 21) as i32 - 10;
        let v = (60 + x as i32 * 140 / 600 + n).clamp(0, 255) as u8;
        if (150..250).contains(&y) {
            Rgb([v, (v as f32 * 0.6) as u8, (v as f32 * 0.3) as u8])
        } else {
            Rgb([v, v, v])
        }
    });
    Source::from_srgb(&img, 600, 400)
}

fn out(src: &Source, s: &Settings) -> RgbImage {
    render_rgb(src, s, 600, Quality::Final)
}

fn mean(img: &RgbImage, x0: u32, y0: u32, x1: u32, y1: u32) -> [f32; 3] {
    let mut a = [0.0; 3];
    let mut n = 0.0;
    for y in y0..y1 {
        for x in x0..x1 {
            let p = img.get_pixel(x, y);
            for c in 0..3 {
                a[c] += p[c] as f32;
            }
            n += 1.0;
        }
    }
    a.map(|v| v / n)
}

fn local(adjust: LocalAdjust, shape: MaskShape) -> Settings {
    Settings {
        masks: vec![LocalMask {
            id: "m1".into(),
            components: vec![MaskComponent { shape, ..Default::default() }],
            adjust,
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn radial() -> MaskShape {
    MaskShape::Radial { cx: 0.5, cy: 0.25, rx: 0.1, ry: 0.1, angle: 0.0, feather: 20.0 }
}

#[test]
fn local_adjustments_stay_inside_the_mask() {
    let src = scene();
    let base = out(&src, &Settings::default());
    let s = local(LocalAdjust { exposure: 1.0, ..Default::default() }, radial());
    let r = out(&src, &s);
    let (inside0, inside1) = (mean(&base, 290, 90, 310, 110), mean(&r, 290, 90, 310, 110));
    assert!(inside1[1] > inside0[1] + 20.0, "{inside0:?} → {inside1:?}");
    assert_eq!(mean(&base, 20, 300, 60, 340), mean(&r, 20, 300, 60, 340), "fora da máscara igual");
    // Temperatura, saturação, contraste locais.
    let warm = out(&src, &local(LocalAdjust { temperature: 50.0, ..Default::default() }, radial()));
    let w = mean(&warm, 290, 90, 310, 110);
    assert!(w[0] - w[2] > inside0[0] - inside0[2] + 8.0);
    let band = MaskShape::Linear { x0: 0.0, y0: 0.0, x1: 0.0, y1: 0.01 };
    let desat = out(
        &src,
        &local(
            LocalAdjust { saturation: -100.0, ..Default::default() },
            MaskShape::Luminance { min: 0.0, max: 1.0, smooth: 0.0 },
        ),
    );
    let d = mean(&desat, 400, 180, 440, 220);
    assert!((d[0] - d[2]).abs() < 3.0, "saturação −100 na faixa colorida: {d:?}");
    let hl = out(
        &src,
        &local(
            LocalAdjust { highlights: -100.0, whites: -50.0, ..Default::default() },
            MaskShape::Luminance { min: 0.0, max: 1.0, smooth: 0.0 },
        ),
    );
    assert!(mean(&hl, 560, 20, 590, 60)[1] < mean(&base, 560, 20, 590, 60)[1] - 3.0);
    let _ = band;
    // Textura/nitidez locais aumentam o detalhe fino só onde há máscara.
    let detail = |img: &RgbImage, x0: u32, y0: u32| {
        let mut acc = 0.0;
        for y in y0..y0 + 30 {
            for x in x0..x0 + 30 {
                acc += (img.get_pixel(x + 1, y)[1] as f32 - img.get_pixel(x, y)[1] as f32).abs();
            }
        }
        acc
    };
    let sharp = out(
        &src,
        &local(LocalAdjust { texture: 80.0, sharpness: 80.0, ..Default::default() }, radial()),
    );
    assert!(detail(&sharp, 285, 85) > detail(&base, 285, 85) * 1.15);
    assert_eq!(detail(&sharp, 20, 300), detail(&base, 20, 300));
    let nr = out(
        &src,
        &local(LocalAdjust { noise: 100.0, clarity: -30.0, ..Default::default() }, radial()),
    );
    assert!(detail(&nr, 285, 85) < detail(&base, 285, 85) * 0.85);
}

#[test]
fn masks_follow_the_content_when_the_geometry_changes() {
    let src = scene();
    let s = local(LocalAdjust { exposure: 1.5, ..Default::default() }, radial());
    // Recorte da metade direita: o centro da máscara (0,5; 0,25) passa para (0; 0,25).
    let mut cropped = s.clone();
    cropped.geometry.crop = Crop { x: 0.5, y: 0.0, w: 0.5, h: 1.0 };
    let mut base = Settings::default();
    base.geometry.crop = cropped.geometry.crop;
    let (a, b) = (
        render_rgb(&src, &cropped, 400, Quality::Final),
        render_rgb(&src, &base, 400, Quality::Final),
    );
    assert_eq!(a.dimensions(), (300, 400));
    assert!(mean(&a, 0, 90, 8, 110)[1] > mean(&b, 0, 90, 8, 110)[1] + 20.0);
    assert_eq!(mean(&a, 200, 300, 280, 340), mean(&b, 200, 300, 280, 340));
    // Sobreposição da máscara (interface): vermelho só dentro.
    let r = render_with_overlay(
        &src,
        &s,
        &RenderOptions {
            max_width: 600,
            max_height: 600,
            quality: Quality::Fast,
            uncropped: false,
        },
        Some(0),
    );
    let px = |x: u32, y: u32| {
        let i = ((y * r.width + x) * 4) as usize;
        [r.rgba[i], r.rgba[i + 1], r.rgba[i + 2]]
    };
    let inside = px(300, 100);
    assert!(inside[0] as i32 - inside[1] as i32 > 60, "{inside:?}");
    let outside = px(30, 320);
    assert!((outside[0] as i32 - outside[1] as i32).abs() < 5);
    // Máscara desligada: sem efeito; de IA em falta: sem efeito nem pânico.
    let mut off = s.clone();
    off.masks[0].enabled = false;
    assert_eq!(out(&src, &off), out(&src, &Settings::default()));
    let missing = local(
        LocalAdjust { exposure: 2.0, ..Default::default() },
        MaskShape::Raster { kind: "person".into(), raster: "nao-existe".into(), feather: 0.0 },
    );
    assert_eq!(out(&src, &missing), out(&src, &Settings::default()));
}

#[test]
fn heal_clone_and_object_removal() {
    // Fundo claro à direita, escuro à esquerda, um "objeto" escuro no lado claro.
    let img = RgbImage::from_fn(600, 400, |x, y| {
        let base = if x < 300 { 70 } else { 200 };
        let obj = (420..460).contains(&x) && (180..220).contains(&y);
        let v = if obj { 30 } else { base + ((x + y) % 5) as u8 };
        Rgb([v, v, v])
    });
    let src = Source::from_srgb(&img, 600, 400);
    let long = 600.0;
    // Clonar a partir do lado escuro → mancha escura; heal → igual ao fundo claro.
    let spot = |heal: bool| Spot {
        x: 440.0 / 600.0,
        y: 0.5,
        r: 30.0 / long,
        sx: 150.0 / 600.0,
        sy: 0.5,
        heal,
        feather: 30.0,
        opacity: 100.0,
    };
    let clone = out(
        &src,
        &Settings {
            retouch: Retouch { spots: vec![spot(false)], ..Default::default() },
            ..Default::default()
        },
    );
    let heal = out(
        &src,
        &Settings {
            retouch: Retouch { spots: vec![spot(true)], ..Default::default() },
            ..Default::default()
        },
    );
    let c = mean(&clone, 432, 192, 448, 208)[1];
    let h = mean(&heal, 432, 192, 448, 208)[1];
    assert!(c < 120.0, "clone traz o escuro: {c}");
    assert!((h - 201.0).abs() < 12.0, "heal ajusta à volta: {h}");
    // Remoção de objeto com pincel (origem acima, no mesmo fundo).
    let patch = Patch {
        stroke: BrushStroke {
            points: vec![[0.72, 0.5], [0.75, 0.5]],
            size: 30.0 / long,
            feather: 30.0,
            flow: 100.0,
            erase: false,
        },
        dx: 0.0,
        dy: -0.3,
        heal: true,
        opacity: 100.0,
    };
    let removed = out(
        &src,
        &Settings {
            retouch: Retouch { patches: vec![patch], ..Default::default() },
            ..Default::default()
        },
    );
    let m = mean(&removed, 432, 192, 448, 208)[1];
    assert!((m - 201.0).abs() < 10.0, "objeto removido: {m}");
    assert!(mean(&out(&src, &Settings::default()), 432, 192, 448, 208)[1] < 60.0);
}

/// Rosto sintético com manchas (banda média), poros (banda fina), olheiras, dentes amarelos.
fn face() -> (Source, FaceRegions) {
    let (w, h) = (600u32, 600u32);
    let img = RgbImage::from_fn(w, h, |x, y| {
        let (xf, yf) = (x as f32, y as f32);
        let in_face = ((xf - 300.0) / 150.0).powi(2) + ((yf - 300.0) / 200.0).powi(2) < 1.0;
        if !in_face {
            return Rgb([40, 50, 60]);
        }
        let pore = if (x + y * 3) % 4 == 0 { 6.0 } else { 0.0 };
        let blotch = ((xf / 18.0).sin() * (yf / 22.0).cos()) * 14.0;
        let mut c = [200.0 + blotch + pore, 150.0 + blotch * 0.6 + pore, 125.0 + pore];
        if (220.0..280.0).contains(&yf.max(0.0))
            && ((230.0..280.0).contains(&xf) || (320.0..370.0).contains(&xf))
            && yf > 255.0
        {
            c = [140.0, 100.0, 105.0]; // olheiras
        }
        if (360.0..400.0).contains(&yf) && (260.0..340.0).contains(&xf) {
            c = [225.0, 215.0, 150.0]; // dentes amarelados
        }
        Rgb(c.map(|v| v.clamp(0.0, 255.0) as u8))
    });
    let n = |p: [f32; 2]| [p[0] / w as f32, p[1] / h as f32];
    let oval: Vec<[f32; 2]> = (0..36)
        .map(|k| {
            let a = k as f32 / 36.0 * std::f32::consts::TAU;
            n([300.0 + 150.0 * a.cos(), 300.0 + 200.0 * a.sin()])
        })
        .collect();
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
        vec![n([x0, y0]), n([x1, y0]), n([x1, y1]), n([x0, y1])]
    };
    let regions = FaceRegions {
        oval,
        left_eye: rect(230.0, 225.0, 280.0, 250.0),
        right_eye: rect(320.0, 225.0, 370.0, 250.0),
        lips_outer: rect(250.0, 350.0, 350.0, 410.0),
        lips_inner: rect(260.0, 360.0, 340.0, 400.0),
        under_left: rect(228.0, 256.0, 282.0, 280.0),
        under_right: rect(318.0, 256.0, 372.0, 280.0),
        left_iris: [255.0 / 600.0, 237.0 / 600.0, 10.0 / 600.0],
        right_iris: [345.0 / 600.0, 237.0 / 600.0, 10.0 / 600.0],
        ..Default::default()
    };
    (Source::from_srgb(&img, w, h), regions)
}

#[test]
fn portrait_retouch_is_subtle_and_local() {
    let (src, f) = face();
    let base = out(&src, &Settings::default());
    let zero = Settings {
        portrait: Portrait { faces: vec![f.clone()], ..Default::default() },
        ..Default::default()
    };
    assert_eq!(out(&src, &zero), base, "quantidades a 0 = original");
    let p = Settings {
        portrait: Portrait {
            skin: 60.0,
            dark_circles: 60.0,
            shine: 40.0,
            teeth: 60.0,
            eyes: 40.0,
            faces: vec![f],
        },
        ..Default::default()
    };
    let r = out(&src, &p);
    // Pele: manchas (variação a média escala) atenuadas; poros (diferença entre vizinhos) mantidos.
    let blotch = |img: &RgbImage| {
        let v: Vec<f32> = (0..40).map(|k| mean(img, 330 + k, 140, 331 + k, 141)[1]).collect();
        let m = v.iter().sum::<f32>() / v.len() as f32;
        v.iter().map(|x| (x - m).abs()).sum::<f32>()
    };
    let pores = |img: &RgbImage| {
        (0..60)
            .map(|k| {
                (img.get_pixel(330 + k, 150)[1] as f32 - img.get_pixel(331 + k, 150)[1] as f32)
                    .abs()
            })
            .sum::<f32>()
    };
    assert!(blotch(&r) < blotch(&base) * 0.75, "manchas {} → {}", blotch(&base), blotch(&r));
    assert!(pores(&r) > pores(&base) * 0.6, "poros mantidos {} → {}", pores(&base), pores(&r));
    // Olheiras mais claras (mas não mais claras que a pele).
    let (u0, u1) = (mean(&base, 240, 262, 270, 274)[1], mean(&r, 240, 262, 270, 274)[1]);
    assert!(u1 > u0 + 8.0 && u1 < mean(&base, 330, 140, 370, 160)[1] + 5.0, "{u0} → {u1}");
    // Dentes: menos amarelo.
    let (t0, t1) = (mean(&base, 280, 370, 320, 390), mean(&r, 280, 370, 320, 390));
    assert!(
        (t1[0] + t1[1]) / 2.0 - t1[2] < ((t0[0] + t0[1]) / 2.0 - t0[2]) * 0.75,
        "{t0:?} → {t1:?}"
    );
    // Fora do rosto e contorno: nada muda (sem deformação).
    assert_eq!(mean(&r, 10, 10, 60, 60), mean(&base, 10, 10, 60, 60));
    assert_eq!(r.get_pixel(300, 95), base.get_pixel(300, 95), "contorno no mesmo sítio");
    assert_eq!(r.get_pixel(152, 300), base.get_pixel(152, 300));
}

#[test]
fn overlay_does_not_change_the_histogram() {
    let src = scene();
    let s = local(LocalAdjust::default(), radial());
    let o =
        RenderOptions { max_width: 300, max_height: 300, quality: Quality::Fast, uncropped: false };
    let a = render(&src, &s, &o);
    let b = render_with_overlay(&src, &s, &o, Some(0));
    assert_eq!(a.histogram, b.histogram);
    assert_ne!(a.rgba, b.rgba);
}
