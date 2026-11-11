//! **Consistência visual**: sincronizar fotografias com uma fotografia de referência
//! **sem copiar os mesmos valores**.
//!
//! 1. O *aspeto* da referência (tons, cor, HSL, curva, tonalização, P&B, efeitos, LUT,
//!    nitidez) é copiado — é o estilo escolhido.
//! 2. O que depende de cada fotografia é **medido no resultado** e ajustado até coincidir
//!    com a referência:
//!    - balanço de brancos: a cor do iluminante de cada fotografia é estimada e o
//!      balanço é escolhido para que o branco fique da mesma cor que na referência;
//!    - exposição: luminância dos rostos (se as duas fotografias os tiverem) ou chave da
//!      cena, depois de editada;
//!    - contraste e cor: desvio-padrão da luminância e saturação média do resultado;
//!    - tons de pele: matiz e saturação médios dos rostos → HSL laranja/vermelho.
//!
//! As medições fazem-se em renders pequenos (lado maior [`MEASURE_EDGE`]).

use super::auto::{estimate_illuminant, FaceBox};
use super::color::{eotf, luma, match_white_at, mul3, wb_matrix_at};
use super::render::{render, Quality, RenderOptions};
use super::settings::Settings;
use super::source::Source;
use serde::{Deserialize, Serialize};

pub const MEASURE_EDGE: u32 = 384;

/// O que ajustar em cada fotografia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchOptions {
    pub exposure: bool,
    pub white_balance: bool,
    pub skin: bool,
    pub contrast: bool,
    pub color: bool,
    /// Copiar o aspeto da referência (tons, cor, curva, tonalização, P&B, efeitos, LUT).
    pub look: bool,
}

impl Default for MatchOptions {
    fn default() -> Self {
        MatchOptions {
            exposure: true,
            white_balance: true,
            skin: true,
            contrast: true,
            color: true,
            look: true,
        }
    }
}

/// Medições do resultado de uma fotografia.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookMeasure {
    /// Mediana da luminância de visualização (0–1).
    pub key: f32,
    /// Luminância média dos rostos (visualização), se houver.
    pub face: Option<f32>,
    /// Desvio-padrão da luminância.
    pub contrast: f32,
    /// Saturação média (HSV) das zonas não escuras.
    pub saturation: f32,
    /// Matiz (graus) e saturação médios da pele, se houver rostos.
    pub skin: Option<(f32, f32)>,
}

/// Resultado por fotografia (para a interface e os testes).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchReport {
    pub exposure: f32,
    pub temperature: f32,
    pub tint: f32,
    pub contrast: f32,
    pub vibrance: f32,
    pub skin_hue: f32,
    pub skin_saturation: f32,
    pub used_faces: bool,
}

fn hsv(c: [f32; 3]) -> (f32, f32, f32) {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let d = mx - mn;
    let h = if d <= 1e-6 {
        0.0
    } else if mx == c[0] {
        60.0 * ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if mx == c[1] {
        60.0 * ((c[2] - c[0]) / d + 2.0)
    } else {
        60.0 * ((c[0] - c[1]) / d + 4.0)
    };
    (h, if mx <= 1e-6 { 0.0 } else { d / mx }, mx)
}

/// Mede o resultado de `s` (render pequeno, sem recorte, para os rostos coincidirem).
pub fn measure(src: &Source, s: &Settings, faces: &[FaceBox]) -> LookMeasure {
    let r = render(
        src,
        s,
        &RenderOptions {
            max_width: MEASURE_EDGE,
            max_height: MEASURE_EDGE,
            quality: Quality::Fast,
            uncropped: true,
        },
    );
    let (w, h) = (r.width as usize, r.height as usize);
    let px: Vec<[f32; 3]> = r
        .rgba
        .chunks_exact(4)
        .map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0])
        .collect();
    let mut lum: Vec<f32> = px.iter().map(|p| luma(*p)).collect();
    let n = lum.len().max(1) as f32;
    let mean = lum.iter().sum::<f32>() / n;
    let contrast = (lum.iter().map(|l| (l - mean).powi(2)).sum::<f32>() / n).sqrt();
    let (mut sat, mut ns) = (0.0, 0.0);
    for p in &px {
        let (_, sv, v) = hsv(*p);
        if v > 0.15 {
            sat += sv;
            ns += 1.0;
        }
    }
    lum.sort_by(f32::total_cmp);
    let key = lum[lum.len() / 2];
    // Rostos: interior da caixa (sem cabelo nem fundo), só píxeis com cor de pele plausível.
    let geometric = s.geometry.quarter_turns == 0 && !s.geometry.flip_horizontal;
    let (mut fl, mut fh, mut fs, mut nf) = (0.0, (0.0f32, 0.0f32), 0.0, 0.0);
    if geometric {
        for f in faces {
            let (x0, x1) = (
                ((f.x + f.w * 0.25) * w as f32) as usize,
                ((f.x + f.w * 0.75) * w as f32) as usize,
            );
            let (y0, y1) =
                (((f.y + f.h * 0.3) * h as f32) as usize, ((f.y + f.h * 0.8) * h as f32) as usize);
            for y in y0.min(h)..y1.min(h) {
                for x in x0.min(w)..x1.min(w) {
                    let p = px[y * w + x];
                    let (hh, ss, v) = hsv(p);
                    fl += luma(p);
                    // Matiz como vetor (média circular).
                    if v > 0.1 && ss > 0.05 && !(60.0..=330.0).contains(&hh) {
                        let a = hh.to_radians();
                        fh.0 += a.cos();
                        fh.1 += a.sin();
                        fs += ss;
                    }
                    nf += 1.0;
                }
            }
        }
    }
    let skin_n = (fh.0 * fh.0 + fh.1 * fh.1).sqrt();
    LookMeasure {
        key,
        face: (nf >= 9.0).then(|| fl / nf),
        contrast,
        saturation: if ns > 0.0 { sat / ns } else { 0.0 },
        skin: (nf >= 9.0 && skin_n > 1.0).then(|| {
            let hue = fh.1.atan2(fh.0).to_degrees().rem_euclid(360.0);
            (hue, fs / nf)
        }),
    }
}

