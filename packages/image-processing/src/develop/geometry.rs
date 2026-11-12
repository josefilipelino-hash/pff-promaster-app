//! Geometria e correções de objetiva numa única transformação inversa: para cada píxel do
//! resultado calcula-se de onde vem no original (recorte → escala/deslocamento → proporção →
//! endireitar → perspetiva projetiva → rotações de 90°/espelho → distorção → aberração
//! cromática). Uma só reamostragem.
//!
//! A perspetiva é um modelo de câmara real: rodar a câmara virtual (inclinação e guinada) em
//! torno do centro ótico, com a distância focal da fotografia. É uma homografia — as linhas
//! retas continuam retas (o trapézio simples da versão anterior curvava as diagonais). O zoom
//! é calculado para o enquadramento nunca sair da fotografia (sem cantos vazios).

use super::settings::{Geometry, Lens};

/// Ângulo da perspetiva para ±100 no controlo.
pub const MAX_PERSPECTIVE_DEG: f32 = 40.0;
/// Diagonal do formato 35 mm (mm), para converter a focal equivalente.
const DIAG_35MM: f32 = 43.267;

pub struct GeoMap {
    /// Proporção da imagem orientada (largura/altura) e depois das rotações de 90°.
    src_aspect: f32,
    canvas_w: f32,
    canvas_h: f32,
    crop: (f32, f32, f32, f32),
    persp: Perspective,
    zoom: f32,
    turns: u8,
    flip: bool,
    k_dist: f32,
    ca_r: f32,
    ca_b: f32,
    /// Ignora o recorte (modo de recorte na interface mostra a imagem toda).
    pub uncropped: bool,
    /// 1 / (meia-diagonal)² — constante, calculada uma vez (antes: `sqrt` por píxel).
    inv_hd2: f32,
}

/// Parte da transformação anterior às rotações de 90° (unidades: altura do original = 1,
/// origem no centro do enquadramento).
#[derive(Clone, Copy)]
struct Perspective {
    cos: f32,
    sin: f32,
    /// Raio original = `r` · raio corrigido (matriz de rotação da câmara); `None` sem perspetiva.
    r: Option<[[f32; 3]; 3]>,
    f: f32,
    /// Centro da fotografia original no plano corrigido (antes de endireitar).
    c: (f32, f32),
    sx: f32,
    sy: f32,
    off: (f32, f32),
}

impl Perspective {
    fn new(g: &Geometry, cw: f32, ch: f32) -> Self {
        let a = g.straighten.clamp(-45.0, 45.0).to_radians();
        let (sin, cos) = a.sin_cos();
        // Sinal: positivo alarga o topo (como a versão anterior).
        let pitch = -(g.vertical.clamp(-100.0, 100.0) / 100.0 * MAX_PERSPECTIVE_DEG).to_radians();
        let yaw = (g.horizontal.clamp(-100.0, 100.0) / 100.0 * MAX_PERSPECTIVE_DEG).to_radians();
        let focal35 = if g.focal35 > 1.0 { g.focal35.clamp(8.0, 1200.0) } else { 35.0 };
        let f = focal35 / DIAG_35MM * (cw * cw + ch * ch).sqrt();
        let (r, c) = if pitch == 0.0 && yaw == 0.0 {
            (None, (0.0, 0.0))
        } else {
            let r = camera_rotation(pitch, yaw);
            // Onde fica o centro do original na imagem corrigida: raio corrigido = rᵀ·(0,0,f).
            let d = [r[2][0] * f, r[2][1] * f, r[2][2] * f];
            (Some(r), (f * d[0] / d[2], f * d[1] / d[2]))
        };
        let k = 1.5f32.powf(g.aspect_adjust.clamp(-100.0, 100.0) / 100.0);
        Perspective {
            cos,
            sin,
            r,
            f,
            c,
            sx: k.sqrt(),
            sy: 1.0 / k.sqrt(),
            off: (
                g.offset_x.clamp(-100.0, 100.0) / 100.0 * cw * 0.5,
                g.offset_y.clamp(-100.0, 100.0) / 100.0 * ch * 0.5,
            ),
        }
    }

