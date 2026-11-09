//! **Ajustes locais** (máscaras) — não destrutivos.
//!
//! Todas as formas estão em coordenadas normalizadas da fotografia **orientada** (antes do
//! recorte e do endireitar), por isso acompanham o conteúdo quando a geometria muda.
//!
//! - Formas analíticas (gradiente linear e radial) avaliam-se por píxel.
//! - Pincel e polígonos (retrato) são rasterizados numa grelha ([`Raster`]) com cache.
//! - Máscaras de IA e de céu são imagens guardadas no projeto (`masks/<id>.png`) e
//!   carregadas por identificador ([`get_raster`]).
//! - Intervalos de luminância e de cor avaliam-se sobre a cor do píxel **depois** dos ajustes
//!   globais (o que se vê).
//!
//! Combinação: cada parte soma (máximo), subtrai (×(1−v)) ou interseta (×v); inverter por
//! parte e por máscara; densidade = opacidade final.

use super::color::{luma, mul3, wb_matrix, Transfer};
use super::curves::CurveLuts;
use super::filters::gaussian;
use super::settings::*;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Lado maior das grelhas de pincel/polígonos de imagem inteira.
pub const GRID: usize = 1024;

/// Grelha de valores 0–1 sobre um retângulo (coordenadas normalizadas) da fotografia.
#[derive(Debug, Clone, PartialEq)]
pub struct Raster {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

impl Raster {
    pub fn new(bounds: [f32; 4], w: usize, h: usize) -> Self {
        Raster {
            x0: bounds[0],
            y0: bounds[1],
            x1: bounds[2],
            y1: bounds[3],
            w: w.max(1),
            h: h.max(1),
            data: vec![0.0; w.max(1) * h.max(1)],
        }
    }

    /// Grelha da imagem inteira com a proporção da fotografia (`aspect` = largura/altura).
    pub fn full(aspect: f32) -> Self {
        let (w, h) = if aspect >= 1.0 {
            (GRID, (GRID as f32 / aspect).round() as usize)
        } else {
            ((GRID as f32 * aspect).round() as usize, GRID)
        };
        Raster::new([0.0, 0.0, 1.0, 1.0], w, h)
    }

    /// Centro da célula (i, j) em coordenadas normalizadas.
    #[inline]
    pub fn cell_uv(&self, i: usize, j: usize) -> (f32, f32) {
        (
            self.x0 + (i as f32 + 0.5) / self.w as f32 * (self.x1 - self.x0),
            self.y0 + (j as f32 + 0.5) / self.h as f32 * (self.y1 - self.y0),
        )
    }

    /// Amostragem bilinear; 0 fora do retângulo.
    #[inline]
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        if u < self.x0 || v < self.y0 || u > self.x1 || v > self.y1 {
            return 0.0;
        }
        let x = ((u - self.x0) / (self.x1 - self.x0) * self.w as f32 - 0.5)
            .clamp(0.0, (self.w - 1) as f32);
        let y = ((v - self.y0) / (self.y1 - self.y0) * self.h as f32 - 0.5)
            .clamp(0.0, (self.h - 1) as f32);
        let (xi, yi) = (x as usize, y as usize);
        let (x2, y2) = ((xi + 1).min(self.w - 1), (yi + 1).min(self.h - 1));
        let (fx, fy) = (x - xi as f32, y - yi as f32);
        let a = self.data[yi * self.w + xi] * (1.0 - fx) + self.data[yi * self.w + x2] * fx;
        let b = self.data[y2 * self.w + xi] * (1.0 - fx) + self.data[y2 * self.w + x2] * fx;
        a * (1.0 - fy) + b * fy
    }

