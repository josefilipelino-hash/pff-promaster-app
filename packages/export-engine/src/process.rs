//! Dimensões, reamostragem (Lanczos 3 em luz linear), nitidez de saída e marca de água.

use crate::recipe::{
    Anchor, Recipe, SharpenAmount, SharpenTarget, Size, SizeMode, Unit, Watermark, WatermarkKind,
};
use momento_core_types::{CoreError, CoreResult};
use momento_image_processing::develop::color::{eotf, luma, mul3, oetf, SRGB_TO_REC2020};
use momento_image_processing::develop::filters::gaussian;
use rayon::prelude::*;

/// Retângulo de origem (em píxeis da imagem revelada) e tamanho final.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    pub x0: f32,
    pub y0: f32,
    pub cw: f32,
    pub ch: f32,
    pub w: u32,
    pub h: u32,
}

fn px(s: &Size, v: f32) -> f32 {
    match s.unit {
        Unit::Px => v,
        Unit::Cm => v / 2.54 * s.ppi,
        Unit::In => v * s.ppi,
    }
}

/// Calcula o recorte e as dimensões finais para uma imagem `w`×`h`.
pub fn plan(s: &Size, w: u32, h: u32) -> Plan {
    let (fw, fh) = (w as f32, h as f32);
    let (mut a, mut b) = (px(s, s.a).max(1.0), px(s, s.b).max(1.0));
    if s.match_orientation && ((a > b) != (fw > fh)) && a != b {
        std::mem::swap(&mut a, &mut b);
    }
    let full = Plan { x0: 0.0, y0: 0.0, cw: fw, ch: fh, w, h };
    let mut scale = match s.mode {
        SizeMode::Original => 1.0,
        SizeMode::LongEdge => a / fw.max(fh),
        SizeMode::ShortEdge => a / fw.min(fh),
        SizeMode::Width => a / fw,
        SizeMode::Height => a / fh,
        SizeMode::Fit => (a / fw).min(b / fh),
        SizeMode::Fill => (a / fw).max(b / fh),
        SizeMode::Megapixels => (s.a.max(0.01) * 1e6 / (fw * fh)).sqrt(),
    };
    if s.mode == SizeMode::Fill {
        // Recorte ao centro com a proporção a:b.
        let (mut cw, mut ch) = (a / scale, b / scale);
        if s.no_enlarge && scale > 1.0 {
            scale = 1.0;
            let k = (fw / a).min(fh / b);
            cw = a * k;
            ch = b * k;
        }
        let (ow, oh) = ((cw * scale).round().max(1.0), (ch * scale).round().max(1.0));
        return Plan {
            x0: (fw - cw) / 2.0,
            y0: (fh - ch) / 2.0,
            cw,
            ch,
            w: ow as u32,
            h: oh as u32,
        };
    }
    if s.no_enlarge {
        scale = scale.min(1.0);
    }
    if (scale - 1.0).abs() < 1e-6 {
        return full;
    }
    Plan {
        w: (fw * scale).round().max(1.0) as u32,
        h: (fh * scale).round().max(1.0) as u32,
        ..full
    }
}

fn lanczos3(x: f32) -> f32 {
    if x == 0.0 {
        return 1.0;
    }
    if x.abs() >= 3.0 {
        return 0.0;
    }
    let px = std::f32::consts::PI * x;
    3.0 * px.sin() * (px / 3.0).sin() / (px * px)
}

/// Pesos de uma dimensão: para cada saída, (primeiro índice, pesos normalizados).
fn weights(src_len: usize, start: f32, len: f32, out: usize) -> Vec<(usize, Vec<f32>)> {
    let step = len / out as f32;
    let scale = step.max(1.0);
    let support = 3.0 * scale;
    (0..out)
        .map(|i| {
            let c = start + (i as f32 + 0.5) * step;
            let lo = ((c - support).floor() as isize).max(0) as usize;
            let hi = ((c + support).ceil() as isize).min(src_len as isize - 1).max(0) as usize;
            let mut w: Vec<f32> =
                (lo..=hi).map(|j| lanczos3((j as f32 + 0.5 - c) / scale)).collect();
            let sum: f32 = w.iter().sum();
            if sum.abs() > 1e-8 {
                w.iter_mut().for_each(|v| *v /= sum);
            }
            (lo, w)
        })
        .collect()
}

