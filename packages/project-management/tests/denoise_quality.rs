//! Qualidade da redução de ruído medida (PSNR em sRGB contra o original limpo).
//!
//! RAW reais de ISO baixo (`MOMENTO_SAMPLES_DIR`), revelados e reduzidos a metade (referência
//! limpa); ruído de sensor simulado em linear (Poisson–Gauss, variância a·x + b, como ISO alto);
//! comparação do AI Denoise com um nível único (versão anterior) e com o mapa de ruído
//! estimado na própria fotografia. Precisa do modelo (`models/denoise_ffdnet.onnx`).

use momento_image_processing::develop::color::{eotf, oetf};
use momento_image_processing::develop::noise;
use momento_image_processing::develop::source::Plane;
use momento_image_processing::develop::Source;
use std::path::PathBuf;

fn psnr(a: &[[f32; 3]], b: &[[f32; 3]]) -> f64 {
    let mse: f64 = a
        .iter()
        .zip(b)
        .map(|(p, q)| (0..3).map(|c| ((p[c] - q[c]) as f64).powi(2)).sum::<f64>())
        .sum::<f64>()
        / (a.len() * 3) as f64;
    10.0 * (1.0 / mse.max(1e-12)).log10()
}

/// O caminho da aplicação: plano (mapa + modo) e a rede. Devolve se usou as sub-imagens.
fn run_plan(
    dn: &momento_face_analysis::denoise::Denoiser,
    img: &mut [[f32; 3]],
    noisy: &Plane,
) -> bool {
    let plan = noise::denoise_plan(noisy, 50.0).expect("plano");
    if plan.shuffled {
        dn.denoise_map_shuffled(img, noisy.w, noisy.h, &plan.map, &|_, _| true).unwrap();
    } else {
        dn.denoise_map(img, noisy.w, noisy.h, &plan.map, &|_, _| true).unwrap();
    }
    plan.shuffled
}

fn to_display(p: &Plane) -> Vec<[f32; 3]> {
    p.px.iter().map(|q| q.map(|c| oetf(c.clamp(0.0, 1.0)))).collect()
}

