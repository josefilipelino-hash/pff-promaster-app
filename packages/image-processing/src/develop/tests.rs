//! Testes de renderização: cada ajuste tem o efeito esperado, medido em números, e a
//! receita neutra reproduz o original.

use super::color::{eotf, mul3, oetf, wb_matrix, REC2020_TO_SRGB};
use super::*;
use image::{Rgb, RgbImage};

/// Cena de teste 600×400: rampa de cinzentos (cima), manchas de cor (meio), textura fina
/// e uma zona escura e outra clara (baixo).
fn scene() -> RgbImage {
    RgbImage::from_fn(600, 400, |x, y| {
        if y < 100 {
            let v = (x * 255 / 599) as u8;
            Rgb([v, v, v])
        } else if y < 200 {
            const C: [[u8; 3]; 6] = [
                [200, 40, 40],
                [230, 140, 40],
                [220, 210, 50],
                [50, 170, 60],
                [40, 90, 200],
                [150, 60, 170],
            ];
            Rgb(C[(x / 100) as usize])
        } else if y < 300 {
            let n = (((x * 7919) ^ (y * 104_729)) % 61) as i32 - 30;
            let b = 128 + if (x / 6 + y / 6) % 2 == 0 { 30 } else { -30 };
            let v = (b + n).clamp(0, 255) as u8;
            Rgb([v, v, v])
        } else if x < 300 {
            Rgb([25, 25, 28])
        } else {
            Rgb([235, 235, 230])
        }
    })
}

fn src() -> Source {
    Source::from_srgb(&scene(), 6000, 4000)
}

fn opts(q: Quality) -> RenderOptions {
    RenderOptions { max_width: 600, max_height: 400, quality: q, uncropped: false }
}

fn px(r: &Rendered, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * r.width + x) * 4) as usize;
    [r.rgba[i], r.rgba[i + 1], r.rgba[i + 2]]
}

fn mean_region(r: &Rendered, x0: u32, y0: u32, x1: u32, y1: u32) -> [f32; 3] {
    let mut acc = [0.0f32; 3];
    let mut n = 0.0;
    for y in y0..y1 {
        for x in x0..x1 {
            let p = px(r, x, y);
            for i in 0..3 {
                acc[i] += p[i] as f32;
            }
            n += 1.0;
        }
    }
    acc.map(|a| a / n)
}

fn lum(p: [f32; 3]) -> f32 {
    0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]
}

fn render_with(s: &Settings) -> Rendered {
    render(&src(), s, &opts(Quality::Final))
}

#[test]
fn neutral_recipe_reproduces_the_original() {
    let img = scene();
    let r = render(&src(), &Settings::default(), &opts(Quality::Final));
    assert_eq!((r.width, r.height), (600, 400));
    let mut max = 0i32;
    for (a, b) in r.rgba.chunks_exact(4).zip(img.pixels()) {
        for c in 0..3 {
            max = max.max((a[c] as i32 - b[c] as i32).abs());
        }
    }
    assert!(max <= 1, "diferença máxima {max} níveis");
    let fast = render(&src(), &Settings::default(), &opts(Quality::Fast));
    assert!(fast.rgba.chunks_exact(4).zip(img.pixels()).all(|(a, b)| (0..3).all(|c| (a[c]
        as i32
        - b[c] as i32)
        .abs()
        <= 1)));
}

#[test]
fn exposure_plus_one_doubles_linear_light() {
    let s = Settings { light: Light { exposure: 1.0, ..Default::default() }, ..Default::default() };
    let r = render_with(&s);
    // Cinzento 100 (linear 0,127) → 0,254 linear → ~137.
    let before = eotf(100.0 / 255.0);
    let x = (100.0 / 255.0 * 599.0) as u32;
    let after = eotf(px(&r, x, 50)[0] as f32 / 255.0);
    assert!((after / before - 2.0).abs() < 0.08, "{}", after / before);
}