    /// Desfoca (borda suave), `sigma` em células.
    pub fn blurred(mut self, sigma: f32) -> Self {
        if sigma >= 0.3 {
            self.data = gaussian(&self.data, self.w, self.h, sigma);
        }
        self
    }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x >= e1 { 1.0 } else { 0.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Rasteriza traços de pincel (`ru`, `rv`: escala de u e v para unidades do lado maior).
pub fn rasterize_strokes(strokes: &[BrushStroke], mut r: Raster, ru: f32, rv: f32) -> Raster {
    for s in strokes {
        let rad = s.size.max(1e-4);
        let inner = rad * (1.0 - s.feather.clamp(0.0, 100.0) / 100.0);
        let flow = (s.flow.clamp(0.0, 100.0) / 100.0).max(0.01);
        // Carimbos ao longo do traço (espaçamento ¼ do raio).
        let mut stamps: Vec<(f32, f32)> = Vec::new();
        for (k, p) in s.points.iter().enumerate() {
            if k == 0 {
                stamps.push((p[0], p[1]));
                continue;
            }
            let q = s.points[k - 1];
            let (dx, dy) = ((p[0] - q[0]) * ru, (p[1] - q[1]) * rv);
            let n = ((dx * dx + dy * dy).sqrt() / (rad * 0.25)).ceil().max(1.0) as usize;
            for t in 1..=n {
                let f = t as f32 / n as f32;
                stamps.push((q[0] + (p[0] - q[0]) * f, q[1] + (p[1] - q[1]) * f));
            }
        }
        // Cobertura do traço (máximo dos carimbos) e acumulação com o fluxo.
        let mut cover = vec![0.0f32; r.data.len()];
        let (cw, ch) = ((r.x1 - r.x0) / r.w as f32, (r.y1 - r.y0) / r.h as f32);
        for &(cx, cy) in &stamps {
            let (ri, rj) =
                ((rad / ru / cw).ceil() as isize + 1, (rad / rv / ch).ceil() as isize + 1);
            let (ci, cj) = (((cx - r.x0) / cw) as isize, ((cy - r.y0) / ch) as isize);
            for j in (cj - rj).max(0)..(cj + rj + 1).min(r.h as isize) {
                for i in (ci - ri).max(0)..(ci + ri + 1).min(r.w as isize) {
                    let (u, v) = r.cell_uv(i as usize, j as usize);
                    let d = (((u - cx) * ru).powi(2) + ((v - cy) * rv).powi(2)).sqrt();
                    if d < rad {
                        let a = 1.0 - smooth(inner, rad, d);
                        let k = j as usize * r.w + i as usize;
                        cover[k] = cover[k].max(a);
                    }
                }
            }
        }
        for (m, c) in r.data.iter_mut().zip(&cover) {
            let a = c * flow;
            *m = if s.erase { *m * (1.0 - a) } else { 1.0 - (1.0 - *m) * (1.0 - a) };
        }
    }
    r
}

/// Preenche um polígono (par-ímpar) na grelha (valor 1 dentro).
pub fn fill_polygon(r: &mut Raster, pts: &[[f32; 2]], value: f32) {
    if pts.len() < 3 {
        return;
    }
    for j in 0..r.h {
        let (_, v) = r.cell_uv(0, j);
        let mut xs: Vec<f32> = Vec::new();
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            if (a[1] <= v && b[1] > v) || (b[1] <= v && a[1] > v) {
                xs.push(a[0] + (v - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
            }
        }
        xs.sort_by(f32::total_cmp);
        for pair in xs.chunks(2) {
            if pair.len() < 2 {
                continue;
            }
            for i in 0..r.w {
                let (u, _) = r.cell_uv(i, j);
                if u >= pair[0] && u <= pair[1] {
                    r.data[j * r.w + i] = value;
                }
            }
        }
    }
}

/// Origem automática para remover uma mancha/objeto em (cx, cy) com raio `r` (unidades do
/// lado maior): a zona vizinha (12 direções, 2,2–3,4 raios) com o nível e a textura mais
/// parecidos com a coroa à volta do destino.
pub fn suggest_source(src: &super::source::Source, cx: f32, cy: f32, r: f32) -> (f32, f32) {
    let pl = src.level_for(1024.0);
    let long = pl.w.max(pl.h) as f32;
    let (ru, rv) = (pl.w as f32 / long, pl.h as f32 / long);
    let stats = |x: f32, y: f32, r0: f32, r1: f32| -> Option<(f32, f32)> {
        let (mut s, mut s2, mut n) = (0.0f32, 0.0f32, 0.0f32);
        for k in 0..48 {
            let a = k as f32 / 48.0 * std::f32::consts::TAU;
            for f in [r0, (r0 + r1) / 2.0, r1] {
                let (u, v) = (x + a.cos() * f / ru, y + a.sin() * f / rv);
                let q = pl.sample(u, v)?;
                let l = luma(q);
                s += l;
                s2 += l * l;
                n += 1.0;
            }
        }
        let m = s / n;
        Some((m, (s2 / n - m * m).max(0.0).sqrt()))
    };
    let Some((ring, _)) = stats(cx, cy, r * 1.1, r * 1.5) else { return (cx + 2.5 * r / ru, cy) };
    let mut best = (cx + 2.5 * r / ru, cy, f32::MAX);
    for dist in [2.2f32, 2.8, 3.4] {
        for k in 0..12 {
            let a = k as f32 / 12.0 * std::f32::consts::TAU;
            let (sx, sy) = (cx + a.cos() * r * dist / ru, cy + a.sin() * r * dist / rv);
            if let Some((m, sd)) = stats(sx, sy, 0.0, r * 1.5) {
                let cost = (m - ring).abs() / ring.max(1e-3) + sd / m.max(1e-3) * 2.0 + dist * 0.01;
                if cost < best.2 {
                    best = (sx, sy, cost);
                }
            }
        }
    }
    (best.0, best.1)
}

// ---------------------------------------------------------------- caches e registo

type RasterCache = Mutex<VecDeque<(u64, Arc<Raster>)>>;

fn cache() -> &'static RasterCache {
    static C: OnceLock<RasterCache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Grelha em cache (chave = hash do que a define).
pub fn cached(key: u64, build: impl FnOnce() -> Raster) -> Arc<Raster> {
    if let Some(r) = cache().lock().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e.0 == key) {
        return r.1.clone();
    }
    let r = Arc::new(build());
    let mut c = cache().lock().unwrap_or_else(|e| e.into_inner());
    c.push_front((key, r.clone()));
    c.truncate(48);
    r
}

pub fn hash_of<T: serde::Serialize>(v: &T, extra: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(v).unwrap_or_default().hash(&mut h);
    extra.hash(&mut h);
    h.finish()
}

fn registry() -> &'static RwLock<HashMap<String, Arc<Raster>>> {
    static R: OnceLock<RwLock<HashMap<String, Arc<Raster>>>> = OnceLock::new();
    R.get_or_init(|| RwLock::new(HashMap::new()))
}

fn raster_paths() -> &'static RwLock<Vec<std::path::PathBuf>> {
    static P: OnceLock<RwLock<Vec<std::path::PathBuf>>> = OnceLock::new();
    P.get_or_init(|| RwLock::new(Vec::new()))
}