/// Branco da referência depois do seu balanço (para o reproduzir noutras fotografias).
fn rendered_white(src: &Source, s: &Settings) -> Option<([f32; 3], [f32; 3])> {
    let ill = estimate_illuminant(src.level_for(640.0))?;
    let w = mul3(&wb_matrix_at(s.wb.temperature, s.wb.tint, src.wb_reference_k()), ill);
    let g = w[1].max(1e-6);
    Some((ill, w.map(|v| v / g)))
}

/// Ajusta `target` (ajustes atuais da fotografia) para coincidir com a referência.
pub fn match_reference(
    ref_src: &Source,
    ref_settings: &Settings,
    ref_faces: &[FaceBox],
    src: &Source,
    target: &Settings,
    faces: &[FaceBox],
    o: &MatchOptions,
) -> (Settings, MatchReport) {
    let mut t = target.clone();
    if o.look {
        // Aspeto da referência; o que é de cada fotografia (geometria, retoque, objetiva,
        // ruído) fica.
        let keep = t.clone();
        t = ref_settings.clone();
        t.geometry = keep.geometry;
        t.retouch = keep.retouch;
        // Máscaras (pinceladas, IA de sujeito/céu) e regiões de retrato são da fotografia:
        // copiá-las punha a máscara e o alisar de pele da referência no sítio errado das
        // outras, e apagava as suas (Fase 15).
        t.masks = keep.masks;
        t.portrait = keep.portrait;
        t.lens = keep.lens;
        t.detail.noise_luminance = keep.detail.noise_luminance;
        t.detail.noise_color = keep.detail.noise_color;
        t.detail.noise_detail = keep.detail.noise_detail;
        t.detail.ai_denoise = keep.detail.ai_denoise;
        // Exposição e balanço são recalculados abaixo (ou mantidos, se desligados).
        t.light.exposure = keep.light.exposure;
        t.wb = keep.wb;
    }
    let start = t.clone();
    if o.white_balance {
        if let (Some((_, w_ref)), Some(ill)) =
            (rendered_white(ref_src, ref_settings), estimate_illuminant(src.level_for(640.0)))
        {
            let (temp, tint) = match_white_at(ill, w_ref, src.wb_reference_k());
            t.wb.temperature = temp.clamp(-100.0, 100.0);
            t.wb.tint = tint.clamp(-100.0, 100.0);
        }
    }
    let r = measure(ref_src, ref_settings, ref_faces);
    let use_faces = r.face.is_some() && !faces.is_empty() && !ref_faces.is_empty();
    let fit_exposure = |t: &mut Settings| {
        for _ in 0..4 {
            let m = measure(src, t, faces);
            let (want, have) = match (use_faces, r.face, m.face) {
                (true, Some(a), Some(b)) => (a, b),
                _ => (r.key, m.key),
            };
            let (lw, lh) = (eotf(want.clamp(0.01, 0.99)), eotf(have.clamp(0.01, 0.99)));
            let step = (lw / lh).log2().clamp(-2.0, 2.0);
            if step.abs() < 0.02 {
                break;
            }
            t.light.exposure = (t.light.exposure + step * 0.9).clamp(-5.0, 5.0);
        }
    };
    if o.exposure {
        fit_exposure(&mut t);
    }
    if o.contrast {
        for _ in 0..3 {
            let m = measure(src, &t, faces);
            let d = r.contrast - m.contrast;
            if d.abs() < 0.004 {
                break;
            }
            t.light.contrast = (t.light.contrast + d * 300.0)
                .clamp(start.light.contrast - 40.0, start.light.contrast + 40.0)
                .clamp(-100.0, 100.0);
        }
    }
    if o.color {
        for _ in 0..3 {
            let m = measure(src, &t, faces);
            let d = r.saturation - m.saturation;
            if d.abs() < 0.005 {
                break;
            }
            t.color.vibrance = (t.color.vibrance + d * 180.0)
                .clamp(start.color.vibrance - 30.0, start.color.vibrance + 30.0)
                .clamp(-100.0, 100.0);
        }
    }
    let mut skin = (0.0, 0.0);
    if o.skin && use_faces {
        if let (Some((rh, rs)), Some((th, ts))) = (r.skin, measure(src, &t, faces).skin) {
            // Diferença de matiz (graus) → HSL laranja e vermelho (100 = 30°).
            let mut dh = rh - th;
            if dh > 180.0 {
                dh -= 360.0;
            } else if dh < -180.0 {
                dh += 360.0;
            }
            let hue = (dh / 30.0 * 100.0 * 0.8).clamp(-20.0, 20.0);
            let sat = ((rs / ts.max(1e-3) - 1.0) * 100.0 * 0.8).clamp(-25.0, 25.0);
            if dh.abs() > 1.0 {
                t.hsl.hue[1] = (t.hsl.hue[1] + hue).clamp(-100.0, 100.0);
                t.hsl.hue[0] = (t.hsl.hue[0] + hue * 0.5).clamp(-100.0, 100.0);
                skin.0 = hue;
            }
            if (rs - ts).abs() > 0.01 {
                t.hsl.saturation[1] = (t.hsl.saturation[1] + sat).clamp(-100.0, 100.0);
                skin.1 = sat;
            }
        }
        // A pele mexe ligeiramente na luminância dos rostos: confirmar a exposição.
        if o.exposure {
            fit_exposure(&mut t);
        }
    }
    let round = |v: f32, k: f32| (v * k).round() / k;
    t.light.exposure = round(t.light.exposure, 100.0);
    t.light.contrast = round(t.light.contrast, 1.0);
    t.color.vibrance = round(t.color.vibrance, 1.0);
    t.wb.temperature = round(t.wb.temperature, 10.0);
    t.wb.tint = round(t.wb.tint, 10.0);
    let rep = MatchReport {
        exposure: t.light.exposure - target.light.exposure,
        temperature: t.wb.temperature - target.wb.temperature,
        tint: t.wb.tint - target.wb.tint,
        contrast: t.light.contrast - start.light.contrast,
        vibrance: t.color.vibrance - start.color.vibrance,
        skin_hue: skin.0,
        skin_saturation: skin.1,
        used_faces: use_faces,
    };
    (t, rep)
}