#[test]
fn white_balance_warms_and_the_picker_neutralizes() {
    let warm = render_with(&Settings {
        wb: WhiteBalance { temperature: 40.0, tint: 0.0 },
        ..Default::default()
    });
    let g = mean_region(&warm, 280, 40, 320, 60);
    assert!(g[0] > g[2] + 8.0, "mais quente: {g:?}");
    // Uma fotografia com dominante azul: o seletor torna o cinzento neutro.
    let tinted = {
        let base = scene();
        let m = wb_matrix(-35.0, 10.0);
        let rgb: RgbImage = RgbImage::from_fn(600, 400, |x, y| {
            let p = base.get_pixel(x, y);
            let lin = mul3(
                &super::color::SRGB_TO_REC2020,
                [eotf(p[0] as f32 / 255.0), eotf(p[1] as f32 / 255.0), eotf(p[2] as f32 / 255.0)],
            );
            let o = mul3(&REC2020_TO_SRGB, mul3(&m, lin));
            Rgb(o.map(|c| (super::color::oetf(c.clamp(0.0, 1.0)) * 255.0).round() as u8))
        });
        Source::from_srgb(&rgb, 6000, 4000)
    };
    let sample = sample_for_white_balance(&tinted, &Settings::default(), 0.5, 0.125).unwrap();
    let (t, n) = color::neutralize(sample);
    let fixed = render(
        &tinted,
        &Settings { wb: WhiteBalance { temperature: t, tint: n }, ..Default::default() },
        &opts(Quality::Final),
    );
    let g = mean_region(&fixed, 280, 40, 320, 60);
    assert!(
        (g[0] - g[2]).abs() < 4.0 && (g[1] - g[0]).abs() < 4.0,
        "neutro: {g:?} (t={t}, tint={n})"
    );
}

#[test]
fn tone_controls_move_the_right_zones() {
    let base = render_with(&Settings::default());
    let dark = |r: &Rendered| lum(mean_region(r, 50, 320, 250, 390));
    let bright = |r: &Rendered| lum(mean_region(r, 350, 320, 550, 390));
    let sh = render_with(&Settings {
        light: Light { shadows: 80.0, ..Default::default() },
        ..Default::default()
    });
    assert!(dark(&sh) > dark(&base) + 10.0, "sombras levantadas: {} → {}", dark(&base), dark(&sh));
    assert!((bright(&sh) - bright(&base)).abs() < 4.0, "realces quase iguais");
    let hl = render_with(&Settings {
        light: Light { highlights: -80.0, ..Default::default() },
        ..Default::default()
    });
    assert!(
        bright(&hl) < bright(&base) - 6.0,
        "realces recuperados: {} → {}",
        bright(&base),
        bright(&hl)
    );
    assert!((dark(&hl) - dark(&base)).abs() < 3.0);
    let c = render_with(&Settings {
        light: Light { contrast: 60.0, ..Default::default() },
        ..Default::default()
    });
    assert!(dark(&c) < dark(&base) && bright(&c) > bright(&base), "contraste");
    let w = render_with(&Settings {
        light: Light { whites: 60.0, blacks: -60.0, ..Default::default() },
        ..Default::default()
    });
    assert!(bright(&w) > bright(&base) && dark(&w) < dark(&base));
}

#[test]
fn saturation_vibrance_and_hsl() {
    let chroma = |p: [f32; 3]| p[0].max(p[1]).max(p[2]) - p[0].min(p[1]).min(p[2]);
    let base = render_with(&Settings::default());
    let gray = render_with(&Settings {
        color: Color { saturation: -100.0, vibrance: 0.0 },
        ..Default::default()
    });
    assert!(chroma(mean_region(&gray, 20, 120, 80, 180)) < 2.0, "saturação −100 = cinzento");
    // Vibração: mexe mais na cor pouco saturada (laranja suave) do que na muito saturada?
    let vib = render_with(&Settings {
        color: Color { saturation: 0.0, vibrance: 80.0 },
        ..Default::default()
    });
    let blue0 = chroma(mean_region(&base, 420, 120, 480, 180));
    let blue1 = chroma(mean_region(&vib, 420, 120, 480, 180));
    assert!(blue1 > blue0, "vibração aumenta a cor");
    // HSL: luminância do azul −100 escurece o azul e não o vermelho.
    let mut h = Hsl::default();
    h.luminance[5] = -100.0;
    let hsl = render_with(&Settings { hsl: h, ..Default::default() });
    assert!(
        lum(mean_region(&hsl, 420, 120, 480, 180))
            < lum(mean_region(&base, 420, 120, 480, 180)) - 10.0
    );
    let red = |r: &Rendered| mean_region(r, 20, 120, 80, 180);
    assert!((lum(red(&hsl)) - lum(red(&base))).abs() < 2.0, "vermelho intacto");
    // Matiz do vermelho → aproxima do laranja (mais verde).
    let mut h2 = Hsl::default();
    h2.hue[0] = 100.0;
    let hue = render_with(&Settings { hsl: h2, ..Default::default() });
    assert!(red(&hue)[1] > red(&base)[1] + 15.0, "{:?} → {:?}", red(&base), red(&hue));
}