/// Pasta onde procurar `<id>.png` (máscaras de IA/céu de um projeto).
pub fn add_raster_path(p: std::path::PathBuf) {
    let mut g = raster_paths().write().unwrap_or_else(|e| e.into_inner());
    if !g.contains(&p) {
        g.push(p);
    }
}

pub fn register_raster(id: &str, r: Raster) -> Arc<Raster> {
    let r = Arc::new(r);
    registry().write().unwrap_or_else(|e| e.into_inner()).insert(id.into(), r.clone());
    r
}

/// Converte uma imagem de 8 bits (máscara guardada) numa grelha da imagem inteira.
pub fn raster_from_gray(img: &image::GrayImage) -> Raster {
    Raster {
        x0: 0.0,
        y0: 0.0,
        x1: 1.0,
        y1: 1.0,
        w: img.width() as usize,
        h: img.height() as usize,
        data: img.pixels().map(|p| p[0] as f32 / 255.0).collect(),
    }
}

/// Máscara guardada pelo identificador (memória ou pastas registadas).
pub fn get_raster(id: &str) -> Option<Arc<Raster>> {
    if id.is_empty() {
        return None;
    }
    if let Some(r) = registry().read().unwrap_or_else(|e| e.into_inner()).get(id) {
        return Some(r.clone());
    }
    let paths = raster_paths().read().unwrap_or_else(|e| e.into_inner()).clone();
    for d in paths {
        if let Ok(img) = image::open(d.join(format!("{id}.png"))) {
            return Some(register_raster(id, raster_from_gray(&img.to_luma8())));
        }
    }
    None
}