#[cfg(test)]
mod tests {
    use super::super::color::SRGB_TO_REC2020;
    use super::super::source::Plane;
    use super::*;
    use image::{Rgb, RgbImage};

    /// Cena com um rosto, um vestido branco e fundo; `gain` e `cast` simulam outra luz.
    fn shot(gain: f32, cast: [f32; 3], seed: u32) -> (Source, Vec<FaceBox>) {
        let img = RgbImage::from_fn(480, 320, |x, y| {
            let (u, v) = (x as f32 / 480.0, y as f32 / 320.0);
            let off = (seed % 3) as f32 * 0.03;
            if (u - 0.5 - off).powi(2) / 0.01 + (v - 0.35).powi(2) / 0.02 < 1.0 {
                Rgb([215, 160, 130])
            } else if (0.35 + off..0.65 + off).contains(&u) && v > 0.5 {
                Rgb([235, 233, 228])
            } else if v < 0.3 {
                Rgb([120, 150, 190])
            } else {
                let t = ((x / 20 + y / 20 + seed) % 2) as u8;
                Rgb([90 + t * 40, 80 + t * 30, 60 + t * 20])
            }
        });
        let px: Vec<[f32; 3]> = img
            .pixels()
            .map(|p| {
                let c: [f32; 3] =
                    std::array::from_fn(|i| (eotf(p[i] as f32 / 255.0) * cast[i] * gain).min(1.0));
                mul3(&SRGB_TO_REC2020, c)
            })
            .collect();
        let off = (seed % 3) as f32 * 0.03;
        (
            Source::from_plane(Plane { w: 480, h: 320, px }, 4800, 3200, 0.0),
            vec![FaceBox { x: 0.4 + off, y: 0.21, w: 0.2, h: 0.28 }],
        )
    }

