//! Testes da edição automática: cada fotografia recebe ajustes diferentes conforme o que foi
//! medido; intensidades ordenadas; ruído com ISO; horizonte; objetiva; olhos vermelhos;
//! manchas; recorte.

use super::super::color::{eotf, SRGB_TO_REC2020};
use super::super::{render, render_rgb, Quality, RenderOptions};
use super::*;
use image::{Rgb, RgbImage};

/// Gerador determinístico (xorshift) com ruído gaussiano.
struct Rng(u64);
impl Rng {
    fn uniform(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f32 / (1u64 << 53) as f32
    }
    fn gauss(&mut self) -> f32 {
        let (u1, u2) = (self.uniform().max(1e-7), self.uniform());
        (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
    }
}

/// Cena "normal": céu, edifício com arestas, relva, pessoas (manchas de pele).
fn scene(w: u32, h: u32) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| {
        let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
        if v < 0.2 {
            Rgb([150, 185, 225])
        } else if v < 0.7 && (0.3..0.5).contains(&u) {
            // Vestido branco e fato cinzento (zonas neutras, como num casamento).
            if u < 0.4 {
                Rgb([235, 233, 230])
            } else {
                Rgb([95, 95, 98])
            }
        } else if v < 0.7 {
            let stripe = ((x / 25 + y / 25) % 2) as u8;
            if u < 0.5 {
                Rgb([170 - stripe * 60, 160 - stripe * 60, 150 - stripe * 60])
            } else {
                Rgb([200, 120, 90])
            }
        } else {
            Rgb([70, 130, 60])
        }
    })
}

/// Escala uma imagem sRGB linearmente (simula sub/sobre-exposição) e aplica uma dominante.
fn graded(img: &RgbImage, gain: f32, tint: [f32; 3]) -> Source {
    let px: Vec<[f32; 3]> = img
        .pixels()
        .map(|p| {
            let lin =
                [eotf(p[0] as f32 / 255.0), eotf(p[1] as f32 / 255.0), eotf(p[2] as f32 / 255.0)];
            let c = [lin[0] * tint[0] * gain, lin[1] * tint[1] * gain, lin[2] * tint[2] * gain];
            mul3(&SRGB_TO_REC2020, c.map(|v| v.min(1.0)))
        })
        .collect();
    Source::from_plane(
        Plane { w: img.width() as usize, h: img.height() as usize, px },
        img.width() * 4,
        img.height() * 4,
        0.0,
    )
}

fn run(src: &Source, o: &AutoOptions) -> (Settings, AutoReport) {
    auto_edit(src, &Settings::default(), &AutoContext::default(), o)
}

#[test]
fn each_photo_gets_its_own_settings() {
    let img = scene(600, 400);
    let o = AutoOptions::default();
    let (dark, _) = run(&graded(&img, 0.18, [1.0; 3]), &o);
    let (bright, _) = run(&graded(&img, 2.5, [1.0; 3]), &o);
    let (normal, _) = run(&graded(&img, 1.0, [1.0; 3]), &o);
    assert!(dark.light.exposure > 1.0, "subexposta clareia: {}", dark.light.exposure);
    assert!(
        bright.light.exposure < 0.0 || bright.light.highlights < -20.0,
        "sobre-exposta: {:?}",
        bright.light
    );
    assert!(normal.light.exposure.abs() < dark.light.exposure.abs());
    assert_ne!(dark.light, bright.light);

    // Dominante azul → aquecer; dominante quente → arrefecer (mas menos).
    let (blue, _) = run(&graded(&img, 1.0, [0.75, 0.9, 1.25]), &o);
    let (warm, _) = run(&graded(&img, 1.0, [1.25, 1.0, 0.7]), &o);
    assert!(blue.wb.temperature > 5.0, "azul: {:?}", blue.wb);
    assert!(warm.wb.temperature < -5.0, "quente: {:?}", warm.wb);
    assert!(warm.wb.temperature.abs() < blue.wb.temperature.abs() * 1.3);

    // Convergência: depois do automático, a versão escura e a normal ficam muito mais
    // parecidas do que antes.
    let mean = |s: &Source, st: &Settings| {
        let r = render_rgb(s, st, 300, Quality::Fast);
        r.pixels().map(|p| p[1] as f32).sum::<f32>() / (r.width() * r.height()) as f32
    };
    let (ds, ns) = (graded(&img, 0.18, [1.0; 3]), graded(&img, 1.0, [1.0; 3]));
    let gap_before = mean(&ns, &Settings::default()) - mean(&ds, &Settings::default());
    let gap_after = mean(&ns, &normal) - mean(&ds, &dark);
    assert!(gap_after.abs() < gap_before.abs() * 0.5, "{gap_before} → {gap_after}");
}