// ---------------------------------------------------------------- preparação e avaliação

enum PShape {
    Raster(Arc<Raster>),
    Linear {
        x0: f32,
        y0: f32,
        dx: f32,
        dy: f32,
        len2: f32,
    },
    Radial {
        cx: f32,
        cy: f32,
        cos: f32,
        sin: f32,
        rx: f32,
        ry: f32,
        inner: f32,
    },
    Lum {
        min: f32,
        max: f32,
        smooth: f32,
    },
    Color {
        hue: f32,
        range: f32,
        min_sat: f32,
        smooth: f32,
    },
    /// Máscara de IA em falta (ficheiro apagado): vazia.
    Empty,
}

struct PComp {
    op: MaskOp,
    invert: bool,
    shape: PShape,
}

/// Máscara pronta a avaliar e os seus ajustes.
pub struct PreparedMask {
    comps: Vec<PComp>,
    invert: bool,
    density: f32,
    pub adjust: LocalAdjust,
    curves: Option<CurveLuts>,
    gain: f32,
    wbm: Option<[[f32; 3]; 3]>,
    /// Precisa da cor do píxel (intervalos de luminância/cor).
    pub uses_color: bool,
    /// Tem textura, clareza, nitidez ou ruído (passo espacial).
    pub spatial: bool,
}

/// Prepara as máscaras ativas. `ru`, `rv`: escala de u e v para unidades do lado maior.
pub fn prepare(masks: &[LocalMask], ru: f32, rv: f32) -> Vec<PreparedMask> {
    let aspect = ru / rv;
    masks
        .iter()
        .filter(|m| m.enabled && !m.components.is_empty() && m.density > 0.0)
        .map(|m| {
            let comps = m
                .components
                .iter()
                .map(|c| PComp {
                    op: c.op,
                    invert: c.invert,
                    shape: match &c.shape {
                        MaskShape::Brush { strokes } => {
                            let key = hash_of(strokes, &format!("brush{aspect}"));
                            PShape::Raster(cached(key, || {
                                rasterize_strokes(strokes, Raster::full(aspect), ru, rv)
                            }))
                        }
                        MaskShape::Linear { x0, y0, x1, y1 } => {
                            let (dx, dy) = ((x1 - x0) * ru, (y1 - y0) * rv);
                            PShape::Linear {
                                x0: *x0,
                                y0: *y0,
                                dx,
                                dy,
                                len2: (dx * dx + dy * dy).max(1e-8),
                            }
                        }
                        MaskShape::Radial { cx, cy, rx, ry, angle, feather } => {
                            let a = angle.to_radians();
                            PShape::Radial {
                                cx: *cx,
                                cy: *cy,
                                cos: a.cos(),
                                sin: a.sin(),
                                rx: rx.max(1e-4),
                                ry: ry.max(1e-4),
                                inner: 1.0 - feather.clamp(0.0, 100.0) / 100.0,
                            }
                        }
                        MaskShape::Luminance { min, max, smooth } => {
                            PShape::Lum { min: *min, max: *max, smooth: smooth.max(0.0) }
                        }
                        MaskShape::Color { hue, range, min_sat, smooth } => PShape::Color {
                            hue: *hue,
                            range: range.max(1.0),
                            min_sat: *min_sat,
                            smooth: smooth.max(0.0),
                        },
                        MaskShape::Raster { raster, feather, .. } => match get_raster(raster) {
                            Some(r) if *feather > 0.0 => {
                                let f = *feather;
                                let key = hash_of(raster, &format!("feather{f}"));
                                PShape::Raster(cached(key, || {
                                    (*r).clone().blurred(f / 100.0 * r.w.max(r.h) as f32 * 0.02)
                                }))
                            }
                            Some(r) => PShape::Raster(r),
                            None => PShape::Empty,
                        },
                    },
                })
                .collect::<Vec<_>>();
            let a = m.adjust;
            let tone = a.contrast != 0.0
                || a.highlights != 0.0
                || a.shadows != 0.0
                || a.whites != 0.0
                || a.blacks != 0.0;
            let uses_color =
                comps.iter().any(|c| matches!(c.shape, PShape::Lum { .. } | PShape::Color { .. }));
            PreparedMask {
                comps,
                invert: m.invert,
                density: m.density.clamp(0.0, 100.0) / 100.0,
                adjust: a,
                curves: tone.then(|| {
                    CurveLuts::new(
                        &Light {
                            exposure: 0.0,
                            contrast: a.contrast,
                            highlights: a.highlights,
                            shadows: a.shadows,
                            whites: a.whites,
                            blacks: a.blacks,
                        },
                        &Curve::default(),
                    )
                }),
                gain: 2f32.powf(a.exposure.clamp(-5.0, 5.0)),
                wbm: (a.temperature != 0.0 || a.tint != 0.0)
                    .then(|| wb_matrix(a.temperature, a.tint)),
                uses_color,
                spatial: a.texture != 0.0
                    || a.clarity != 0.0
                    || a.sharpness != 0.0
                    || a.noise > 0.0,
            }
        })
        .collect()
}