/// Reamostra a região `p` de uma imagem linear para `p.w`×`p.h`.
pub fn resample(src: &[[f32; 3]], sw: usize, sh: usize, p: &Plan) -> Vec<[f32; 3]> {
    let (ow, oh) = (p.w as usize, p.h as usize);
    if p.x0 == 0.0 && p.y0 == 0.0 && ow == sw && oh == sh {
        return src.to_vec();
    }
    let wx = weights(sw, p.x0, p.cw, ow);
    let wy = weights(sh, p.y0, p.ch, oh);
    let (r0, r1) = (wy.first().map(|w| w.0).unwrap_or(0), {
        let l = wy.last().unwrap();
        l.0 + l.1.len()
    });
    // Passo horizontal só nas linhas necessárias.
    let mut tmp = vec![[0.0f32; 3]; ow * (r1 - r0)];
    tmp.par_chunks_mut(ow).enumerate().for_each(|(k, row)| {
        let line = &src[(r0 + k) * sw..(r0 + k + 1) * sw];
        for (x, (lo, w)) in wx.iter().enumerate() {
            let mut acc = [0.0f32; 3];
            for (j, wt) in w.iter().enumerate() {
                let q = line[lo + j];
                acc[0] += q[0] * wt;
                acc[1] += q[1] * wt;
                acc[2] += q[2] * wt;
            }
            row[x] = acc;
        }
    });
    let mut out = vec![[0.0f32; 3]; ow * oh];
    out.par_chunks_mut(ow).enumerate().for_each(|(y, row)| {
        let (lo, w) = &wy[y];
        for x in 0..ow {
            let mut acc = [0.0f32; 3];
            for (j, wt) in w.iter().enumerate() {
                let q = tmp[(lo + j - r0) * ow + x];
                acc[0] += q[0] * wt;
                acc[1] += q[1] * wt;
                acc[2] += q[2] * wt;
            }
            row[x] = acc.map(|v| v.max(0.0));
        }
    });
    out
}

/// Nitidez de saída (máscara de nitidez na luminância percetual), conforme o destino:
/// ecrã (raio fixo pequeno) ou papel mate/brilhante (raio proporcional ao PPI).
pub fn sharpen(px: &mut [[f32; 3]], w: usize, h: usize, r: &Recipe) {
    let s = r.sharpen;
    let (sigma, amounts) = match s.target {
        SharpenTarget::None => return,
        SharpenTarget::Screen => (0.6, [0.35, 0.6, 0.9]),
        SharpenTarget::Matte => ((0.9 * r.size.ppi / 300.0).max(0.6), [0.5, 0.8, 1.2]),
        SharpenTarget::Glossy => ((0.7 * r.size.ppi / 300.0).max(0.5), [0.4, 0.65, 1.0]),
    };
    let amount = amounts[match s.amount {
        SharpenAmount::Low => 0,
        SharpenAmount::Standard => 1,
        SharpenAmount::High => 2,
    }];
    let l: Vec<f32> = px.par_iter().map(|q| oetf(luma(*q).clamp(0.0, 1.0))).collect();
    let blur = gaussian(&l, w, h, sigma);
    px.par_iter_mut().enumerate().for_each(|(i, q)| {
        let d = l[i] - blur[i];
        // Limiar suave: o ruído fino não é reforçado.
        let t = ((d.abs() - 0.002) / 0.008).clamp(0.0, 1.0);
        let nl = (l[i] + amount * d * t).clamp(0.0, 1.0);
        let (a, b) = (eotf(l[i]), eotf(nl));
        let k = if a > 1e-6 { (b / a).min(4.0) } else { 1.0 };
        *q = q.map(|c| c * k);
    });
}

/// Marca de água pronta a aplicar (fonte ou imagem carregada uma vez por exportação).
pub enum Mark {
    Text { font: ab_glyph::FontVec, color: [f32; 3] },
    Image(image::RgbaImage),
}

static INTER: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.otf");