    /// Ponto do enquadramento (centrado) → ponto do original (centrado), com zoom `z`.
    #[inline]
    fn apply(&self, x: f32, y: f32, z: f32) -> (f32, f32) {
        let (x, y) = ((x + self.off.0) / z / self.sx, (y + self.off.1) / z / self.sy);
        let (x, y) =
            (self.cos * x + self.sin * y + self.c.0, -self.sin * x + self.cos * y + self.c.1);
        match &self.r {
            None => (x, y),
            Some(r) => {
                let d = [
                    r[0][0] * x + r[0][1] * y + r[0][2] * self.f,
                    r[1][0] * x + r[1][1] * y + r[1][2] * self.f,
                    r[2][0] * x + r[2][1] * y + r[2][2] * self.f,
                ];
                if d[2] <= 1e-6 {
                    // Atrás da câmara: fora de qualquer fotografia.
                    return (1e6, 1e6);
                }
                (self.f * d[0] / d[2], self.f * d[1] / d[2])
            }
        }
    }

    /// Inverso de `apply` com zoom 1 e sem deslocamento: ponto do original (centrado) →
    /// ponto do enquadramento corrigido (centrado). `None` atrás da câmara.
    fn inverse(&self, px: f32, py: f32) -> Option<(f32, f32)> {
        let (x, y) = match &self.r {
            None => (px, py),
            Some(r) => {
                // rᵀ·(px, py, f)
                let d = [
                    r[0][0] * px + r[1][0] * py + r[2][0] * self.f,
                    r[0][1] * px + r[1][1] * py + r[2][1] * self.f,
                    r[0][2] * px + r[1][2] * py + r[2][2] * self.f,
                ];
                if d[2] <= 1e-6 {
                    return None;
                }
                (self.f * d[0] / d[2], self.f * d[1] / d[2])
            }
        };
        let (x, y) = (x - self.c.0, y - self.c.1);
        let (a, b) = (self.cos * x - self.sin * y, self.sin * x + self.cos * y);
        Some((a * self.sx, b * self.sy))
    }

    /// Menor zoom com o enquadramento todo dentro da fotografia.
    fn min_zoom(&self, cw: f32, ch: f32) -> f32 {
        if self.r.is_none() && self.off == (0.0, 0.0) && self.sx == 1.0 {
            // Só endireitar: forma exata (a mesma de sempre).
            let (s, c) = (self.sin.abs(), self.cos.abs());
            return ((cw * c + ch * s) / cw).max((cw * s + ch * c) / ch);
        }
        let (hw, hh) = (cw * 0.5, ch * 0.5);
        const N: usize = 24;
        let border: Vec<(f32, f32)> = (0..N)
            .flat_map(|i| {
                let t = i as f32 / N as f32;
                [
                    ((t - 0.5) * cw, -hh),
                    (hw, (t - 0.5) * ch),
                    ((0.5 - t) * cw, hh),
                    (-hw, (0.5 - t) * ch),
                ]
            })
            .collect();
        let inside = |z: f32| {
            border.iter().all(|&(x, y)| {
                let (px, py) = self.apply(x, y, z);
                px.abs() <= hw * 1.0001 && py.abs() <= hh * 1.0001
            })
        };
        let (mut lo, mut hi) = (0.05f32, 1.0f32);
        while !inside(hi) && hi < 1e4 {
            lo = hi;
            hi *= 2.0;
        }
        if inside(lo) {
            return lo;
        }
        for _ in 0..40 {
            let m = 0.5 * (lo + hi);
            if inside(m) {
                hi = m
            } else {
                lo = m
            }
        }
        hi
    }
}

/// Correção de perspetiva/endireitar de uma geometria sobre um enquadramento `cw`×`ch`
/// (unidades: altura do original = 1, origem no centro), sem zoom nem recorte — para medir
/// linhas antes e depois (Upright).
pub struct PerspectiveMap {
    p: Perspective,
}