fn hsv(c: [f32; 3]) -> (f32, f32) {
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
    (h, if mx <= 1e-6 { 0.0 } else { d / mx })
}

impl PreparedMask {
    /// Valor da máscara (0–1) no ponto (u, v) da fotografia, com a cor `d` (visualização).
    pub fn value(&self, u: f32, v: f32, d: [f32; 3], ru: f32, rv: f32) -> f32 {
        let mut m = 0.0f32;
        for (k, c) in self.comps.iter().enumerate() {
            let mut x = match &c.shape {
                PShape::Raster(r) => r.sample(u, v),
                PShape::Empty => 0.0,
                PShape::Linear { x0, y0, dx, dy, len2 } => {
                    let t = ((u - x0) * ru * dx + (v - y0) * rv * dy) / len2;
                    1.0 - smooth(0.0, 1.0, t)
                }
                PShape::Radial { cx, cy, cos, sin, rx, ry, inner } => {
                    let (px, py) = ((u - cx) * ru, (v - cy) * rv);
                    let (qx, qy) = (px * cos + py * sin, -px * sin + py * cos);
                    let e = ((qx / rx).powi(2) + (qy / ry).powi(2)).sqrt();
                    1.0 - smooth(*inner, 1.0, e)
                }
                PShape::Lum { min, max, smooth: s } => {
                    let l = luma(d);
                    smooth(min - s, *min, l) * (1.0 - smooth(*max, max + s, l))
                }
                PShape::Color { hue, range, min_sat, smooth: s } => {
                    let (h, sat) = hsv(d.map(|c| c.clamp(0.0, 1.0)));
                    let mut dh = (h - hue).abs() % 360.0;
                    if dh > 180.0 {
                        dh = 360.0 - dh;
                    }
                    let sw = s * 60.0;
                    (1.0 - smooth(*range, range + sw.max(1.0), dh))
                        * smooth(min_sat * 0.5, min_sat.max(0.01), sat)
                }
            };
            if c.invert {
                x = 1.0 - x;
            }
            m = if k == 0 && c.op != MaskOp::Intersect {
                if c.op == MaskOp::Subtract {
                    0.0
                } else {
                    x
                }
            } else {
                match c.op {
                    MaskOp::Add => m.max(x),
                    MaskOp::Subtract => m * (1.0 - x),
                    MaskOp::Intersect => m * x,
                }
            };
        }
        if self.invert {
            m = 1.0 - m;
        }
        (m * self.density).clamp(0.0, 1.0)
    }