fn hex(s: &str) -> [f32; 3] {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0xffffff);
    let srgb = [(v >> 16) & 255, (v >> 8) & 255, v & 255].map(|c| eotf(c as f32 / 255.0));
    mul3(&SRGB_TO_REC2020, srgb)
}

pub fn prepare_mark(w: &Watermark) -> CoreResult<Option<Mark>> {
    if !w.enabled {
        return Ok(None);
    }
    Ok(Some(match w.kind {
        WatermarkKind::Text => {
            if w.text.trim().is_empty() {
                return Ok(None);
            }
            let data = if w.font_path.trim().is_empty() {
                INTER.to_vec()
            } else {
                std::fs::read(&w.font_path)
                    .map_err(|e| CoreError::Other(format!("fonte da marca de água: {e}")))?
            };
            let font = ab_glyph::FontVec::try_from_vec(data)
                .map_err(|_| CoreError::Corrupt("fonte da marca de água inválida".into()))?;
            Mark::Text { font, color: hex(&w.color) }
        }
        WatermarkKind::Image => {
            let img = image::open(&w.image_path)
                .map_err(|e| CoreError::Other(format!("imagem da marca de água: {e}")))?;
            Mark::Image(img.to_rgba8())
        }
    }))
}

/// Cobertura (0–1) do texto rasterizado, com a sua largura e altura.
fn text_mask(font: &ab_glyph::FontVec, text: &str, px: f32) -> (Vec<f32>, usize, usize) {
    use ab_glyph::{Font, ScaleFont};
    let sf = font.as_scaled(px);
    let lines: Vec<&str> = text.lines().collect();
    let line_h = sf.height() + sf.line_gap();
    let widths: Vec<f32> = lines
        .iter()
        .map(|l| {
            let mut x = 0.0;
            let mut prev = None;
            for ch in l.chars() {
                let g = sf.glyph_id(ch);
                if let Some(p) = prev {
                    x += sf.kern(p, g);
                }
                x += sf.h_advance(g);
                prev = Some(g);
            }
            x
        })
        .collect();
    let tw = widths.iter().cloned().fold(0.0, f32::max).ceil() as usize + 2;
    let th = (line_h * lines.len() as f32).ceil() as usize + 2;
    let mut mask = vec![0.0f32; tw * th];
    for (li, l) in lines.iter().enumerate() {
        let mut x = 1.0;
        let base = 1.0 + li as f32 * line_h + sf.ascent();
        let mut prev = None;
        for ch in l.chars() {
            let gid = sf.glyph_id(ch);
            if let Some(p) = prev {
                x += sf.kern(p, gid);
            }
            let g = gid.with_scale_and_position(px, ab_glyph::point(x, base));
            x += sf.h_advance(gid);
            prev = Some(gid);
            if let Some(o) = font.outline_glyph(g) {
                let b = o.px_bounds();
                o.draw(|gx, gy, c| {
                    let (xx, yy) = (b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32);
                    if xx >= 0 && yy >= 0 && (xx as usize) < tw && (yy as usize) < th {
                        let m = &mut mask[yy as usize * tw + xx as usize];
                        *m = (*m + c).min(1.0);
                    }
                });
            }
        }
    }
    (mask, tw, th)
}