#[test]
fn intensities_are_ordered() {
    let src = graded(&scene(600, 400), 0.25, [0.8, 0.9, 1.2]);
    let ev = |i: Intensity| run(&src, &AutoOptions { intensity: i, ..Default::default() }).0;
    let (vs, so, na, ba, st) = (
        ev(Intensity::VerySoft),
        ev(Intensity::Soft),
        ev(Intensity::Natural),
        ev(Intensity::Balanced),
        ev(Intensity::Strong),
    );
    let e = [
        vs.light.exposure,
        so.light.exposure,
        na.light.exposure,
        ba.light.exposure,
        st.light.exposure,
    ];
    assert!(e.windows(2).all(|w| w[0] < w[1]), "exposição {e:?}");
    assert!(st.color.vibrance > na.color.vibrance);
    // Personalizada: força 0 → só se corrigem erros grandes de exposição; nada de gosto.
    let custom = AutoOptions { intensity: Intensity::Custom, strength: 0.0, ..Default::default() };
    let (c, _) = run(&src, &custom);
    assert!(c.light.exposure < vs.light.exposure);
    assert_eq!((c.light.contrast, c.color.vibrance, c.detail.sharpen_amount), (0.0, 0.0, 0.0));
    let only_wb = AutoOptions {
        intensity: Intensity::Custom,
        strength: 100.0,
        tone: false,
        color: false,
        sharpen: false,
        denoise: false,
        lens: false,
        straighten: false,
        ..Default::default()
    };
    let (w, _) = run(&src, &only_wb);
    assert_eq!(w.light, Light::default());
    assert!(w.wb.temperature > 0.0);
    assert_eq!(w.detail, Detail::default());
}

#[test]
fn noise_is_measured_and_only_noisy_photos_get_strong_denoise() {
    let img = scene(800, 600);
    let clean = graded(&img, 1.0, [1.0; 3]);
    let mut rng = Rng(7);
    let noisy = {
        let mut s = graded(&img, 1.0, [1.0; 3]);
        let base = s.levels[0].clone();
        let px = base
            .px
            .iter()
            .map(|p| {
                // Ruído de luminância e de cor, no domínio de visualização (σ ≈ 9/255).
                let n = rng.gauss() * 0.035;
                let c = [rng.gauss() * 0.02, 0.0, rng.gauss() * 0.02];
                std::array::from_fn(|i| eotf((oetf(p[i]) + n + c[i]).clamp(0.0, 1.0)))
            })
            .collect();
        s = Source::from_plane(Plane { px, ..base }, 3200, 2400, 0.0);
        s
    };
    let (lc, cc) = measure_noise(&clean, 1.0);
    let (ln, cn) = measure_noise(&noisy, 1.0);
    assert!(lc < 1.0 && cc < 1.0, "limpa: {lc} {cc}");
    assert!((6.0..12.0).contains(&ln), "ruidosa: {ln}");
    assert!(cn > 2.0, "cor: {cn}");

    let o = AutoOptions::default();
    let (sc, rc) = run(&clean, &o);
    assert_eq!(rc.noise.level, NoiseLevel::None);
    assert_eq!((sc.detail.noise_luminance, sc.detail.ai_denoise), (0.0, 0.0));
    let ctx_high = AutoContext { iso: Some(6400), is_raw: true, ..Default::default() };
    let (sn, rn) = auto_edit(&noisy, &Settings::default(), &ctx_high, &o);
    assert!(matches!(rn.noise.level, NoiseLevel::High | NoiseLevel::VeryHigh), "{:?}", rn.noise);
    assert!(rn.noise.ai_recommended);
    assert!(sn.detail.noise_luminance > 30.0 && sn.detail.noise_color > 15.0, "{:?}", sn.detail);
    assert_eq!(sn.detail.ai_denoise, 0.0, "AI só quando pedido");
    // ISO baixo: a mesma "granulação" é tratada como textura (limite baixo).
    let ctx_low = AutoContext { iso: Some(100), ..Default::default() };
    let (sl, _) = auto_edit(&noisy, &Settings::default(), &ctx_low, &o);
    assert!(sl.detail.noise_luminance <= 12.0);
    // AI Denoise quando necessário: liga-se na ruidosa, não na limpa.
    let ai = AutoOptions { ai_denoise: AiDenoise::WhenNeeded, ..Default::default() };
    let (sa, _) = auto_edit(&noisy, &Settings::default(), &ctx_high, &ai);
    assert!(sa.detail.ai_denoise > 0.0 && sa.detail.noise_luminance < sn.detail.noise_luminance);
    assert_eq!(run(&clean, &ai).0.detail.ai_denoise, 0.0);
    // Telemóvel ao mesmo ISO conta como mais ruído.
    assert!(
        sensor_factor(Some("Apple"), Some("iPhone 15"))
            > sensor_factor(Some("NIKON CORPORATION"), Some("NIKON D810"))
    );
}