#[test]
fn curves_parametric_points_and_channels() {
    let base = render_with(&Settings::default());
    let mid = |r: &Rendered| px(r, 300, 50);
    let inv = Settings {
        curve: Curve { rgb: vec![[0.0, 1.0], [1.0, 0.0]], ..Default::default() },
        ..Default::default()
    };
    let r = render_with(&inv);
    assert!(
        px(&r, 10, 50)[0] >= 245 && px(&r, 590, 50)[0] <= 10,
        "curva invertida: {:?} {:?}",
        px(&r, 10, 50),
        px(&r, 590, 50)
    );
    let red_up = Settings {
        curve: Curve { red: vec![[0.0, 0.0], [0.5, 0.7], [1.0, 1.0]], ..Default::default() },
        ..Default::default()
    };
    let r = render_with(&red_up);
    assert!(
        mid(&r)[0] > mid(&base)[0] + 20 && (mid(&r)[2] as i32 - mid(&base)[2] as i32).abs() <= 2
    );
    let par = Settings { curve: Curve { darks: 80.0, ..Default::default() }, ..Default::default() };
    let r = render_with(&par);
    assert!(px(&r, 180, 50)[0] > px(&base, 180, 50)[0] + 5, "tons escuros levantados");
}

#[test]
fn geometry_crop_rotate_flip_perspective() {
    let s = Settings {
        geometry: Geometry { quarter_turns: 1, ..Default::default() },
        ..Default::default()
    };
    let r = render(
        &src(),
        &s,
        &RenderOptions {
            max_width: 1000,
            max_height: 1000,
            quality: Quality::Final,
            uncropped: false,
        },
    );
    assert_eq!((r.width, r.height), (667, 1000));
    // Rodar 90° horário: a rampa (topo) passa a ser a coluna da direita.
    assert!(
        px(&r, 650, 980)[0] > 200 && px(&r, 650, 20)[0] < 50,
        "{:?} {:?}",
        px(&r, 650, 980),
        px(&r, 650, 20)
    );
    let crop = Settings {
        geometry: Geometry { crop: Crop { x: 0.5, y: 0.0, w: 0.5, h: 0.25 }, ..Default::default() },
        ..Default::default()
    };
    let r = render(
        &src(),
        &crop,
        &RenderOptions {
            max_width: 300,
            max_height: 300,
            quality: Quality::Final,
            uncropped: false,
        },
    );
    assert_eq!((r.width, r.height), (300, 100));
    assert!(
        px(&r, 2, 50)[0] >= 125 && px(&r, 2, 50)[0] <= 135,
        "começa a meio da rampa: {:?}",
        px(&r, 2, 50)
    );
    let flip = Settings {
        geometry: Geometry { flip_horizontal: true, ..Default::default() },
        ..Default::default()
    };
    let r = render_with(&flip);
    assert!(px(&r, 5, 50)[0] > 245);
    // Endireitar 8°: sem cantos pretos.
    let st = Settings {
        geometry: Geometry { straighten: 8.0, ..Default::default() },
        ..Default::default()
    };
    let r = render_with(&st);
    for (x, y) in [(0, 0), (599, 0), (0, 399), (599, 399)] {
        let p = px(&r, x, y);
        assert!(!(p == [0, 0, 0]) || y > 300, "canto {x},{y} vazio");
    }
    let pv = Settings {
        geometry: Geometry { vertical: 60.0, ..Default::default() },
        ..Default::default()
    };
    let r = render_with(&pv);
    assert_ne!(
        px(&r, 5, 50),
        px(&render_with(&Settings::default()), 5, 50),
        "perspetiva altera o topo"
    );
}