impl PerspectiveMap {
    pub fn new(g: &Geometry, cw: f32, ch: f32) -> Self {
        let g = Geometry { offset_x: 0.0, offset_y: 0.0, ..g.clone() };
        PerspectiveMap { p: Perspective::new(&g, cw, ch) }
    }
    /// Ponto do original (antes da perspetiva) → ponto corrigido (a menos de escala).
    pub fn forward(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        self.p.inverse(x, y)
    }
    /// Ponto corrigido → ponto do original.
    pub fn backward(&self, x: f32, y: f32) -> (f32, f32) {
        self.p.apply(x, y, 1.0)
    }
}

/// Rotação da câmara: inclinação (eixo x) e depois guinada (eixo y). Linhas = raio original.
fn camera_rotation(pitch: f32, yaw: f32) -> [[f32; 3]; 3] {
    let (sp, cp) = pitch.sin_cos();
    let (sy, cy) = yaw.sin_cos();
    let rx = [[1.0, 0.0, 0.0], [0.0, cp, -sp], [0.0, sp, cp]];
    let ry = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
    let mut m = [[0f32; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| rx[i][k] * ry[k][j]).sum();
        }
    }
    m
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

/// Tamanho nativo do resultado (recorte incluído, sem ampliar nem reduzir): a
/// resolução em que a exportação revela a fotografia.
pub fn native_size(src_w: usize, src_h: usize, g: &Geometry) -> (u32, u32) {
    let (cw, ch) = if g.quarter_turns % 2 == 1 { (src_h, src_w) } else { (src_w, src_h) };
    (
        ((cw as f32 * g.crop.w.clamp(0.01, 1.0)).round() as u32).max(1),
        ((ch as f32 * g.crop.h.clamp(0.01, 1.0)).round() as u32).max(1),
    )
}

impl GeoMap {
    pub fn new(src_w: usize, src_h: usize, g: &Geometry, lens: &Lens, uncropped: bool) -> Self {
        let src_aspect = src_w as f32 / src_h as f32;
        let (canvas_w, canvas_h) =
            if g.quarter_turns % 2 == 1 { (1.0, src_aspect) } else { (src_aspect, 1.0) };
        let persp = Perspective::new(g, canvas_w, canvas_h);
        let scale = if g.scale > 0.0 { g.scale.clamp(100.0, 400.0) / 100.0 } else { 1.0 };
        let zoom = persp.min_zoom(canvas_w, canvas_h) * scale;
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
            persp,
            zoom,
            turns: g.quarter_turns % 4,
            flip: g.flip_horizontal,
            k_dist: -lens.distortion / 100.0 * 0.12,
            ca_r: lens.ca_red / 100.0 * 0.004,
            ca_b: lens.ca_blue / 100.0 * 0.004,
            uncropped,
            inv_hd2: 4.0 / (src_aspect * src_aspect + 1.0),
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
        let (x, y) =
            self.persp.apply((cx - 0.5) * self.canvas_w, (cy - 0.5) * self.canvas_h, self.zoom);
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
        let r2 = (ox * ox + oy * oy) * self.inv_hd2;
        let k = 1.0 + self.k_dist * r2;
        ox *= k;
        oy *= k;
        let to_uv = |s: f32| (ox * s / self.src_aspect + 0.5, oy * s + 0.5);
        let g = to_uv(1.0);
        ([to_uv(1.0 + self.ca_r * r2), g, to_uv(1.0 + self.ca_b * r2)], r2.sqrt())
    }