#[test]
fn sensor_table() {
    assert_eq!(sensor_factor(Some("NIKON CORPORATION"), Some("NIKON D850")), 1.0);
    assert_eq!(sensor_factor(Some("NIKON CORPORATION"), Some("NIKON D7500")), 1.3);
    assert_eq!(sensor_factor(Some("NIKON CORPORATION"), Some("NIKON Z 6_2")), 1.0);
    assert_eq!(sensor_factor(Some("Canon"), Some("Canon EOS R6m2")), 1.0);
    assert_eq!(sensor_factor(Some("Canon"), Some("Canon EOS 90D")), 1.3);
    assert_eq!(sensor_factor(Some("SONY"), Some("ILCE-7M4")), 1.0);
    assert_eq!(sensor_factor(Some("SONY"), Some("ILCE-6400")), 1.3);
    assert_eq!(sensor_factor(Some("FUJIFILM"), Some("X-T5")), 1.3);
    assert_eq!(sensor_factor(None, None), 1.2);
}

#[test]
fn horizon_is_straightened_only_when_slightly_tilted() {
    let tilted = |deg: f32| {
        let a = deg.to_radians();
        let mut r = Rng(0x9e37_79b9);
        let img = RgbImage::from_fn(900, 600, |x, y| {
            let (cx, cy) = (x as f32 - 450.0, y as f32 - 300.0);
            let d = -cx * a.sin() + cy * a.cos();
            // Aresta com anti-aliasing (como numa fotografia real) e ruído.
            let t = (d * 0.7 + 0.5).clamp(0.0, 1.0);
            let v = (200.0 - 140.0 * t + r.gauss() * 2.0) as i32;
            Rgb([v as u8, v as u8, (v + 10).min(255) as u8])
        });
        Source::from_srgb(&img, 900, 600)
    };
    let (s, r) = run(&tilted(3.0), &AutoOptions::default());
    assert!((s.geometry.straighten + 3.0).abs() < 0.8, "{:?} {:?}", s.geometry.straighten, r.tilt);
    let (s0, _) = run(&tilted(0.0), &AutoOptions::default());
    assert_eq!(s0.geometry.straighten, 0.0);
}

#[test]
fn chromatic_aberration_is_estimated() {
    // Grelha de arestas; o vermelho é amostrado com escala radial (aberração lateral).
    let (w, h) = (1200usize, 800usize);
    let pattern = |u: f32, v: f32| -> f32 {
        let (x, y) = (u * w as f32, v * h as f32);
        if ((x / 40.0) as i32 + (y / 40.0) as i32) % 2 == 0 {
            0.8
        } else {
            0.05
        }
    };
    let lens = Lens { ca_red: 40.0, ..Default::default() };
    let geo = GeoMap::new(w, h, &Geometry::default(), &lens, false);
    let px: Vec<[f32; 3]> = (0..w * h)
        .map(|i| {
            let (u, v) = (((i % w) as f32 + 0.5) / w as f32, ((i / w) as f32 + 0.5) / h as f32);
            let m = geo.map(u, v).0;
            [pattern(m[0].0, m[0].1), pattern(u, v), pattern(u, v)]
        })
        .collect();
    let src = Source::from_plane(Plane { w, h, px }, w as u32, h as u32, 0.0);
    let (r, b) = estimate_chromatic_aberration(&src);
    // A imagem tem a aberração de "ca_red = 40" aplicada ao contrário → corrige com ≈ −40.
    assert!((r + 40.0).abs() < 10.0, "vermelho {r}");
    assert_eq!(b, 0.0, "azul sem aberração: {b}");
    // Imagem sem aberração: nada.
    let clean: Vec<[f32; 3]> = (0..w * h)
        .map(|i| {
            let (u, v) = (((i % w) as f32 + 0.5) / w as f32, ((i / w) as f32 + 0.5) / h as f32);
            [pattern(u, v); 3]
        })
        .collect();
    let c = Source::from_plane(Plane { w, h, px: clean }, w as u32, h as u32, 0.0);
    assert_eq!(estimate_chromatic_aberration(&c), (0.0, 0.0));
}