#[test]
fn auto_straighten_corrects_a_tilted_horizon() {
    // Horizonte inclinado 4°: céu claro em cima, mar escuro em baixo, com textura.
    let a = 4f32.to_radians();
    let img = RgbImage::from_fn(900, 600, |x, y| {
        let (cx, cy) = (x as f32 - 450.0, y as f32 - 300.0);
        let d = -cx * a.sin() + cy * a.cos();
        let n = ((x * 31 + y * 17) % 13) as i32 - 6;
        let v = if d < 0.0 { 200 } else { 60 } + n;
        Rgb([v as u8, v as u8, (v + 10).min(255) as u8])
    });
    let s = Source::from_srgb(&img, 900, 600);
    let angle = auto_straighten(&s, &Settings::default()).expect("deteta o horizonte");
    let fixed = Settings {
        geometry: Geometry { straighten: angle, ..Default::default() },
        ..Default::default()
    };
    let out = render_rgb(&s, &fixed, 900, Quality::Final);
    let tilt = momento_ai_analysis::metrics::technical(&out).tilt_degrees.unwrap_or(0.0);
    assert!(tilt.abs() < 0.8, "ângulo {angle}, inclinação final {tilt}");
}

#[test]
fn lens_corrections_move_pixels_and_brighten_corners() {
    let base = render_with(&Settings::default());
    let vig = render_with(&Settings {
        lens: Lens { vignetting: 80.0, ..Default::default() },
        ..Default::default()
    });
    assert!(
        lum(mean_region(&vig, 0, 300, 30, 330)) > lum(mean_region(&base, 0, 300, 30, 330)) + 3.0,
        "cantos mais claros"
    );
    assert!(
        (lum(mean_region(&vig, 290, 190, 310, 210)) - lum(mean_region(&base, 290, 190, 310, 210)))
            .abs()
            < 3.0,
        "centro igual"
    );
    let dist = render_with(&Settings {
        lens: Lens { distortion: 60.0, ..Default::default() },
        ..Default::default()
    });
    assert_ne!(px(&dist, 30, 60), px(&base, 30, 60));
    let ca = render_with(&Settings {
        lens: Lens { ca_red: 100.0, ..Default::default() },
        ..Default::default()
    });
    // Nas arestas entre manchas, o vermelho muda muito mais do que o verde e o azul (estes só
    // mudam pela conversão Rec. 2020 → sRGB, que mistura ligeiramente os canais).
    let mut d = [0.0f32; 3];
    for y in 105..195 {
        for x in [100u32, 200, 400, 500] {
            for dx in 0..3 {
                let (a, b) = (px(&ca, x - 1 + dx, y), px(&base, x - 1 + dx, y));
                for c in 0..3 {
                    d[c] += (a[c] as f32 - b[c] as f32).abs();
                }
            }
        }
    }
    assert!(d[0] > d[1] * 3.0 && d[0] > d[2] * 3.0, "{d:?}");
}

#[test]
fn detail_sharpening_and_noise_reduction() {
    let tex = |r: &Rendered| {
        let mut v = Vec::new();
        for y in 210..290 {
            for x in 10..590 {
                let c = px(r, x, y)[1] as f32;
                let l = px(r, x - 1, y)[1] as f32;
                let rr = px(r, x + 1, y)[1] as f32;
                v.push((2.0 * c - l - rr).abs());
            }
        }
        v.iter().sum::<f32>() / v.len() as f32
    };
    // Render a 600 px de um “original” de 600 px: escala 1, a nitidez é visível.
    let s1 = Source::from_srgb(&scene(), 600, 400);
    let o = opts(Quality::Final);
    let base = render(&s1, &Settings::default(), &o);
    let sharp = render(
        &s1,
        &Settings {
            detail: Detail { sharpen_amount: 100.0, ..Default::default() },
            ..Default::default()
        },
        &o,
    );
    assert!(tex(&sharp) > tex(&base) * 1.15, "nitidez: {} → {}", tex(&base), tex(&sharp));
    let nr = render(
        &s1,
        &Settings {
            detail: Detail { noise_luminance: 80.0, ..Default::default() },
            ..Default::default()
        },
        &o,
    );
    assert!(tex(&nr) < tex(&base) * 0.9, "redução de ruído: {} → {}", tex(&base), tex(&nr));
    // Máscara 100: zonas lisas ficam iguais.
    let masked = render(
        &s1,
        &Settings {
            detail: Detail { sharpen_amount: 150.0, sharpen_masking: 100.0, ..Default::default() },
            ..Default::default()
        },
        &o,
    );
    assert_eq!(px(&masked, 150, 350), px(&base, 150, 350));
    // Ruído de cor: uma mancha colorida com ruído cromático fica mais uniforme.
    let noisy = RgbImage::from_fn(200, 200, |x, y| {
        let n = (((x * 7919) ^ (y * 104_729)) % 41) as i32 - 20;
        Rgb([(120 + n).clamp(0, 255) as u8, 120, (120 - n).clamp(0, 255) as u8])
    });
    let ns = Source::from_srgb(&noisy, 200, 200);
    let o2 = RenderOptions {
        max_width: 200,
        max_height: 200,
        quality: Quality::Final,
        uncropped: false,
    };
    let var = |r: &Rendered| {
        let v: Vec<f32> = r.rgba.chunks_exact(4).map(|p| p[0] as f32 - p[2] as f32).collect();
        let m = v.iter().sum::<f32>() / v.len() as f32;
        v.iter().map(|x| (x - m).powi(2)).sum::<f32>() / v.len() as f32
    };
    let a = render(&ns, &Settings::default(), &o2);
    let b = render(
        &ns,
        &Settings {
            detail: Detail { noise_color: 100.0, ..Default::default() },
            ..Default::default()
        },
        &o2,
    );
    assert!(var(&b) < var(&a) * 0.5, "{} → {}", var(&a), var(&b));
}