    /// Ponto do enquadramento sem recorte (0–1) → ponto do plano antes da perspetiva
    /// (0–1, depois das rotações de 90°/espelho; sem distorção da objetiva).
    pub fn frame_to_unrotated(&self, u: f32, v: f32) -> (f32, f32) {
        let (x, y) =
            self.persp.apply((u - 0.5) * self.canvas_w, (v - 0.5) * self.canvas_h, self.zoom);
        (x / self.canvas_w + 0.5, y / self.canvas_h + 0.5)
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
    #[test]
    fn perspective_keeps_straight_lines_straight() {
        let g =
            Geometry { vertical: 45.0, horizontal: -30.0, straighten: 3.0, ..Default::default() };
        let m = GeoMap::new(300, 200, &g, &Lens::default(), false);
        // Uma diagonal do resultado tem de vir de uma reta do original (o trapézio antigo curvava-a).
        let pts: Vec<(f32, f32)> =
            (0..=10).map(|i| m.map(0.1 + 0.08 * i as f32, 0.15 + 0.07 * i as f32).0[1]).collect();
        let (a, b) = (pts[0], pts[10]);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        for p in &pts {
            let dist = ((b.0 - a.0) * (a.1 - p.1) - (a.0 - p.0) * (b.1 - a.1)).abs() / len;
            assert!(dist < 1e-4, "{p:?} fora da reta ({dist})");
        }
    }

    #[test]
    fn perspective_never_shows_empty_corners() {
        for (v, h, st, asp, ox) in [
            (80.0, 0.0, 0.0, 0.0, 0.0),
            (-60.0, 50.0, 8.0, 40.0, 0.0),
            (30.0, -90.0, -5.0, -70.0, 60.0),
        ] {
            let g = Geometry {
                vertical: v,
                horizontal: h,
                straighten: st,
                aspect_adjust: asp,
                offset_x: ox,
                ..Default::default()
            };
            let m = GeoMap::new(300, 200, &g, &Lens::default(), false);
            for i in 0..=20 {
                let t = i as f32 / 20.0;
                for (u, v2) in [(t, 0.0), (t, 1.0), (0.0, t), (1.0, t)] {
                    let p = m.map(u, v2).0[1];
                    assert!(
                        (-1e-3..=1.001).contains(&p.0) && (-1e-3..=1.001).contains(&p.1),
                        "{v},{h}: ({u},{v2}) → {p:?}"
                    );
                }
            }
            // E não amplia mais do que o necessário: algum ponto da borda toca o limite.
            let touches = (0..=40).any(|i| {
                let t = i as f32 / 40.0;
                [(t, 0.0), (t, 1.0), (0.0, t), (1.0, t)].iter().any(|&(u, v2)| {
                    let p = m.map(u, v2).0[1];
                    p.0 < 0.01 || p.0 > 0.99 || p.1 < 0.01 || p.1 > 0.99
                })
            });
            assert!(touches, "{v},{h}: zoom excessivo");
        }
    }

    #[test]
    fn positive_vertical_widens_the_top() {
        // Corrige o "fugir" das verticais de um edifício fotografado de baixo: o topo da
        // fotografia é alargado (vem de uma faixa mais estreita do original).
        let g = Geometry { vertical: 50.0, ..Default::default() };
        let m = GeoMap::new(300, 200, &g, &Lens::default(), false);
        let width = |v: f32| m.map(1.0, v).0[1].0 - m.map(0.0, v).0[1].0;
        assert!(width(0.0) < width(1.0) * 0.95, "topo {} base {}", width(0.0), width(1.0));
    }

    #[test]
    fn perspective_inverse_is_exact() {
        let g = Geometry {
            vertical: -35.0,
            horizontal: 60.0,
            straighten: -4.0,
            aspect_adjust: 30.0,
            ..Default::default()
        };
        let m = PerspectiveMap::new(&g, 1.5, 1.0);
        for (x, y) in [(0.0, 0.0), (0.4, -0.3), (-0.6, 0.45)] {
            let (a, b) = m.forward(x, y).unwrap();
            let (x2, y2) = m.backward(a, b);
            assert!((x - x2).abs() < 1e-4 && (y - y2).abs() < 1e-4, "{x},{y} → {x2},{y2}");
        }
    }
}