    /// Ajustes não espaciais no domínio de visualização, com a força `m`.
    pub fn apply(&self, mut d: [f32; 3], m: f32, tr: &Transfer) -> [f32; 3] {
        if m <= 0.0 {
            return d;
        }
        if self.gain != 1.0 || self.wbm.is_some() {
            let mut lin = d.map(|c| tr.to_linear.get(c));
            let g = 1.0 + (self.gain - 1.0) * m;
            lin = lin.map(|c| c * g);
            if let Some(w) = &self.wbm {
                let t = mul3(w, lin);
                lin = std::array::from_fn(|i| lin[i] + (t[i] - lin[i]) * m);
            }
            d = lin.map(|c| tr.to_display.get(c.max(0.0)));
        }
        if let Some(c) = &self.curves {
            d = std::array::from_fn(|i| d[i] + (c.apply(i, d[i]) - d[i]) * m);
        }
        if self.adjust.saturation != 0.0 {
            let l = luma(d);
            let k = (1.0 + self.adjust.saturation / 100.0 * m).max(0.0);
            d = d.map(|c| l + (c - l) * k);
        }
        d
    }
}

/// Planos por píxel dos ajustes locais espaciais (para o passo espacial do render).
#[derive(Default)]
pub struct LocalSpatial {
    /// (força da máscara por píxel, ajustes).
    pub planes: Vec<(Vec<f32>, LocalAdjust)>,
}