    #[test]
    fn syncing_matches_the_result_not_the_values() {
        let (rs, rf) = shot(1.0, [1.0, 1.0, 1.0], 0);
        let mut reference = Settings::default();
        reference.light.exposure = 0.3;
        reference.light.contrast = 15.0;
        reference.wb.temperature = 10.0;
        reference.color.vibrance = 10.0;
        reference.hsl.saturation[3] = -30.0;
        reference.grading.highlights_sat = 15.0;
        // Outra fotografia: 1,3 EV mais escura e com luz de tungsténio (quente).
        let (ts, tf) = shot(0.4, [1.3, 1.0, 0.65], 1);
        let target = Settings::default();
        let (out, rep) =
            match_reference(&rs, &reference, &rf, &ts, &target, &tf, &MatchOptions::default());
        // Não é uma cópia: exposição e balanço diferentes dos da referência…
        assert!(out.light.exposure > reference.light.exposure + 0.8, "{rep:?}");
        assert!(
            out.wb.temperature < reference.wb.temperature - 15.0,
            "luz quente → arrefecer: {rep:?}"
        );
        assert!(rep.used_faces);
        // …o aspeto é copiado…
        assert_eq!(out.hsl.saturation[3], -30.0);
        assert_eq!(out.grading, reference.grading);
        // …e o resultado coincide: rostos, branco do vestido e pele.
        let mr = measure(&rs, &reference, &rf);
        let mt = measure(&ts, &out, &tf);
        assert!((mr.face.unwrap() - mt.face.unwrap()).abs() < 0.03, "{mr:?} {mt:?}");
        let mean = |s: &Source, st: &Settings| {
            let img = super::super::render_rgb(s, st, 240, Quality::Final);
            let mut acc = [0.0f32; 3];
            let mut n = 0.0;
            for y in 105..135 {
                for x in 100..130 {
                    let p = img.get_pixel(x, y);
                    for i in 0..3 {
                        acc[i] += p[i] as f32;
                    }
                    n += 1.0;
                }
            }
            acc.map(|v| v / n)
        };
        let (wr, wt) = (mean(&rs, &reference), mean(&ts, &out));
        let chroma = |c: [f32; 3]| (c[0] - c[2]) / (c[0] + c[1] + c[2]);
        assert!((chroma(wr) - chroma(wt)).abs() < 0.03, "vestido: {wr:?} vs {wt:?}");
        let naive = mean(&ts, &Settings { geometry: target.geometry.clone(), ..reference.clone() });
        assert!(
            (chroma(wr) - chroma(naive)).abs() > 0.06,
            "copiar os valores não chegava: {naive:?}"
        );
        let (sr, st) = (mr.skin.unwrap(), mt.skin.unwrap());
        assert!((sr.0 - st.0).abs() < 4.0 && (sr.1 - st.1).abs() < 0.05, "pele {sr:?} vs {st:?}");
        // Contraste e saturação próximos.
        assert!(
            (mr.contrast - mt.contrast).abs() < 0.02
                && (mr.saturation - mt.saturation).abs() < 0.03,
            "{mr:?} {mt:?}"
        );
    }

    #[test]
    fn options_limit_what_changes() {
        let (rs, rf) = shot(1.0, [1.0; 3], 0);
        let (ts, tf) = shot(0.5, [1.2, 1.0, 0.8], 2);
        let mut reference = Settings::default();
        reference.light.contrast = 20.0;
        let target = Settings {
            light: super::super::Light { exposure: 0.2, ..Default::default() },
            ..Default::default()
        };
        let only_exp = MatchOptions {
            white_balance: false,
            skin: false,
            contrast: false,
            color: false,
            look: false,
            exposure: true,
        };
        let (out, _) = match_reference(&rs, &reference, &rf, &ts, &target, &tf, &only_exp);
        assert!(out.light.exposure > 0.6);
        assert_eq!((out.wb, out.light.contrast), (target.wb, 0.0));
        // Fase 15: o aspeto da referência não traz as suas máscaras nem regiões de retrato.
        let mut reference2 = reference.clone();
        reference2.masks.push(super::super::LocalMask {
            id: "ref".into(),
            name: "Sujeito".into(),
            enabled: true,
            density: 100.0,
            ..Default::default()
        });
        reference2.portrait.skin = 40.0;
        let mut target2 = target.clone();
        target2.masks.push(super::super::LocalMask {
            id: "alvo".into(),
            name: "Céu".into(),
            enabled: true,
            density: 100.0,
            ..Default::default()
        });
        let (out3, _) =
            match_reference(&rs, &reference2, &rf, &ts, &target2, &tf, &MatchOptions::default());
        assert_eq!(out3.masks, target2.masks, "máscaras da própria fotografia");
        assert_eq!(out3.portrait, target2.portrait, "retrato da própria fotografia");
        // Sem rostos: usa a chave da cena.
        let (out2, rep2) =
            match_reference(&rs, &reference, &[], &ts, &target, &[], &MatchOptions::default());
        assert!(!rep2.used_faces && out2.light.exposure > 0.5);
    }
}