/// Rosto simples (pele) com duas pupilas de cor dada.
fn face_with_pupils(pupil: [u8; 3]) -> (Source, Vec<RedEye>) {
    let (w, h) = (800u32, 600u32);
    let eyes = [(340.0f32, 260.0f32), (460.0, 260.0)];
    let img = RgbImage::from_fn(w, h, |x, y| {
        let (xf, yf) = (x as f32, y as f32);
        for (ex, ey) in eyes {
            let d = ((xf - ex).powi(2) + (yf - ey).powi(2)).sqrt();
            if d < 9.0 {
                return Rgb(pupil);
            }
            if d < 16.0 {
                return Rgb([90, 70, 50]);
            }
            if d < 24.0 {
                return Rgb([235, 235, 230]);
            }
        }
        if ((xf - 400.0) / 160.0).powi(2) + ((yf - 300.0) / 210.0).powi(2) < 1.0 {
            Rgb([215, 165, 135])
        } else {
            Rgb([40, 40, 60])
        }
    });
    let src = Source::from_srgb(&img, w, h);
    let eyes = eyes
        .iter()
        .map(|&(x, y)| RedEye { x: x / w as f32, y: y / h as f32, r: 16.0 / w as f32 })
        .collect();
    (src, eyes)
}

#[test]
fn red_eyes_are_detected_and_corrected() {
    let (red, eyes) = face_with_pupils([200, 30, 35]);
    let found = detect_red_eyes(&red, &eyes);
    assert_eq!(found.len(), 2);
    let (dark, eyes2) = face_with_pupils([25, 20, 18]);
    assert!(detect_red_eyes(&dark, &eyes2).is_empty(), "pupilas escuras: nada");
    // Correção no render: a pupila deixa de ser vermelha; a pele à volta não muda.
    let s = Settings {
        retouch: Retouch { red_eyes: found, ..Default::default() },
        ..Default::default()
    };
    let before = render_rgb(&red, &Settings::default(), 800, Quality::Final);
    let after = render_rgb(&red, &s, 800, Quality::Final);
    let p = after.get_pixel(340, 260);
    assert!(p[0] < 90 && (p[0] as i32 - p[1] as i32) < 30, "pupila {p:?}");
    assert_eq!(before.get_pixel(400, 400), after.get_pixel(400, 400));
    // Pela edição automática (olhos da deteção de rostos no contexto).
    let ctx = AutoContext { eyes, ..Default::default() };
    let (st, rep) = auto_edit(&red, &Settings::default(), &ctx, &AutoOptions::default());
    assert_eq!((st.retouch.red_eyes.len(), rep.red_eyes), (2, 2));
}

