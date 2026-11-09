//! Geometria e correções de objetiva numa única transformação inversa: para cada píxel do
//! resultado calcula-se de onde vem no original (recorte → endireitar → perspetiva →
//! rotações de 90°/espelho → distorção → aberração cromática). Uma só reamostragem.

use super::settings::{Geometry, Lens};

pub struct GeoMap {
    /// Proporção da imagem orientada (largura/altura) e depois das rotações de 90°.
    src_aspect: f32,
    canvas_w: f32,
    canvas_h: f32,
    crop: (f32, f32, f32, f32),
    cos: f32,
    sin: f32,
    zoom: f32,
    kv: f32,
    kh: f32,
    turns: u8,
    flip: bool,
    k_dist: f32,
    ca_r: f32,
    ca_b: f32,
    /// Ignora o recorte (modo de recorte na interface mostra a imagem toda).
    pub uncropped: bool,
}

/// Dimensões do resultado (proporção do recorte) dentro de uma caixa `max_w`×`max_h`.
pub fn output_size(
    src_w: usize,
    src_h: usize,
    g: &Geometry,
    uncropped: bool,
    max_w: u32,
    max_h: u32,
) -> (u32, u32) {
    let (cw, ch) = if g.quarter_turns % 2 == 1 {
        (src_h as f32, src_w as f32)
    } else {
        (src_w as f32, src_h as f32)
    };
    let (w, h) = if uncropped {
        (cw, ch)
    } else {
        (cw * g.crop.w.clamp(0.01, 1.0), ch * g.crop.h.clamp(0.01, 1.0))
    };
    let s = (max_w as f32 / w).min(max_h as f32 / h);
    (((w * s).round() as u32).max(1), ((h * s).round() as u32).max(1))
}

impl GeoMap {
    pub fn new(src_w: usize, src_h: usize, g: &Geometry, lens: &Lens, uncropped: bool) -> Self {
        let src_aspect = src_w as f32 / src_h as f32;
        let (canvas_w, canvas_h) =
            if g.quarter_turns % 2 == 1 { (1.0, src_aspect) } else { (src_aspect, 1.0) };
        let a = g.straighten.clamp(-45.0, 45.0).to_radians();
        let (sin, cos) = a.sin_cos();
        // Zoom mínimo para a imagem rodada cobrir o enquadramento (sem cantos pretos).
        let zoom = ((canvas_w * cos.abs() + canvas_h * sin.abs()) / canvas_w)
            .max((canvas_w * sin.abs() + canvas_h * cos.abs()) / canvas_h);
        let c = &g.crop;
        GeoMap {
            src_aspect,
            canvas_w,
            canvas_h,
            crop: (
                c.x.clamp(0.0, 1.0),
                c.y.clamp(0.0, 1.0),
                c.w.clamp(0.01, 1.0),
                c.h.clamp(0.01, 1.0),
            ),
            cos,
            sin,
            zoom,
            kv: g.vertical / 100.0 * 0.6,
            kh: g.horizontal / 100.0 * 0.6,
            turns: g.quarter_turns % 4,
            flip: g.flip_horizontal,
            k_dist: -lens.distortion / 100.0 * 0.12,
            ca_r: lens.ca_red / 100.0 * 0.004,
            ca_b: lens.ca_blue / 100.0 * 0.004,
            uncropped,
        }
    }

    /// (u, v) normalizados no resultado → posição no original orientado, por canal (R, G, B),
    /// em coordenadas normalizadas (0–1). Também devolve o raio normalizado (vinheta da lente).
    #[inline]
    pub fn map(&self, u: f32, v: f32) -> ([(f32, f32); 3], f32) {
        let (cx, cy) = if self.uncropped {
            (u, v)
        } else {
            (self.crop.0 + u * self.crop.2, self.crop.1 + v * self.crop.3)
        };
        // Enquadramento centrado (unidades: altura do original = 1 antes das rotações).
        let (mut x, mut y) = ((cx - 0.5) * self.canvas_w, (cy - 0.5) * self.canvas_h);
        // Endireitar: rotação inversa + zoom para não haver cantos vazios.
        let (rx, ry) = (self.cos * x + self.sin * y, -self.sin * x + self.cos * y);
        x = rx / self.zoom;
        y = ry / self.zoom;
        // Perspetiva (trapézio): vertical comprime/estica as linhas conforme a altura.
        let (yn, xn) = (y / self.canvas_h, x / self.canvas_w);
        x *= 1.0 + self.kv * yn;
        y *= 1.0 + self.kh * xn;
        // Rotações de 90° e espelho: do enquadramento para o original orientado.
        let (mut ox, mut oy) = match self.turns {
            1 => (y, -x),
            2 => (-x, -y),
            3 => (-y, x),
            _ => (x, y),
        };
        if self.flip {
            ox = -ox;
        }
        // Distorção da objetiva (raio normalizado pela meia-diagonal).
        let hd = (self.src_aspect * self.src_aspect + 1.0).sqrt() / 2.0;
        let r2 = (ox * ox + oy * oy) / (hd * hd);
        let k = 1.0 + self.k_dist * r2;
        ox *= k;
        oy *= k;
        let to_uv = |s: f32| (ox * s / self.src_aspect + 0.5, oy * s + 0.5);
        let g = to_uv(1.0);
        ([to_uv(1.0 + self.ca_r * r2), g, to_uv(1.0 + self.ca_b * r2)], r2.sqrt())
    }
}