/// Aplica a marca de água (mistura em luz linear).
pub fn watermark(px: &mut [[f32; 3]], w: usize, h: usize, wm: &Watermark, mark: &Mark) {
    let short = w.min(h) as f32;
    let size = (wm.size.clamp(0.5, 50.0) / 100.0 * short).max(4.0);
    let opacity = wm.opacity.clamp(0.0, 100.0) / 100.0;
    // (cobertura, cor) por píxel da marca.
    let build = |size: f32| -> (Vec<f32>, Option<Vec<[f32; 3]>>, usize, usize) {
        match mark {
            Mark::Text { font, .. } => {
                let (m, tw, th) = text_mask(font, &wm.text, size);
                (m, None, tw, th)
            }
            Mark::Image(img) => {
                let (iw, ih) = img.dimensions();
                let k = size / iw.max(ih) as f32;
                let (nw, nh) = (
                    ((iw as f32 * k).round() as u32).max(1),
                    ((ih as f32 * k).round() as u32).max(1),
                );
                let r = image::imageops::resize(img, nw, nh, image::imageops::FilterType::Lanczos3);
                let cov = r.pixels().map(|p| p[3] as f32 / 255.0).collect();
                let col = r
                    .pixels()
                    .map(|p| {
                        mul3(&SRGB_TO_REC2020, [p[0], p[1], p[2]].map(|c| eotf(c as f32 / 255.0)))
                    })
                    .collect();
                (cov, Some(col), nw as usize, nh as usize)
            }
        }
    };
    let (mut cov, mut colors, mut mw, mut mh) = build(size);
    // Marca maior do que a fotografia (texto longo num retrato, por exemplo): reduz-se para
    // caber — antes era omitida em silêncio e a prova saía sem marca de água (Fase 15).
    let limit = |total: usize| (total as f32 * 0.9).max(1.0);
    if mw as f32 > limit(w) || mh as f32 > limit(h) {
        let mut cur = size * (limit(w) / mw as f32).min(limit(h) / mh as f32);
        for _ in 0..8 {
            (cov, colors, mw, mh) = build(cur.max(1.0));
            if (mw as f32) <= limit(w) && (mh as f32) <= limit(h) {
                break;
            }
            cur *= 0.8; // o texto não escala exatamente em proporção: ajustar
        }
        if mw >= w || mh >= h {
            return; // imagem minúscula (alguns píxeis): não há onde pôr a marca
        }
    }
    let margin = (wm.margin.clamp(0.0, 40.0) / 100.0 * short).round() as usize;
    let (fx, fy) = match wm.anchor {
        Anchor::TopLeft => (0, 0),
        Anchor::Top => (1, 0),
        Anchor::TopRight => (2, 0),
        Anchor::Left => (0, 1),
        Anchor::Center => (1, 1),
        Anchor::Right => (2, 1),
        Anchor::BottomLeft => (0, 2),
        Anchor::Bottom => (1, 2),
        Anchor::BottomRight => (2, 2),
    };
    let pos = |f: usize, total: usize, size: usize| match f {
        0 => margin.min(total - size),
        1 => (total - size) / 2,
        _ => total.saturating_sub(size + margin),
    };
    let (ox, oy) = (pos(fx, w, mw), pos(fy, h, mh));
    let text_color = match mark {
        Mark::Text { color, .. } => *color,
        _ => [1.0; 3],
    };
    for y in 0..mh {
        for x in 0..mw {
            let a = cov[y * mw + x] * opacity;
            if a <= 0.0 {
                continue;
            }
            let c = colors.as_ref().map(|v| v[y * mw + x]).unwrap_or(text_color);
            let q = &mut px[(oy + y) * w + ox + x];
            *q = std::array::from_fn(|i| q[i] * (1.0 - a) + c[i] * a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(mode: SizeMode, a: f32, b: f32) -> Size {
        Size { mode, a, b, ..Default::default() }
    }

    /// Fase 15: marca de água maior do que a fotografia (texto longo num retrato) é
    /// reduzida e aplicada — nunca omitida em silêncio.
    #[test]
    fn oversized_watermark_is_scaled_not_dropped() {
        let (w, h) = (300usize, 450usize);
        let mut px = vec![[0.0f32; 3]; w * h];
        let wm = Watermark {
            enabled: true,
            text: "© Estúdio Fotográfico Inês & João — Casamento 2026".into(),
            size: 40.0,
            opacity: 100.0,
            ..Default::default()
        };
        let mark = Mark::Text {
            font: ab_glyph::FontVec::try_from_vec(INTER.to_vec()).unwrap(),
            color: [1.0; 3],
        };
        watermark(&mut px, w, h, &wm, &mark);
        assert!(px.iter().any(|p| p[0] > 0.1), "a marca de água desapareceu");
    }

    #[test]
    fn sizes() {
        let p = plan(&size(SizeMode::LongEdge, 2048.0, 0.0), 6000, 4000);
        assert_eq!((p.w, p.h), (2048, 1365));
        let p = plan(&size(SizeMode::ShortEdge, 1000.0, 0.0), 6000, 4000);
        assert_eq!((p.w, p.h), (1500, 1000));
        let p = plan(&size(SizeMode::Fit, 1000.0, 1000.0), 4000, 6000);
        assert_eq!((p.w, p.h), (667, 1000));
        // Instagram 4:5 numa fotografia horizontal: corta ao centro.
        let p = plan(&size(SizeMode::Fill, 1080.0, 1350.0), 6000, 4000);
        assert_eq!((p.w, p.h), (1080, 1350));
        assert!((p.cw / p.ch - 0.8).abs() < 1e-3 && (p.x0 - (6000.0 - p.cw) / 2.0).abs() < 1e-3);
        // Nunca ampliar.
        let p = plan(&size(SizeMode::LongEdge, 9000.0, 0.0), 6000, 4000);
        assert_eq!((p.w, p.h), (6000, 4000));
        let p = plan(&size(SizeMode::Fill, 3000.0, 3000.0), 2000, 1000);
        assert_eq!((p.w, p.h), (1000, 1000), "Fill sem ampliar mantém a proporção");
        // Centímetros a 300 PPI: 10 cm = 1181 px.
        let s = Size { mode: SizeMode::LongEdge, unit: Unit::Cm, a: 10.0, ..Default::default() };
        assert_eq!(plan(&s, 6000, 4000).w, 1181);
        let p = plan(&size(SizeMode::Megapixels, 6.0, 0.0), 6000, 4000);
        assert!(((p.w * p.h) as f32 / 6e6 - 1.0).abs() < 0.01);
    }

    #[test]
    fn resample_keeps_flat_color_and_mean() {
        let (w, h) = (300, 200);
        let src: Vec<[f32; 3]> = (0..w * h)
            .map(|i| if (i % w) < 150 { [0.2, 0.3, 0.4] } else { [0.6, 0.5, 0.1] })
            .collect();
        let p = plan(&size(SizeMode::LongEdge, 100.0, 0.0), w as u32, h as u32);
        let out = resample(&src, w, h, &p);
        assert_eq!(out.len(), 100 * 67);
        assert!((out[10 * 100 + 5][0] - 0.2).abs() < 1e-4);
        assert!((out[10 * 100 + 95][0] - 0.6).abs() < 1e-4);
        let mean = |v: &[[f32; 3]]| v.iter().map(|q| q[1]).sum::<f32>() / v.len() as f32;
        assert!((mean(&out) - mean(&src)).abs() < 2e-3);
    }

    #[test]
    fn sharpen_increases_edge_contrast_only_when_asked() {
        let (w, h) = (64, 64);
        let src: Vec<[f32; 3]> =
            (0..w * h).map(|i| if (i % w) < 32 { [0.1; 3] } else { [0.5; 3] }).collect();
        let mut r = Recipe::default();
        let mut a = src.clone();
        sharpen(&mut a, w, h, &r);
        assert_eq!(a, src);
        r.sharpen.target = SharpenTarget::Screen;
        sharpen(&mut a, w, h, &r);
        let row = 32 * w;
        assert!(a[row + 32][0] > 0.5 && a[row + 31][0] < 0.1, "halo de nitidez no contorno");
        assert!((a[row + 5][0] - 0.1).abs() < 1e-4, "zona lisa intacta");
    }

    #[test]
    fn text_watermark_lands_in_the_chosen_corner() {
        let (w, h) = (400, 300);
        let mut px = vec![[0.0f32; 3]; w * h];
        let wm = Watermark {
            enabled: true,
            text: "© Momento".into(),
            size: 8.0,
            opacity: 100.0,
            ..Default::default()
        };
        let mark = prepare_mark(&wm).unwrap().unwrap();
        watermark(&mut px, w, h, &wm, &mark);
        let lit = |x0: usize, x1: usize, y0: usize, y1: usize| {
            (y0..y1)
                .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .filter(|&(x, y)| px[y * w + x][0] > 0.5)
                .count()
        };
        assert!(lit(200, 400, 150, 300) > 100, "canto inferior direito");
        assert_eq!(lit(0, 200, 0, 150), 0, "resto da imagem intacto");
    }
}