#[test]
fn sensor_dust_is_found_on_smooth_areas_and_removed() {
    // Céu liso em degradê com 3 manchas suaves; metade de baixo com textura forte.
    let (w, h) = (1600u32, 1000u32);
    let dust = [(300.0f32, 200.0f32, 9.0f32), (900.0, 300.0, 6.0), (1300.0, 150.0, 12.0)];
    let img = RgbImage::from_fn(w, h, |x, y| {
        let (xf, yf) = (x as f32, y as f32);
        if yf > 600.0 {
            let t = (((x * 7919) ^ (y * 104_729)) % 90) as u8;
            return Rgb([60 + t, 80 + t, 50 + t]);
        }
        let mut k = 1.0;
        for (dx, dy, r) in dust {
            let d2 = (xf - dx).powi(2) + (yf - dy).powi(2);
            k *= 1.0 - 0.12 * (-d2 / (2.0 * (r * 0.6).powi(2))).exp();
        }
        let base = [130.0 + yf * 0.08, 170.0 + yf * 0.05, 230.0];
        Rgb(base.map(|c| (c * k) as u8))
    });
    let src = Source::from_srgb(&img, w, h);
    let spots = detect_dust(&src);
    for (dx, dy, _) in dust {
        assert!(
            spots
                .iter()
                .any(|s| ((s.x * w as f32 - dx).powi(2) + (s.y * h as f32 - dy).powi(2)).sqrt()
                    < 8.0),
            "mancha em ({dx}, {dy}) não encontrada: {spots:?}"
        );
    }
    assert!(spots.len() <= 4, "falsos positivos: {spots:?}");
    // Remoção: o centro da mancha fica perto do fundo.
    let st = Settings {
        retouch: Retouch { spots: spots.clone(), ..Default::default() },
        ..Default::default()
    };
    let out = render_rgb(&src, &st, w, Quality::Final);
    let (cx, cy) = (1300, 150);
    let bg = img.get_pixel(1300, 120)[0] as i32;
    assert!(
        (out.get_pixel(cx, cy)[0] as i32 - bg).abs() <= 4,
        "{} vs {bg}",
        out.get_pixel(cx, cy)[0]
    );
    assert!((img.get_pixel(cx, cy)[0] as i32 - bg).abs() > 10);
    // Imagem sem manchas: nenhuma deteção.
    assert!(detect_dust(&graded(&scene(1600, 1000), 1.0, [1.0; 3])).is_empty());
}

#[test]
fn auto_crop_keeps_people_and_improves_composition() {
    let src = graded(&scene(1200, 800), 1.0, [1.0; 3]);
    // Rosto pequeno no centro exato, um pouco baixo.
    let face = FaceBox { x: 0.47, y: 0.42, w: 0.06, h: 0.09 };
    let ctx = AutoContext { faces: vec![face], ..Default::default() };
    let o = AutoOptions { crop: true, ..Default::default() };
    let (s, rep) = auto_edit(&src, &Settings::default(), &ctx, &o);
    assert!(rep.cropped);
    let c = s.geometry.crop;
    assert!((c.w - c.h).abs() < 1e-4, "mantém a proporção");
    assert!(c.w >= 0.79, "corte limitado: {c:?}");
    assert!(
        c.x <= face.x
            && c.y <= face.y
            && c.x + c.w >= face.x + face.w
            && c.y + c.h >= face.y + face.h
    );
    // Sem pessoas: sem recorte.
    let (s2, r2) = auto_edit(&src, &Settings::default(), &AutoContext::default(), &o);
    assert!(!r2.cropped && s2.geometry.crop == Crop::default());
}

#[test]
fn auto_keeps_unrelated_user_settings() {
    let src = graded(&scene(600, 400), 0.5, [1.0; 3]);
    let mut base = Settings::default();
    base.hsl.saturation[5] = -40.0;
    base.effects.grain = 20.0;
    base.geometry.quarter_turns = 1;
    base.curve.rgb = vec![[0.0, 0.05], [1.0, 1.0]];
    let (s, _) = auto_edit(&src, &base, &AutoContext::default(), &AutoOptions::default());
    assert_eq!(s.hsl, base.hsl);
    assert_eq!(s.effects.grain, 20.0);
    assert_eq!(s.geometry.quarter_turns, 1);
    assert_eq!(s.curve.rgb, base.curve.rgb);
    // O render com a receita automática funciona (e não é igual ao original).
    let o = RenderOptions {
        max_width: 300,
        max_height: 300,
        quality: Quality::Final,
        uncropped: false,
    };
    assert_ne!(render(&src, &s, &o).rgba, render(&src, &base, &o).rgba);
}

#[test]
fn retouch_block_roundtrip() {
    let s = Settings {
        retouch: Retouch {
            red_eyes: vec![RedEye { x: 0.4, y: 0.3, r: 0.01 }],
            spots: vec![Spot { x: 0.1, y: 0.2, r: 0.005, sx: 0.12, sy: 0.2, ..Default::default() }],
            ..Default::default()
        },
        ..Default::default()
    };
    let r = s.to_recipe(&Default::default());
    assert!(r.blocks.contains_key("retouch"));
    assert_eq!(Settings::from_recipe(&r), s);
    assert!(Settings::default().to_recipe(&Default::default()).blocks.is_empty());
}