#[test]
fn effects_clarity_dehaze_vignette_grain() {
    let base = render_with(&Settings::default());
    let vig = render_with(&Settings {
        effects: Effects { vignette: -80.0, ..Default::default() },
        ..Default::default()
    });
    // Canto superior direito (ponta clara da rampa).
    assert!(
        lum(mean_region(&vig, 580, 0, 600, 20)) < lum(mean_region(&base, 580, 0, 600, 20)) - 10.0
    );
    assert_eq!(px(&vig, 300, 200), px(&base, 300, 200), "centro intacto");
    let g1 = render_with(&Settings {
        effects: Effects { grain: 60.0, ..Default::default() },
        ..Default::default()
    });
    let g2 = render_with(&Settings {
        effects: Effects { grain: 60.0, ..Default::default() },
        ..Default::default()
    });
    assert_eq!(g1.rgba, g2.rgba, "grão determinístico");
    assert_ne!(g1.rgba, base.rgba);
    // Clareza aumenta o contraste local (variação na textura); negativa suaviza.
    let spread = |r: &Rendered| {
        let v: Vec<f32> = (210..290)
            .flat_map(|y| (10..590).map(move |x| (x, y)))
            .map(|(x, y)| px(r, x, y)[1] as f32)
            .collect();
        let m = v.iter().sum::<f32>() / v.len() as f32;
        v.iter().map(|x| (x - m).powi(2)).sum::<f32>() / v.len() as f32
    };
    let cl = render_with(&Settings {
        effects: Effects { clarity: 80.0, ..Default::default() },
        ..Default::default()
    });
    let cn = render_with(&Settings {
        effects: Effects { clarity: -80.0, ..Default::default() },
        ..Default::default()
    });
    assert!(spread(&cl) > spread(&base) && spread(&cn) < spread(&base));
    let tx = render_with(&Settings {
        effects: Effects { texture: 80.0, ..Default::default() },
        ..Default::default()
    });
    assert!(spread(&tx) > spread(&base));
    // Dehaze numa imagem enevoada (pouco contraste, véu claro) aumenta o contraste.
    let hazy = RgbImage::from_fn(300, 200, |x, y| {
        let v = 150 + ((x / 10 + y / 10) % 2) as u8 * 25;
        Rgb([v, v, v + 10])
    });
    let hs = Source::from_srgb(&hazy, 300, 200);
    let o2 = RenderOptions {
        max_width: 300,
        max_height: 200,
        quality: Quality::Final,
        uncropped: false,
    };
    let c0 = render(&hs, &Settings::default(), &o2);
    let c1 = render(
        &hs,
        &Settings { effects: Effects { dehaze: 80.0, ..Default::default() }, ..Default::default() },
        &o2,
    );
    let span = |r: &Rendered| {
        let v: Vec<u8> = r.rgba.chunks_exact(4).map(|p| p[1]).collect();
        *v.iter().max().unwrap() as i32 - *v.iter().min().unwrap() as i32
    };
    assert!(span(&c1) > span(&c0) + 10, "{} → {}", span(&c0), span(&c1));
}

#[test]
fn histogram_counts_every_pixel() {
    let r = render_with(&Settings::default());
    for c in 0..3 {
        assert_eq!(r.histogram[c].iter().sum::<u32>(), 600 * 400);
    }
    // A rampa dá contagens em quase todas as classes.
    assert!(r.histogram[0].iter().filter(|&&n| n > 0).count() > 200);
}