/// Ponto da vista (0–1, depois da geometria) → ponto da fotografia orientada.
pub fn view_to_source(
    src_w: usize,
    src_h: usize,
    g: &Geometry,
    lens: &Lens,
    uncropped: bool,
    u: f32,
    v: f32,
) -> (f32, f32) {
    GeoMap::new(src_w, src_h, g, lens, uncropped).map(u, v).0[1]
}

/// Ponto da fotografia orientada → ponto da vista (inversão numérica, Newton).
pub fn source_to_view(
    src_w: usize,
    src_h: usize,
    g: &Geometry,
    lens: &Lens,
    uncropped: bool,
    x: f32,
    y: f32,
) -> Option<(f32, f32)> {
    let geo = GeoMap::new(src_w, src_h, g, lens, uncropped);
    let (mut u, mut v) = (x, y);
    for _ in 0..20 {
        let p = geo.map(u, v).0[1];
        let (ex, ey) = (p.0 - x, p.1 - y);
        if ex.abs() < 1e-6 && ey.abs() < 1e-6 {
            return Some((u, v));
        }
        let e = 1e-3;
        let pu = geo.map(u + e, v).0[1];
        let pv = geo.map(u, v + e).0[1];
        let (a, b, c, d) = ((pu.0 - p.0) / e, (pv.0 - p.0) / e, (pu.1 - p.1) / e, (pv.1 - p.1) / e);
        let det = a * d - b * c;
        if det.abs() < 1e-9 {
            return None;
        }
        u -= (d * ex - b * ey) / det;
        v -= (-c * ex + a * ey) / det;
    }
    let p = geo.map(u, v).0[1];
    ((p.0 - x).abs() < 1e-3 && (p.1 - y).abs() < 1e-3).then_some((u, v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_and_source_are_inverse() {
        let g = Geometry {
            crop: super::super::settings::Crop { x: 0.1, y: 0.2, w: 0.6, h: 0.5 },
            straighten: 7.0,
            quarter_turns: 1,
            flip_horizontal: true,
            vertical: 20.0,
            ..Default::default()
        };
        let l = Lens { distortion: 30.0, ..Default::default() };
        for (u, v) in [(0.3, 0.4), (0.5, 0.5), (0.8, 0.2)] {
            let (x, y) = view_to_source(300, 200, &g, &l, false, u, v);
            let (u2, v2) = source_to_view(300, 200, &g, &l, false, x, y).unwrap();
            assert!(
                (u - u2).abs() < 1e-3 && (v - v2).abs() < 1e-3,
                "{u},{v} → {x},{y} → {u2},{v2}"
            );
        }
    }

    #[test]
    fn identity_maps_centers_and_corners() {
        let g = Geometry::default();
        let m = GeoMap::new(300, 200, &g, &Lens::default(), false);
        let ((p, _), r) = (m.map(0.5, 0.5).0[1], 0);
        let _ = r;
        assert!((p - 0.5).abs() < 1e-6);
        let c = m.map(0.0, 0.0).0[1];
        assert!(c.0.abs() < 1e-5 && c.1.abs() < 1e-5);
    }

    #[test]
    fn quarter_turn_swaps_dimensions_and_maps_corners() {
        let g = Geometry { quarter_turns: 1, ..Default::default() };
        assert_eq!(output_size(300, 200, &g, false, 1000, 1000), (667, 1000));
        let m = GeoMap::new(300, 200, &g, &Lens::default(), false);
        // Rodar 90° horário: o canto superior esquerdo do resultado vem do inferior esquerdo.
        let c = m.map(0.0, 0.0).0[1];
        assert!(c.0.abs() < 1e-4 && (c.1 - 1.0).abs() < 1e-4, "{c:?}");
    }

    #[test]
    fn straighten_zooms_to_avoid_empty_corners() {
        let g = Geometry { straighten: 10.0, ..Default::default() };
        let m = GeoMap::new(300, 200, &g, &Lens::default(), false);
        for (u, v) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let p = m.map(u, v).0[1];
            assert!((-1e-4..=1.0001).contains(&p.0) && (-1e-4..=1.0001).contains(&p.1), "{p:?}");
        }
    }

    #[test]
    fn crop_and_lens_corrections() {
        let g = Geometry {
            crop: super::super::settings::Crop { x: 0.5, y: 0.5, w: 0.5, h: 0.5 },
            ..Default::default()
        };
        let m = GeoMap::new(300, 200, &g, &Lens::default(), false);
        let p = m.map(0.0, 0.0).0[1];
        assert!((p.0 - 0.5).abs() < 1e-5 && (p.1 - 0.5).abs() < 1e-5);
        // Corrigir barril: os cantos vão buscar mais perto do centro.
        let l = Lens { distortion: 50.0, ..Default::default() };
        let d = GeoMap::new(300, 200, &Geometry::default(), &l, false).map(0.0, 0.0).0[1];
        assert!(d.0 > 0.0 && d.1 > 0.0);
        // Aberração cromática: o vermelho é amostrado a outro raio que o verde.
        let ca = Lens { ca_red: 100.0, ..Default::default() };
        let q = GeoMap::new(300, 200, &Geometry::default(), &ca, false).map(0.05, 0.05).0;
        assert!((q[0].0 - q[1].0).abs() > 1e-4 && (q[2].0 - q[1].0).abs() < 1e-7);
    }
}