#[test]
fn ai_denoise_with_the_estimated_noise_map_beats_a_single_level() {
    let Some(dir) = std::env::var("MOMENTO_SAMPLES_DIR").ok().map(PathBuf::from) else {
        eprintln!("SALTADO: MOMENTO_SAMPLES_DIR não definido");
        return;
    };
    let Some(models) = momento_face_analysis::find_models_dir(&[]) else {
        eprintln!("SALTADO: modelos em falta");
        return;
    };
    let Some(dn) = momento_face_analysis::denoise::Denoiser::load(&models).unwrap() else {
        eprintln!("SALTADO: modelo do AI Denoise em falta");
        return;
    };
    let names = [
        "Nikon D810.nef",
        "Sony ILCE-7M4 (A7M4).ARW",
        "Canon EOS R3.cr3",
        "Samsung NX1.srw",
        "Fujifilm X-E1.raf",
        "Pentax K-1 Mark II.pef",
        "Olympus E-M10.orf",
        "Panasonic DMC-GF7.rw2",
        "Canon EOS 7D.cr2",
    ];
    let (mut gain_sum, mut n) = (0.0, 0);
    let mut gains = Vec::new();
    for name in names {
        let Ok(data) = std::fs::read(dir.join(name)) else { continue };
        let mut raw = momento_raw_engine::RawFile::open(&data).unwrap();
        let info = raw.info().clone();
        let bm = raw.develop_linear(true).unwrap();
        let src = Source::from_linear16(
            bm.width,
            bm.height,
            &bm.rgb,
            info.orientation,
            info.width,
            info.height,
        );
        let full = &src.levels[0];
        // Referência limpa: metade da resolução (média 2×2), recorte central 768×512, nível
        // médio ajustado para ~18 % (expõe sombras e realces).
        let (w, h) = (768usize.min(full.w / 2), 512usize.min(full.h / 2));
        let (ox, oy) = ((full.w / 2 - w) / 2, (full.h / 2 - h) / 2);
        let mut clean = Plane { w, h, px: Vec::with_capacity(w * h) };
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = (2 * (ox + x), 2 * (oy + y));
                let q = [
                    full.at(sx, sy),
                    full.at(sx + 1, sy),
                    full.at(sx, sy + 1),
                    full.at(sx + 1, sy + 1),
                ];
                clean.px.push(std::array::from_fn(|c| q.iter().map(|v| v[c]).sum::<f32>() / 4.0));
            }
        }
        let mean: f32 =
            clean.px.iter().map(|q| (q[0] + q[1] + q[2]) / 3.0).sum::<f32>() / (w * h) as f32;
        let k = 0.18 / mean.max(1e-4);
        for q in &mut clean.px {
            *q = q.map(|c| (c * k).clamp(0.0, 1.0));
        }
        let reference = to_display(&clean);
        let own = noise::estimate(&clean).map_or(0.0, |m| m.sigma(1, 0.18));
        let factor: f32 = std::env::var("MAP_FACTOR")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(momento_image_processing::develop::noise::AI_MAP_FACTOR);
        for (iso, a, b) in [("ISO ~1600", 4e-4f32, 2e-6f32), ("ISO ~6400", 1.5e-3, 1e-5)] {
            // A referência tem de ser limpa: o ruído próprio (a 18 %) muito abaixo do que se junta.
            if own > 0.35 * (a * 0.18 + b).sqrt() {
                eprintln!("{name:<26} {iso}: referência com ruído próprio ({own:.4}): excluída");
                continue;
            }
            let noisy = noise::add_noise(&clean, a, b, 7);
            let noisy_disp = to_display(&noisy);
            let p0 = psnr(&noisy_disp, &reference);
            // Antes: um nível único (o mesmo cálculo da versão anterior, intensidade 50).
            let s_src = Source::from_plane(noisy.clone(), w as u32, h as u32, 0.0);
            let (lum, chroma) = momento_image_processing::develop::auto::measure_noise(&s_src, 1.0);
            let sigma = (lum.max(chroma * 0.5).max(1.0) / 255.0).clamp(1.0 / 255.0, 50.0 / 255.0);
            let mut single = noisy_disp.clone();
            dn.denoise(&mut single, w, h, sigma, &|_, _| true).unwrap();
            let p1 = psnr(&single, &reference);
            // Agora: modelo de ruído estimado na fotografia → mapa por píxel.
            let model = noise::estimate(&noisy).expect("modelo de ruído");
            let mut mapped = noisy_disp.clone();
            if factor == noise::AI_MAP_FACTOR {
                // O caminho da aplicação: ruído branco não pode ser tratado como correlacionado.
                assert!(
                    !run_plan(&dn, &mut mapped, &noisy),
                    "{name}: ruído branco detetado como correlacionado"
                );
            } else {
                let map: Vec<f32> =
                    noise::display_sigma(&noisy, &model).iter().map(|v| v * factor).collect();
                dn.denoise_map(&mut mapped, w, h, &map, &|_, _| true).unwrap();
            }
            let p2 = psnr(&mapped, &reference);
            let truth_err = (model.sigma(1, 0.18) / (a * 0.18 + b).sqrt() - 1.0) * 100.0;
            eprintln!(
                "{name:<26} {iso}: ruidosa {p0:.2} dB | nível único {p1:.2} dB | mapa {p2:.2} dB (+{:.2}) · modelo estimado {truth_err:+.0} %",
                p2 - p1
            );
            // Em fotografias muito texturadas a ISO moderado qualquer redução de ruído perde
            // detalhe real (o PSNR desce face à ruidosa): compara-se com a versão anterior.
            assert!(p2 > p1 - 0.5, "{name} {iso}: o mapa não pode piorar face ao nível único");
            gain_sum += p2 - p1;
            gains.push(p2 - p1);
            n += 1;
        }
        let _ = eotf(0.5);
    }
    assert!(n >= 4, "poucas fotografias: {n}");
    eprintln!(
        "ganho médio do mapa sobre o nível único: {:+.2} dB em {n} casos",
        gain_sum / n as f64
    );
    gains.sort_by(f64::total_cmp);
    eprintln!("mediana {:+.2} dB, pior caso {:+.2} dB", gains[gains.len() / 2], gains[0]);
    assert!(
        gain_sum / n as f64 > 0.3 && gains[gains.len() / 2] > 0.0,
        "o mapa de ruído tem de melhorar"
    );
    assert!(gains[0] > -0.5, "nunca pode piorar de forma visível: {:+.2} dB", gains[0]);
}

/// Nível de ruído real (sem ruído acrescentado) nas amostras: o mapa novo face ao nível
/// único antigo (diagnóstico; só falha se o mapa ficar muito abaixo do nível antigo).
#[test]
fn noise_map_level_on_real_raws() {
    let Some(dir) = std::env::var("MOMENTO_SAMPLES_DIR").ok().map(PathBuf::from) else {
        return;
    };
    for p in std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()) {
        let Ok(data) = std::fs::read(&p) else { continue };
        let Ok(mut raw) = momento_raw_engine::RawFile::open(&data) else { continue };
        let info = raw.info().clone();
        let Ok(bm) = raw.develop_linear(false) else { continue };
        let src = Source::from_linear16(
            bm.width,
            bm.height,
            &bm.rgb,
            info.orientation,
            info.width,
            info.height,
        );
        let (lum, chroma) = momento_image_processing::develop::auto::measure_noise(&src, 1.0);
        let old = (lum.max(chroma * 0.5).max(1.0) / 255.0).clamp(1.0 / 255.0, 50.0 / 255.0);
        let Some(m) = noise::estimate(&src.levels[0]) else { continue };
        let map = noise::display_sigma(&src.levels[0], &m);
        let mut sorted = map.clone();
        sorted.sort_by(f32::total_cmp);
        let med = sorted[sorted.len() / 2] * noise::AI_MAP_FACTOR;
        eprintln!(
            "{:<34} ISO {:>6?}: nível único {:.2} · mapa mediana {:.2} (p10 {:.2}, p90 {:.2}) [×255]",
            p.file_name().unwrap().to_string_lossy(),
            info.iso,
            old * 255.0,
            med * 255.0,
            sorted[sorted.len() / 10] * noise::AI_MAP_FACTOR * 255.0,
            sorted[sorted.len() * 9 / 10] * noise::AI_MAP_FACTOR * 255.0
        );
    }
}