#[test]
fn fast_and_final_previews_look_the_same() {
    let s = Settings {
        light: Light {
            exposure: 0.4,
            contrast: 30.0,
            highlights: -40.0,
            shadows: 40.0,
            ..Default::default()
        },
        effects: Effects { clarity: 30.0, dehaze: 20.0, ..Default::default() },
        color: Color { vibrance: 30.0, saturation: 0.0 },
        ..Default::default()
    };
    let a = render(&src(), &s, &opts(Quality::Fast));
    let b = render(&src(), &s, &opts(Quality::Final));
    let diff: f32 =
        a.rgba.iter().zip(&b.rgba).map(|(x, y)| (*x as f32 - *y as f32).abs()).sum::<f32>()
            / a.rgba.len() as f32;
    assert!(diff < 2.0, "diferença média {diff}");
}

#[test]
fn recipe_roundtrip_and_merge() {
    let s = Settings {
        light: Light { exposure: 0.7, ..Default::default() },
        geometry: Geometry { straighten: 2.0, ..Default::default() },
        ..Default::default()
    };
    let r = s.to_recipe(&Default::default());
    assert_eq!(
        r.blocks.keys().cloned().collect::<Vec<_>>(),
        vec!["geometry", "light"],
        "só os blocos alterados"
    );
    assert_eq!(Settings::from_recipe(&r), s);
    assert!(Settings::from_recipe(&Default::default()).is_identity());
    let other =
        Settings { light: Light { contrast: 20.0, ..Default::default() }, ..Default::default() }
            .to_recipe(&Default::default());
    let merged = merge_blocks(&r, &other, &["light".to_string()]);
    let m = Settings::from_recipe(&merged);
    assert_eq!(
        (m.light.contrast, m.light.exposure, m.geometry.straighten),
        (20.0, 0.0, 2.0),
        "colar só a luz"
    );
}

