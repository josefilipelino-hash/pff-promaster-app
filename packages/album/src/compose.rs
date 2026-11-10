//! Composição de um spread em Rec. 2020 linear (mesmo espaço da revelação), pronto para a
//! conversão de cor e codificação do motor de exportação.

use crate::model::{crop_window, Spread};
use crate::spec::{AlbumSpec, ExportUnit};
use momento_export_engine::process::{resample, Plan};
use momento_image_processing::develop::color::{eotf, mul3, SRGB_TO_REC2020};
use momento_image_processing::develop::LinearImage;

/// Fornecedor das fotografias reveladas: (chave, lado maior pedido em px).
pub type Provider<'a> = dyn Fn(i64, u32) -> Option<std::sync::Arc<LinearImage>> + 'a;

fn hex(s: &str) -> [f32; 3] {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0xffffff);
    mul3(
        &SRGB_TO_REC2020,
        [(v >> 16) & 255, (v >> 8) & 255, v & 255].map(|c| eotf(c as f32 / 255.0)),
    )
}

/// Tamanho do spread com sangria, em px, para `ppm` px/mm.
pub fn canvas_size(spec: &AlbumSpec, ppm: f32) -> (u32, u32) {
    let (w, h) = spec.spread_mm();
    (
        ((w + 2.0 * spec.bleed_mm) * ppm).round() as u32,
        ((h + 2.0 * spec.bleed_mm) * ppm).round() as u32,
    )
}

/// Desenha o spread. `proof` acrescenta as linhas de corte, zona segura e dobra.
pub fn compose(
    spec: &AlbumSpec,
    spread: &Spread,
    photos: &Provider,
    ppm: f32,
    proof: bool,
) -> LinearImage {
    let (cw, ch) = canvas_size(spec, ppm);
    let (cw_, ch_) = (cw as usize, ch as usize);
    let mut px = vec![hex(&spec.background); cw_ * ch_];
    let b = spec.bleed_mm;
    for sl in &spread.slots {
        let Some(key) = sl.photo else { continue };
        // Retângulo do slot no canvas.
        let x0 = ((sl.x + b) * ppm).round() as i64;
        let y0 = ((sl.y + b) * ppm).round() as i64;
        let tw = ((sl.w * ppm).round() as i64).max(1);
        let th = ((sl.h * ppm).round() as i64).max(1);
        // Resolução necessária da fotografia para este recorte.
        let need = {
            let w0 = crop_window(3.0, 2.0, sl.w, sl.h, &sl.crop);
            let frac = (w0[2] - w0[0]).min(w0[3] - w0[1]).max(0.05);
            ((tw.max(th) as f32 / frac) * 1.05) as u32
        };
        let Some(img) = photos(key, need.clamp(64, 20_000)) else { continue };
        let (pw, ph) = (img.width as usize, img.height as usize);
        let [u0, v0, u1, v1] = crop_window(pw as f32, ph as f32, sl.w, sl.h, &sl.crop);
        let plan = Plan {
            x0: u0 * pw as f32,
            y0: v0 * ph as f32,
            cw: (u1 - u0) * pw as f32,
            ch: (v1 - v0) * ph as f32,
            w: tw as u32,
            h: th as u32,
        };
        let part = resample(&img.px, pw, ph, &plan);
        for y in 0..th {
            let cy = y0 + y;
            if cy < 0 || cy >= ch as i64 {
                continue;
            }
            for x in 0..tw {
                let cx = x0 + x;
                if cx < 0 || cx >= cw as i64 {
                    continue;
                }
                px[cy as usize * cw_ + cx as usize] = part[(y * tw + x) as usize];
            }
        }
    }
    if proof {
        overlays(spec, &mut px, cw_, ch_, ppm);
    }
    LinearImage { width: cw, height: ch, px }
}

/// Linhas da prova: corte (vermelho), zona segura (azul, tracejado), dobra (magenta).
fn overlays(spec: &AlbumSpec, px: &mut [[f32; 3]], w: usize, h: usize, ppm: f32) {
    let b = spec.bleed_mm;
    let (sw, sh) = spec.spread_mm();
    let to = |mm: f32| ((mm + b) * ppm).round() as i64;
    let line = (ppm * 0.35).max(1.0) as i64;
    let mut put = |x: i64, y: i64, c: [f32; 3], a: f32| {
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            let q = &mut px[y as usize * w + x as usize];
            *q = std::array::from_fn(|i| q[i] * (1.0 - a) + c[i] * a);
        }
    };
    let red = [0.8, 0.02, 0.02];
    let blue = [0.02, 0.15, 0.9];
    let magenta = [0.8, 0.0, 0.6];
    let rect = |put: &mut dyn FnMut(i64, i64, [f32; 3], f32),
                x0: f32,
                y0: f32,
                x1: f32,
                y1: f32,
                c: [f32; 3],
                dash: bool| {
        let (ax, ay, bx, by) = (to(x0), to(y0), to(x1), to(y1));
        let on = |t: i64| !dash || (t / (ppm as i64 * 3).max(1)) % 2 == 0;
        for x in ax..=bx {
            if on(x) {
                for k in 0..line {
                    put(x, ay + k, c, 0.9);
                    put(x, by - k, c, 0.9);
                }
            }
        }
        for y in ay..=by {
            if on(y) {
                for k in 0..line {
                    put(ax + k, y, c, 0.9);
                    put(bx - k, y, c, 0.9);
                }
            }
        }
    };
    rect(&mut put, 0.0, 0.0, sw, sh, red, false);
    let s = spec.safe_mm;
    rect(&mut put, s, s, sw - s, sh - s, blue, true);
    // Dobra: faixa translúcida e linha central.
    let (g0, g1) =
        (to(spec.width_mm - spec.gutter_mm / 2.0), to(spec.width_mm + spec.gutter_mm / 2.0));
    for y in 0..h as i64 {
        for x in g0..g1 {
            put(x, y, magenta, 0.12);
        }
        for k in 0..line {
            put(to(spec.width_mm) + k - line / 2, y, magenta, 0.8);
        }
    }
}

/// Divide um spread em páginas (cada uma com sangria nos 4 lados) se o laboratório as
/// pedir assim.
pub fn split(spec: &AlbumSpec, spread: LinearImage, ppm: f32) -> Vec<LinearImage> {
    if spec.export_unit == ExportUnit::Spreads {
        return vec![spread];
    }
    let page_w = ((spec.width_mm + 2.0 * spec.bleed_mm) * ppm).round() as usize;
    let right0 = ((spec.width_mm) * ppm).round() as usize;
    let (w, h) = (spread.width as usize, spread.height as usize);
    let cut = |x0: usize| {
        let pw = page_w.min(w - x0);
        let mut px = Vec::with_capacity(pw * h);
        for y in 0..h {
            px.extend_from_slice(&spread.px[y * w + x0..y * w + x0 + pw]);
        }
        LinearImage { width: pw as u32, height: h as u32, px }
    };
    vec![cut(0), cut(right0.min(w.saturating_sub(page_w)))]
}