impl LocalSpatial {
    pub fn is_empty(&self) -> bool {
        self.planes.is_empty()
    }
    /// Soma por píxel de `m × campo/100`.
    pub fn amount(&self, i: usize, f: fn(&LocalAdjust) -> f32) -> f32 {
        self.planes.iter().map(|(p, a)| p[i] * f(a) / 100.0).sum()
    }
    pub fn any(&self, f: fn(&LocalAdjust) -> f32) -> bool {
        self.planes.iter().any(|(_, a)| f(a) != 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_and_combination() {
        let (ru, rv) = (1.0, 2.0 / 3.0);
        let lin = MaskComponent {
            shape: MaskShape::Linear { x0: 0.2, y0: 0.5, x1: 0.4, y1: 0.5 },
            ..Default::default()
        };
        let rad = MaskComponent {
            shape: MaskShape::Radial {
                cx: 0.7,
                cy: 0.5,
                rx: 0.1,
                ry: 0.1,
                angle: 0.0,
                feather: 50.0,
            },
            ..Default::default()
        };
        let mk = |comps: Vec<MaskComponent>, invert: bool, density: f32| {
            prepare(
                &[LocalMask { components: comps, invert, density, ..Default::default() }],
                ru,
                rv,
            )
            .remove(0)
        };
        let d = [0.5; 3];
        let m = mk(vec![lin.clone()], false, 100.0);
        assert_eq!(m.value(0.1, 0.5, d, ru, rv), 1.0);
        assert!((m.value(0.3, 0.5, d, ru, rv) - 0.5).abs() < 0.01);
        assert_eq!(m.value(0.5, 0.5, d, ru, rv), 0.0);
        let r = mk(vec![rad.clone()], false, 100.0);
        assert_eq!(r.value(0.7, 0.5, d, ru, rv), 1.0);
        assert_eq!(r.value(0.9, 0.5, d, ru, rv), 0.0);
        assert!((0.1..0.9).contains(&r.value(0.775, 0.5, d, ru, rv)));
        // Somar, subtrair, intersetar, inverter, densidade.
        let add = mk(vec![lin.clone(), rad.clone()], false, 100.0);
        assert_eq!((add.value(0.1, 0.5, d, ru, rv), add.value(0.7, 0.5, d, ru, rv)), (1.0, 1.0));
        let sub = mk(
            vec![lin.clone(), MaskComponent { op: MaskOp::Subtract, ..lin.clone() }],
            false,
            100.0,
        );
        assert_eq!(sub.value(0.1, 0.5, d, ru, rv), 0.0);
        let inter = mk(
            vec![lin.clone(), MaskComponent { op: MaskOp::Intersect, ..rad.clone() }],
            false,
            100.0,
        );
        assert_eq!(
            (inter.value(0.1, 0.5, d, ru, rv), inter.value(0.7, 0.5, d, ru, rv)),
            (0.0, 0.0)
        );
        let inv = mk(vec![rad.clone()], true, 50.0);
        assert_eq!((inv.value(0.7, 0.5, d, ru, rv), inv.value(0.1, 0.5, d, ru, rv)), (0.0, 0.5));
        let compinv = mk(vec![MaskComponent { invert: true, ..rad }], false, 100.0);
        assert_eq!(compinv.value(0.1, 0.5, d, ru, rv), 1.0);
        // Intervalos de luminância e de cor.
        let lum = mk(
            vec![MaskComponent {
                shape: MaskShape::Luminance { min: 0.6, max: 1.0, smooth: 0.1 },
                ..Default::default()
            }],
            false,
            100.0,
        );
        assert_eq!(
            (lum.value(0.5, 0.5, [0.9; 3], ru, rv), lum.value(0.5, 0.5, [0.3; 3], ru, rv)),
            (1.0, 0.0)
        );
        let col = mk(
            vec![MaskComponent {
                shape: MaskShape::Color { hue: 220.0, range: 20.0, min_sat: 0.2, smooth: 0.3 },
                ..Default::default()
            }],
            false,
            100.0,
        );
        assert!(col.value(0.5, 0.5, [0.2, 0.4, 0.9], ru, rv) > 0.9);
        assert_eq!(col.value(0.5, 0.5, [0.9, 0.3, 0.2], ru, rv), 0.0);
        assert_eq!(col.value(0.5, 0.5, [0.5, 0.5, 0.52], ru, rv), 0.0, "cinzento não conta");
    }

    #[test]
    fn brush_flow_feather_and_erase() {
        let (ru, rv) = (1.0, 0.5);
        let stroke = |flow: f32, erase: bool| BrushStroke {
            points: vec![[0.2, 0.5], [0.8, 0.5]],
            size: 0.05,
            feather: 40.0,
            flow,
            erase,
        };
        let r = rasterize_strokes(&[stroke(100.0, false)], Raster::full(2.0), ru, rv);
        assert!(r.sample(0.5, 0.5) > 0.99 && r.sample(0.5, 0.9) == 0.0);
        let soft = r.sample(0.5, 0.5 + 0.045 / rv);
        assert!(soft > 0.0 && soft < 0.6, "borda suave: {soft}");
        let half = rasterize_strokes(&[stroke(50.0, false)], Raster::full(2.0), ru, rv);
        assert!((half.sample(0.5, 0.5) - 0.5).abs() < 0.02);
        let twice = rasterize_strokes(
            &[stroke(50.0, false), stroke(50.0, false)],
            Raster::full(2.0),
            ru,
            rv,
        );
        assert!((twice.sample(0.5, 0.5) - 0.75).abs() < 0.02, "fluxo acumula");
        let erased = rasterize_strokes(
            &[stroke(100.0, false), stroke(100.0, true)],
            Raster::full(2.0),
            ru,
            rv,
        );
        assert!(erased.sample(0.5, 0.5) < 0.01);
        let mut poly = Raster::new([0.4, 0.4, 0.6, 0.6], 64, 64);
        fill_polygon(&mut poly, &[[0.45, 0.45], [0.55, 0.45], [0.55, 0.55], [0.45, 0.55]], 1.0);
        assert_eq!(
            (poly.sample(0.5, 0.5), poly.sample(0.42, 0.5), poly.sample(0.9, 0.9)),
            (1.0, 0.0, 0.0)
        );
    }
}