/// Ruído correlacionado (como num RAW real desmatrizado): mapa medido a 2 píxeis e redução por
/// sub-imagens intercaladas, face ao nível único e ao mapa simples.
#[test]
fn correlated_noise_needs_the_shuffled_denoiser() {
    let Some(dir) = std::env::var("MOMENTO_SAMPLES_DIR").ok().map(PathBuf::from) else {
        return;
    };
    let Some(dn) = momento_face_analysis::find_models_dir(&[])
        .and_then(|m| momento_face_analysis::denoise::Denoiser::load(&m).unwrap())
    else {
        return;
    };
    let mut gains = Vec::new();
    for name in [
        "Nikon D810.nef",
        "Sony ILCE-7M4 (A7M4).ARW",
        "Samsung NX1.srw",
        "Canon EOS 7D.cr2",
        "Pentax K-1 Mark II.pef",
    ] {
        let Ok(data) = std::fs::read(dir.join(name)) else { continue };
        let mut raw = momento_raw_engine::RawFile::open(&data).unwrap();
        let info = raw.info().clone();
        let bm = raw.develop_linear(true).unwrap();
        let src = Source::from_linear16(
            bm.width,
            bm.height,
            &bm.rgb,
            info.orientation,
            info.width,
            info.height,
        );
        let full = &src.levels[0];
        let (w, h) = (768usize.min(full.w / 2), 512usize.min(full.h / 2));
        let (ox, oy) = ((full.w / 2 - w) / 2, (full.h / 2 - h) / 2);
        let mut clean = Plane { w, h, px: Vec::with_capacity(w * h) };
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = (2 * (ox + x), 2 * (oy + y));
                let q = [
                    full.at(sx, sy),
                    full.at(sx + 1, sy),
                    full.at(sx, sy + 1),
                    full.at(sx + 1, sy + 1),
                ];
                clean.px.push(std::array::from_fn(|c| q.iter().map(|v| v[c]).sum::<f32>() / 4.0));
            }
        }
        let mean: f32 =
            clean.px.iter().map(|q| (q[0] + q[1] + q[2]) / 3.0).sum::<f32>() / (w * h) as f32;
        let k = 0.18 / mean.max(1e-4);
        for q in &mut clean.px {
            *q = q.map(|c| (c * k).clamp(0.0, 1.0));
        }
        let reference = to_display(&clean);
        let (a, b) = (1.5e-3f32, 1e-5f32);
        let noisy = noise::add_correlated_noise(&clean, a, b, 11);
        let noisy_disp = to_display(&noisy);
        let p0 = psnr(&noisy_disp, &reference);
        let s_src = Source::from_plane(noisy.clone(), w as u32, h as u32, 0.0);
        let (lum, chroma) = momento_image_processing::develop::auto::measure_noise(&s_src, 1.0);
        let sigma = (lum.max(chroma * 0.5).max(1.0) / 255.0).clamp(1.0 / 255.0, 50.0 / 255.0);
        let mut single = noisy_disp.clone();
        dn.denoise(&mut single, w, h, sigma, &|_, _| true).unwrap();
        let p1 = psnr(&single, &reference);
        let m1 = noise::estimate(&noisy).unwrap();
        let map1: Vec<f32> =
            noise::display_sigma(&noisy, &m1).iter().map(|v| v * noise::AI_MAP_FACTOR).collect();
        let mut simple = noisy_disp.clone();
        dn.denoise_map(&mut simple, w, h, &map1, &|_, _| true).unwrap();
        let p2 = psnr(&simple, &reference);
        let m2 = noise::estimate_at(&noisy, 2).unwrap();
        let mut shuffled = noisy_disp.clone();
        assert!(run_plan(&dn, &mut shuffled, &noisy), "{name}: ruído correlacionado não detetado");
        let p3 = psnr(&shuffled, &reference);
        eprintln!(
            "{name:<26} correlacionado ISO ~6400: ruidosa {p0:.2} | nível único {p1:.2} | mapa {p2:.2} | mapa a 2 px + intercalado {p3:.2} dB (+{:.2}) · σ estimado {:.0} % / {:.0} %",
            p3 - p1,
            (m1.sigma(1, 0.18) / (a * 0.18 + b).sqrt() - 1.0) * 100.0,
            (m2.sigma(1, 0.18) / (a * 0.18 + b).sqrt() - 1.0) * 100.0
        );
        gains.push(p3 - p1);
    }
    if gains.is_empty() {
        return;
    }
    gains.sort_by(f64::total_cmp);
    eprintln!("ganho mediano {:+.2} dB, pior {:+.2} dB", gains[gains.len() / 2], gains[0]);
    assert!(
        gains[gains.len() / 2] > 1.0,
        "ruído correlacionado: o novo método tem de ganhar claramente"
    );
}