#[test]
fn performance_interactive_and_final() {
    // Fonte de 6 MP (como um RAW a meia resolução ou um JPEG grande).
    let img = RgbImage::from_fn(3000, 2000, |x, y| {
        Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    });
    let t = std::time::Instant::now();
    let s = Source::from_srgb(&img, 6000, 4000);
    let load = t.elapsed();
    let busy = Settings {
        light: Light {
            exposure: 0.5,
            contrast: 20.0,
            highlights: -30.0,
            shadows: 30.0,
            ..Default::default()
        },
        effects: Effects { clarity: 20.0, dehaze: 10.0, vignette: -20.0, ..Default::default() },
        color: Color { vibrance: 20.0, saturation: 5.0 },
        detail: Detail {
            sharpen_amount: 40.0,
            noise_luminance: 20.0,
            noise_color: 25.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let fast_o = RenderOptions {
        max_width: 1024,
        max_height: 1024,
        quality: Quality::Fast,
        uncropped: false,
    };
    let _ = render(&s, &busy, &fast_o); // aquecimento
    let t = std::time::Instant::now();
    for _ in 0..5 {
        let _ = render(&s, &busy, &fast_o);
    }
    let fast = t.elapsed() / 5;
    let t = std::time::Instant::now();
    let _ = render(
        &s,
        &busy,
        &RenderOptions {
            max_width: 2048,
            max_height: 2048,
            quality: Quality::Final,
            uncropped: false,
        },
    );
    let fin = t.elapsed();
    // Caso típico: arrastar a exposição.
    let expo =
        Settings { light: Light { exposure: 0.8, ..Default::default() }, ..Default::default() };
    let t = std::time::Instant::now();
    for _ in 0..5 {
        let _ = render(&s, &expo, &fast_o);
    }
    let ex = t.elapsed() / 5;
    eprintln!("fonte 6 MP: {load:.0?} · rápido 1024 px (tudo): {fast:.0?} · rápido 1024 px (exposição): {ex:.0?} · final 2048 px: {fin:.0?}");
    if !cfg!(debug_assertions) {
        assert!(fast.as_millis() < 250, "{fast:?}");
    }
}

#[test]
fn black_white_mix_grading_and_lut() {
    // P&B: cinzento; mix do vermelho clareia a mancha vermelha e não mexe no azul.
    let bw = Settings { bw: BlackWhite { enabled: true, mix: [0.0; 8] }, ..Default::default() };
    let r0 = render_with(&bw);
    let red0 = mean_region(&r0, 10, 120, 90, 180);
    assert!((red0[0] - red0[2]).abs() < 1.5, "cinzento: {red0:?}");
    let mut mix = [0.0; 8];
    mix[0] = 80.0;
    let r1 = render_with(&Settings { bw: BlackWhite { enabled: true, mix }, ..Default::default() });
    let red1 = mean_region(&r1, 10, 120, 90, 180);
    let blue0 = mean_region(&r0, 410, 120, 490, 180);
    let blue1 = mean_region(&r1, 410, 120, 490, 180);
    assert!(red1[1] > red0[1] + 15.0, "vermelho mais claro: {red0:?} → {red1:?}");
    assert!((blue1[1] - blue0[1]).abs() < 2.0);
    // Tonalização: sombras azuis, realces quentes; o brilho quase não muda.
    let g = Settings {
        grading: Grading {
            shadows_hue: 220.0,
            shadows_sat: 60.0,
            highlights_hue: 40.0,
            highlights_sat: 60.0,
            balance: 0.0,
        },
        ..Default::default()
    };
    let rg = render_with(&g);
    let base = render_with(&Settings::default());
    let dark = mean_region(&rg, 20, 320, 280, 390);
    let light = mean_region(&rg, 320, 320, 580, 390);
    assert!(dark[2] > dark[0] + 3.0, "sombras azuladas: {dark:?}");
    assert!(light[0] > light[2] + 3.0, "realces quentes: {light:?}");
    let (l0, l1) = (lum(mean_region(&base, 320, 320, 580, 390)), lum(light));
    assert!((l0 - l1).abs() < 6.0, "{l0} vs {l1}");
    // LUT registada: inverter as cores (LUT 2³) com 100 % e 50 %.
    let mut s = String::from("LUT_3D_SIZE 2\n");
    for b in 0..2 {
        for gg in 0..2 {
            for r in 0..2 {
                s += &format!("{} {} {}\n", 1 - r, 1 - gg, 1 - b);
            }
        }
    }
    lut::register("teste-inverter", lut::parse_cube(&s).unwrap());
    let inv = Settings {
        lut: LutRef { id: "teste-inverter".into(), name: "Inverter".into(), amount: 100.0 },
        ..Default::default()
    };
    let ri = render_with(&inv);
    let p0 = px(&base, 300, 50);
    let p1 = px(&ri, 300, 50);
    for c in 0..3 {
        assert!((p0[c] as i32 + p1[c] as i32 - 255).abs() <= 3, "{p0:?} {p1:?}");
    }
    let half = render_with(&Settings {
        lut: LutRef { amount: 50.0, ..inv.lut.clone() },
        ..Default::default()
    });
    let ph = px(&half, 300, 50);
    assert!((ph[1] as i32 - 128).abs() < 8, "50 % de inverter ≈ cinzento médio: {ph:?}");
    // LUT desconhecida: ignorada (sem pânico).
    let unk = Settings {
        lut: LutRef { id: "nao-existe".into(), ..Default::default() },
        ..Default::default()
    };
    assert_eq!(render_with(&unk).rgba, base.rgba);
}

/// A exportação usa o mesmo pipeline que o ecrã: o render linear, convertido para sRGB
/// 8 bits, dá os mesmos bytes (±1) — com efeitos, LUT desligada, grão e vinheta.
#[test]
fn linear_render_matches_screen_render() {
    let mut s = Settings::default();
    s.light.exposure = 0.4;
    s.light.contrast = 30.0;
    s.color.saturation = 20.0;
    s.effects.vignette = -30.0;
    s.effects.grain = 20.0;
    let o = opts(Quality::Final);
    let r = render(&src(), &s, &o);
    let l = super::render_linear(&src(), &s, &o);
    assert_eq!((l.width, l.height), (r.width, r.height));
    let mut worst = 0i32;
    for (i, q) in l.px.iter().enumerate() {
        let srgb = mul3(&REC2020_TO_SRGB, *q);
        for (c, v) in srgb.iter().enumerate() {
            let b = (oetf(v.clamp(0.0, 1.0)) * 255.0).round() as i32;
            worst = worst.max((b - r.rgba[i * 4 + c] as i32).abs());
        }
    }
    assert!(worst <= 1, "diferença máxima {worst}");
}
