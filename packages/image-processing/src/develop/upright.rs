//! Upright: correção automática da perspetiva a partir das linhas da fotografia.
//!
//! 1. Deteção de segmentos de reta (crescimento de regiões de gradiente com a mesma
//!    orientação, como o LSD de von Gioi et al.): retângulo ajustado por momentos de inércia,
//!    aceite só se for comprido, fino e denso.
//! 2. Pontos de fuga por RANSAC no modelo de câmara (cada segmento é um plano pelo centro
//!    ótico; o ponto de fuga é a direção comum), refinados por mínimos quadrados.
//! 3. Rotação da câmara (inclinação, guinada, rotação) por Levenberg–Marquardt, a minimizar
//!    o desvio angular dos segmentos que pertencem a cada ponto de fuga, depois da correção
//!    pelo mesmo modelo projetivo que o render usa (`geometry::PerspectiveMap`).
//!
//! Modos: nível (só rotação), vertical (verticais paralelas + nível), completo (verticais e
//! horizontais), automático (completo só quando as horizontais são fiáveis e a correção é
//! moderada) e guiado (as linhas que o utilizador desenha).

use super::geometry::PerspectiveMap;
use super::settings::Geometry;

/// Diagonal do formato 35 mm (mm).
const DIAG_35MM: f64 = 43.267;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UprightMode {
    Level,
    Vertical,
    Full,
    Auto,
    Guided,
}

impl UprightMode {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "level" => UprightMode::Level,
            "vertical" => UprightMode::Vertical,
            "full" => UprightMode::Full,
            "auto" => UprightMode::Auto,
            "guided" => UprightMode::Guided,
            _ => return None,
        })
    }
    pub fn id(self) -> &'static str {
        match self {
            UprightMode::Level => "level",
            UprightMode::Vertical => "vertical",
            UprightMode::Full => "full",
            UprightMode::Auto => "auto",
            UprightMode::Guided => "guided",
        }
    }
}

/// Segmento em píxeis da imagem analisada.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl Segment {
    pub fn len(&self) -> f32 {
        ((self.x2 - self.x1).powi(2) + (self.y2 - self.y1).powi(2)).sqrt()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UprightResult {
    /// Valores dos controlos (mesmas unidades de `Geometry`).
    pub vertical: f32,
    pub horizontal: f32,
    pub straighten: f32,
    /// Modo efetivamente aplicado (o automático escolhe um dos outros).
    pub mode: UprightMode,
    /// Segmentos usados (verticais, horizontais).
    pub lines: (usize, usize),
    /// Desvio angular médio (RMS, graus) dessas linhas antes e depois.
    pub rms_before: f32,
    pub rms_after: f32,
}

// ---------------------------------------------------------------------------------------
// 1. Segmentos
// ---------------------------------------------------------------------------------------

/// Deteta segmentos de reta numa imagem em tons de cinzento (0–1, `w`×`h`).
pub fn detect_segments(gray: &[f32], w: usize, h: usize) -> Vec<Segment> {
    if w < 8 || h < 8 || gray.len() != w * h {
        return Vec::new();
    }
    let img = blur3(gray, w, h);
    // Gradiente 2×2 (centrado em x+½, y+½), ângulo da linha de nível.
    let (gw, gh) = (w - 1, h - 1);
    let mut mag = vec![0f32; gw * gh];
    let mut ang = vec![0f32; gw * gh];
    for y in 0..gh {
        for x in 0..gw {
            let a = img[y * w + x];
            let b = img[y * w + x + 1];
            let c = img[(y + 1) * w + x];
            let d = img[(y + 1) * w + x + 1];
            let gx = (b + d - a - c) * 0.5;
            let gy = (c + d - a - b) * 0.5;
            mag[y * gw + x] = (gx * gx + gy * gy).sqrt();
            ang[y * gw + x] = gx.atan2(-gy);
        }
    }
    let tau = 22.5f32.to_radians();
    // Limiar do LSD (q = 2 níveis em 255, ruído de quantização) — um pouco mais alto para
    // ignorar o grão de ISO alto.
    let rho = (3.0 / 255.0) / tau.sin();
    let mut order: Vec<u32> = (0..gw * gh).filter(|&i| mag[i] > rho).map(|i| i as u32).collect();
    order.sort_unstable_by(|&a, &b| mag[b as usize].total_cmp(&mag[a as usize]));
    let mut used = vec![false; gw * gh];
    let min_len = 0.035 * gw.max(gh) as f32;
    let mut out = Vec::new();
    let mut region: Vec<usize> = Vec::new();
    for &seed in &order {
        let seed = seed as usize;
        if used[seed] {
            continue;
        }
        region.clear();
        region.push(seed);
        used[seed] = true;
        let (mut sc, mut ss) = (ang[seed].cos(), ang[seed].sin());
        let mut theta = ang[seed];
        let mut k = 0;
        while k < region.len() {
            let i = region[k];
            k += 1;
            let (x, y) = ((i % gw) as isize, (i / gw) as isize);
            for dy in -1..=1isize {
                for dx in -1..=1isize {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= gw as isize || ny >= gh as isize {
                        continue;
                    }
                    let n = ny as usize * gw + nx as usize;
                    if used[n] || mag[n] <= rho {
                        continue;
                    }
                    if angle_diff(ang[n], theta) < tau {
                        used[n] = true;
                        region.push(n);
                        sc += ang[n].cos();
                        ss += ang[n].sin();
                        theta = ss.atan2(sc);
                    }
                }
            }
        }
        if region.len() < 12 {
            continue;
        }
        if let Some(s) = fit_rect(&region, &mag, gw, theta, min_len) {
            out.push(s);
        }
    }
    out
}

fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (a - b).abs() % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d = std::f32::consts::TAU - d;
    }
    d
}

fn blur3(g: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut t = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let l = g[y * w + x.saturating_sub(1)];
            let r = g[y * w + (x + 1).min(w - 1)];
            t[y * w + x] = 0.25 * l + 0.5 * g[y * w + x] + 0.25 * r;
        }
    }
    let mut o = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let u = t[y.saturating_sub(1) * w + x];
            let d = t[(y + 1).min(h - 1) * w + x];
            o[y * w + x] = 0.25 * u + 0.5 * t[y * w + x] + 0.25 * d;
        }
    }
    o
}

/// Retângulo da região (momentos de inércia ponderados pelo gradiente): devolve o segmento
/// se for comprido, fino e denso.
fn fit_rect(region: &[usize], mag: &[f32], gw: usize, theta: f32, min_len: f32) -> Option<Segment> {
    let (mut sw, mut cx, mut cy) = (0f64, 0f64, 0f64);
    for &i in region {
        let m = mag[i] as f64;
        sw += m;
        cx += m * ((i % gw) as f64 + 0.5);
        cy += m * ((i / gw) as f64 + 0.5);
    }
    cx /= sw;
    cy /= sw;
    let (mut ixx, mut iyy, mut ixy) = (0f64, 0f64, 0f64);
    for &i in region {
        let m = mag[i] as f64;
        let (dx, dy) = ((i % gw) as f64 + 0.5 - cx, (i / gw) as f64 + 0.5 - cy);
        ixx += m * dy * dy;
        iyy += m * dx * dx;
        ixy -= m * dx * dy;
    }
    // Direção principal (menor valor próprio da matriz de inércia, como no LSD).
    let lambda = 0.5 * (ixx + iyy - ((ixx - iyy).powi(2) + 4.0 * ixy * ixy).sqrt());
    let mut a =
        if ixx.abs() > iyy.abs() { (lambda - ixx).atan2(ixy) } else { ixy.atan2(lambda - iyy) };
    // Mesmo sentido do ângulo da região (±π).
    if angle_diff(a as f32, theta) > std::f32::consts::FRAC_PI_2 {
        a += std::f64::consts::PI;
    }
    let (dx, dy) = (a.cos(), a.sin());
    let (mut l0, mut l1, mut w0, mut w1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &i in region {
        let (px, py) = ((i % gw) as f64 + 0.5 - cx, (i / gw) as f64 + 0.5 - cy);
        let l = px * dx + py * dy;
        let wv = -px * dy + py * dx;
        l0 = l0.min(l);
        l1 = l1.max(l);
        w0 = w0.min(wv);
        w1 = w1.max(wv);
    }
    let len = l1 - l0;
    let width = w1 - w0 + 1.0;
    if len < min_len as f64
        || len / width < 5.0
        || region.len() as f64 / (len.max(1.0) * width) < 0.55
    {
        return None;
    }
    Some(Segment {
        x1: (cx + l0 * dx) as f32,
        y1: (cy + l0 * dy) as f32,
        x2: (cx + l1 * dx) as f32,
        y2: (cy + l1 * dy) as f32,
    })
}

// ---------------------------------------------------------------------------------------
// 2. Pontos de fuga
// ---------------------------------------------------------------------------------------

/// Segmento no enquadramento centrado (unidades: altura = 1), com o plano pelo centro ótico.
#[derive(Clone, Copy, Debug)]
struct Line {
    p1: (f64, f64),
    p2: (f64, f64),
    n: [f64; 3],
    len: f64,
}

impl Line {
    fn new(p1: (f64, f64), p2: (f64, f64), f: f64) -> Option<Line> {
        let a = [p1.0, p1.1, f];
        let b = [p2.0, p2.1, f];
        let n = normalize(cross(a, b))?;
        let len = ((p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2)).sqrt();
        (len > 0.0).then_some(Line { p1, p2, n, len })
    }
    /// Ângulo (graus, 0–90) entre a reta e a direção vertical da imagem.
    fn tilt_from_vertical(&self) -> f64 {
        let (dx, dy) = (self.p2.0 - self.p1.0, self.p2.1 - self.p1.1);
        dx.abs().atan2(dy.abs()).to_degrees()
    }
    /// Erro angular (graus) da reta em relação ao ponto de fuga `d` (direção 3D).
    fn vp_error(&self, d: [f64; 3], f: f64) -> f64 {
        let m = ((self.p1.0 + self.p2.0) * 0.5, (self.p1.1 + self.p2.1) * 0.5);
        let to_vp = if d[2].abs() > 1e-9 {
            (f * d[0] / d[2] - m.0, f * d[1] / d[2] - m.1)
        } else {
            (d[0], d[1])
        };
        let seg = (self.p2.0 - self.p1.0, self.p2.1 - self.p1.1);
        let n = (to_vp.0.hypot(to_vp.1) * seg.0.hypot(seg.1)).max(1e-12);
        let c = ((to_vp.0 * seg.0 + to_vp.1 * seg.1) / n).abs().min(1.0);
        c.acos().to_degrees()
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (n > 1e-12).then(|| [v[0] / n, v[1] / n, v[2] / n])
}

/// Ponto de fuga com mais suporte (comprimento total) entre as retas dadas.
/// Devolve a direção e os índices das retas que lhe pertencem.
fn vanishing_point(lines: &[Line], f: f64, tol_deg: f64) -> Option<([f64; 3], Vec<usize>)> {
    if lines.len() < 2 {
        return None;
    }
    // Candidatas: as mais compridas (estáveis); pares determinísticos (gerador simples).
    let mut idx: Vec<usize> = (0..lines.len()).collect();
    idx.sort_by(|&a, &b| lines[b].len.total_cmp(&lines[a].len));
    idx.truncate(120);
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let score = |d: [f64; 3]| -> f64 {
        lines.iter().filter(|l| l.vp_error(d, f) < tol_deg).map(|l| l.len).sum()
    };
    let mut best: Option<([f64; 3], f64)> = None;
    let pairs = if idx.len() <= 25 { idx.len() * idx.len() } else { 600 };
    for k in 0..pairs {
        let (i, j) = if idx.len() <= 25 {
            (idx[k / idx.len()], idx[k % idx.len()])
        } else {
            (idx[next(idx.len())], idx[next(idx.len())])
        };
        if i >= j && idx.len() <= 25 || i == j {
            continue;
        }
        let Some(d) = normalize(cross(lines[i].n, lines[j].n)) else { continue };
        let s = score(d);
        if best.is_none_or(|b| s > b.1) {
            best = Some((d, s));
        }
    }
    let (mut d, _) = best?;
    // Refinar: direção que melhor pertence a todos os planos (menor vetor próprio de Σ w·n·nᵀ).
    for _ in 0..3 {
        let inl: Vec<usize> =
            (0..lines.len()).filter(|&k| lines[k].vp_error(d, f) < tol_deg).collect();
        if inl.len() < 2 {
            return None;
        }
        let mut m = [[0f64; 3]; 3];
        for &k in &inl {
            let (n, w) = (lines[k].n, lines[k].len);
            for (a, row) in m.iter_mut().enumerate() {
                for (b, v) in row.iter_mut().enumerate() {
                    *v += w * n[a] * n[b];
                }
            }
        }
        d = smallest_eigenvector(m);
    }
    let inl: Vec<usize> = (0..lines.len()).filter(|&k| lines[k].vp_error(d, f) < tol_deg).collect();
    (inl.len() >= 2).then_some((d, inl))
}

/// Vetor próprio do menor valor próprio de uma matriz simétrica 3×3 (Jacobi).
#[allow(clippy::needless_range_loop)] // álgebra de matrizes: índices explícitos
fn smallest_eigenvector(mut a: [[f64; 3]; 3]) -> [f64; 3] {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        let (mut p, mut q, mut big) = (0, 1, 0.0);
        for i in 0..3 {
            for j in i + 1..3 {
                if a[i][j].abs() > big {
                    big = a[i][j].abs();
                    p = i;
                    q = j;
                }
            }
        }
        if big < 1e-15 {
            break;
        }
        let theta = 0.5 * (2.0 * a[p][q]).atan2(a[q][q] - a[p][p]);
        let (s, c) = theta.sin_cos();
        let mut b = a;
        for k in 0..3 {
            b[k][p] = c * a[k][p] - s * a[k][q];
            b[k][q] = s * a[k][p] + c * a[k][q];
        }
        a = b;
        for k in 0..3 {
            b[p][k] = c * a[p][k] - s * a[q][k];
            b[q][k] = s * a[p][k] + c * a[q][k];
        }
        a = b;
        for row in &mut v {
            let (x, y) = (row[p], row[q]);
            row[p] = c * x - s * y;
            row[q] = s * x + c * y;
        }
    }
    let k = (0..3).min_by(|&i, &j| a[i][i].total_cmp(&a[j][j])).unwrap_or(0);
    [v[0][k], v[1][k], v[2][k]]
}

// ---------------------------------------------------------------------------------------
// 3. Rotação da câmara
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Vertical,
    Horizontal,
}

/// Desvio angular (graus) de cada reta depois de corrigida com os parâmetros dados.
fn residuals(
    lines: &[(Line, Kind)],
    params: [f64; 3],
    cw: f32,
    ch: f32,
    focal35: f32,
    total: f64,
) -> Vec<f64> {
    let g = Geometry {
        vertical: params[0] as f32,
        horizontal: params[1] as f32,
        straighten: params[2] as f32,
        focal35,
        ..Default::default()
    };
    let m = PerspectiveMap::new(&g, cw, ch);
    lines
        .iter()
        .map(|(l, k)| {
            let a = m.forward(l.p1.0 as f32, l.p1.1 as f32);
            let b = m.forward(l.p2.0 as f32, l.p2.1 as f32);
            let dev = match (a, b) {
                (Some(a), Some(b)) => {
                    let (dx, dy) = ((b.0 - a.0) as f64, (b.1 - a.1) as f64);
                    match k {
                        Kind::Vertical => (dx / dy).atan().to_degrees(),
                        Kind::Horizontal => (dy / dx).atan().to_degrees(),
                    }
                }
                _ => 90.0,
            };
            dev * (l.len / total).sqrt()
        })
        .collect()
}

fn rms(r: &[f64]) -> f64 {
    r.iter().map(|v| v * v).sum::<f64>().sqrt()
}

/// Levenberg–Marquardt nos parâmetros livres (`free`); os outros ficam a zero.
fn solve(
    lines: &[(Line, Kind)],
    free: [bool; 3],
    cw: f32,
    ch: f32,
    focal35: f32,
) -> ([f64; 3], f64, f64) {
    let total: f64 = lines.iter().map(|l| l.0.len).sum::<f64>().max(1e-9);
    let limits = [100.0, 100.0, 45.0];
    let mut p = [0f64; 3];
    let mut r = residuals(lines, p, cw, ch, focal35, total);
    let before = rms(&r);
    let mut cost = before;
    let mut lambda = 1e-2;
    for _ in 0..60 {
        // Jacobiano numérico.
        let cols: Vec<usize> = (0..3).filter(|&i| free[i]).collect();
        let mut jac = vec![vec![0f64; cols.len()]; r.len()];
        for (c, &i) in cols.iter().enumerate() {
            let mut q = p;
            let h = 0.05;
            q[i] += h;
            let rq = residuals(lines, q, cw, ch, focal35, total);
            for (row, (a, b)) in jac.iter_mut().zip(rq.iter().zip(&r)) {
                row[c] = (a - b) / h;
            }
        }
        let n = cols.len();
        let mut jtj = vec![vec![0f64; n]; n];
        let mut jtr = vec![0f64; n];
        for (row, ri) in jac.iter().zip(&r) {
            for a in 0..n {
                jtr[a] += row[a] * ri;
                for b in 0..n {
                    jtj[a][b] += row[a] * row[b];
                }
            }
        }
        let mut improved = false;
        for _ in 0..8 {
            let mut m = jtj.clone();
            for (a, row) in m.iter_mut().enumerate() {
                row[a] += lambda * (1.0 + row[a]);
            }
            let Some(step) = solve_linear(m, jtr.iter().map(|v| -v).collect()) else { break };
            let mut q = p;
            for (c, &i) in cols.iter().enumerate() {
                q[i] = (q[i] + step[c]).clamp(-limits[i], limits[i]);
            }
            let rq = residuals(lines, q, cw, ch, focal35, total);
            let cq = rms(&rq);
            if cq < cost {
                let gain = cost - cq;
                p = q;
                r = rq;
                cost = cq;
                lambda = (lambda * 0.3).max(1e-7);
                improved = gain > 1e-7;
                break;
            }
            lambda *= 10.0;
        }
        if !improved {
            break;
        }
    }
    (p, before, cost)
}

#[allow(clippy::needless_range_loop)] // eliminação de Gauss: índices explícitos
fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let piv = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[piv][c].abs() < 1e-14 {
            return None;
        }
        a.swap(c, piv);
        b.swap(c, piv);
        for r in 0..n {
            if r != c {
                let f = a[r][c] / a[c][c];
                for k in c..n {
                    a[r][k] -= f * a[c][k];
                }
                b[r] -= f * b[c];
            }
        }
    }
    Some((0..n).map(|i| b[i] / a[i][i]).collect())
}

/// Calcula a correção Upright.
///
/// * `segments`: segmentos em píxeis de uma imagem `w`×`h` (a fotografia orientada, sem
///   perspetiva nem endireitar) — ou, no modo guiado, as guias do utilizador.
/// * `focal35`: distância focal equivalente a 35 mm (EXIF); `None` = 35 mm.
pub fn solve_upright(
    segments: &[Segment],
    w: usize,
    h: usize,
    focal35: Option<f32>,
    mode: UprightMode,
) -> Option<UprightResult> {
    let (cw, ch) = (w as f32 / h as f32, 1.0f32);
    let focal35 = focal35.filter(|f| *f > 1.0).unwrap_or(35.0);
    let f = focal35 as f64 / DIAG_35MM * ((cw * cw + ch * ch) as f64).sqrt();
    let to_c =
        |x: f32, y: f32| (((x / w as f32) - 0.5) as f64 * cw as f64, ((y / h as f32) - 0.5) as f64);
    let lines: Vec<Line> =
        segments.iter().filter_map(|s| Line::new(to_c(s.x1, s.y1), to_c(s.x2, s.y2), f)).collect();
    // Classes pela inclinação na imagem.
    let (vert, horz): (Vec<Line>, Vec<Line>) = if mode == UprightMode::Guided {
        lines.iter().partition(|l| l.tilt_from_vertical() < 45.0)
    } else {
        let v = lines.iter().filter(|l| l.tilt_from_vertical() < 30.0).copied().collect();
        let h = lines.iter().filter(|l| l.tilt_from_vertical() > 60.0).copied().collect();
        (v, h)
    };
    let tol = 2.0;
    let (vp_v, vp_h) = if mode == UprightMode::Guided {
        // As guias são exatas: todas contam.
        (
            (vert.len() >= 2).then(|| (0..vert.len()).collect::<Vec<_>>()),
            (horz.len() >= 2).then(|| (0..horz.len()).collect::<Vec<_>>()),
        )
    } else {
        (vanishing_point(&vert, f, tol).map(|v| v.1), vanishing_point(&horz, f, tol).map(|v| v.1))
    };
    // Suporte mínimo: comprimento total em alturas da imagem.
    let support = |ls: &[Line], idx: &Option<Vec<usize>>| {
        idx.as_ref().map_or(0.0, |i| i.iter().map(|&k| ls[k].len).sum::<f64>())
    };
    let (sv, sh) = (support(&vert, &vp_v), support(&horz, &vp_h));
    let pick = |ls: &[Line], idx: &Option<Vec<usize>>, k: Kind| -> Vec<(Line, Kind)> {
        idx.as_ref().map(|i| i.iter().map(|&j| (ls[j], k)).collect()).unwrap_or_default()
    };
    let v_lines = pick(&vert, &vp_v, Kind::Vertical);
    let h_lines = pick(&horz, &vp_h, Kind::Horizontal);
    let reliable_v = if mode == UprightMode::Guided {
        v_lines.len() >= 2
    } else {
        v_lines.len() >= 2 && sv >= 0.4
    };
    let reliable_h = if mode == UprightMode::Guided {
        h_lines.len() >= 2
    } else {
        h_lines.len() >= 3 && sh >= 0.6
    };

    let run = |set: Vec<(Line, Kind)>, free: [bool; 3], m: UprightMode| -> Option<UprightResult> {
        if set.is_empty() {
            return None;
        }
        let (p, before, after) = solve(&set, free, cw, ch, focal35);
        let nv = set.iter().filter(|l| l.1 == Kind::Vertical).count();
        Some(UprightResult {
            vertical: p[0] as f32,
            horizontal: p[1] as f32,
            straighten: p[2] as f32,
            mode: m,
            lines: (nv, set.len() - nv),
            rms_before: before as f32,
            rms_after: after as f32,
        })
    };
    let both = || v_lines.iter().chain(&h_lines).copied().collect::<Vec<_>>();
    match mode {
        UprightMode::Level => {
            // Só rodar. Robusto: o pico (densidade ponderada pelo comprimento) dos desvios das
            // retas quase verticais/horizontais é a rotação; só as retas que concordam com o
            // pico entram no ajuste fino (texturas e diagonais ficam de fora).
            let cands: Vec<(Line, Kind, f64)> = vert
                .iter()
                .filter(|l| l.tilt_from_vertical() < 15.0)
                .map(|l| (*l, Kind::Vertical))
                .chain(
                    horz.iter()
                        .filter(|l| l.tilt_from_vertical() > 75.0)
                        .map(|l| (*l, Kind::Horizontal)),
                )
                .map(|(l, k)| {
                    let (dx, dy) = (l.p2.0 - l.p1.0, l.p2.1 - l.p1.1);
                    let dev = match k {
                        Kind::Vertical => (dx / dy).atan().to_degrees(),
                        Kind::Horizontal => (dy / dx).atan().to_degrees(),
                    };
                    (l, k, dev)
                })
                .collect();
            let density = |a: f64| -> f64 {
                cands.iter().map(|c| c.0.len * (-0.5 * ((c.2 - a) / 0.35).powi(2)).exp()).sum()
            };
            let peak = (-300..=300)
                .map(|i| i as f64 * 0.05)
                .max_by(|a, b| density(*a).total_cmp(&density(*b)))?;
            let inl: Vec<(Line, Kind)> =
                cands.iter().filter(|c| (c.2 - peak).abs() < 1.0).map(|c| (c.0, c.1)).collect();
            let support: f64 = inl.iter().map(|l| l.0.len).sum();
            if inl.len() < 4 || support < 0.5 {
                return None;
            }
            run(inl, [false, false, true], UprightMode::Level)
        }
        UprightMode::Vertical => reliable_v
            .then(|| run(v_lines.clone(), [true, false, true], UprightMode::Vertical))
            .flatten(),
        UprightMode::Full => {
            if reliable_v && reliable_h {
                run(both(), [true, true, true], UprightMode::Full)
            } else if reliable_v {
                run(v_lines.clone(), [true, false, true], UprightMode::Vertical)
            } else {
                None
            }
        }
        UprightMode::Auto => {
            // Equilibrado: completo só com horizontais fiáveis e correção moderada; senão
            // vertical; sem verticais, nível.
            let vertical = reliable_v
                .then(|| run(v_lines.clone(), [true, false, true], UprightMode::Vertical))
                .flatten();
            let full = (reliable_v && reliable_h)
                .then(|| run(both(), [true, true, true], UprightMode::Full))
                .flatten();
            match (full, vertical) {
                (Some(fr), _)
                    if fr.horizontal.abs() <= 60.0
                        && fr.vertical.abs() <= 80.0
                        && fr.rms_after < 1.0 =>
                {
                    Some(fr)
                }
                (_, Some(vr)) if vr.vertical.abs() <= 80.0 => Some(vr),
                _ => solve_upright(segments, w, h, Some(focal35), UprightMode::Level)
                    .map(|r| UprightResult { mode: UprightMode::Level, ..r }),
            }
        }
        UprightMode::Guided => {
            let free = [v_lines.len() >= 2, h_lines.len() >= 2, true];
            let mut set = both();
            if set.is_empty() {
                set = vert
                    .iter()
                    .map(|l| (*l, Kind::Vertical))
                    .chain(horz.iter().map(|l| (*l, Kind::Horizontal)))
                    .collect();
            }
            run(set, free, UprightMode::Guided)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fachada sintética (janelas e arestas) vista de frente, `w`×`h`.
    fn facade(u: f32, v: f32) -> f32 {
        let (x, y) = (u * 12.0, v * 8.0);
        let (fx, fy) = (x.fract(), y.fract());
        let window = (0.25..0.75).contains(&fx) && (0.2..0.8).contains(&fy);
        let edge = !(0.04..0.96).contains(&u) || !(0.04..0.96).contains(&v);
        if edge {
            0.1
        } else if window {
            0.25
        } else {
            0.8
        }
    }

    /// "Fotografia" da fachada tirada com a câmara rodada pelos parâmetros dados: cada píxel
    /// da fotografia vem do ponto da fachada que a correção com esses parâmetros lhe atribui.
    fn photograph(w: usize, h: usize, g: &Geometry) -> Vec<f32> {
        let (cw, ch) = (w as f32 / h as f32, 1.0);
        let m = PerspectiveMap::new(g, cw, ch);
        let mut out = vec![0.5f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let p = ((x as f32 + 0.5) / w as f32 - 0.5) * cw;
                let q = (y as f32 + 0.5) / h as f32 - 0.5;
                if let Some((a, b)) = m.forward(p, q) {
                    // Fachada a ocupar ~1,6× o enquadramento corrigido.
                    let (u, v) = (a / (cw * 1.6) + 0.5, b / 1.6 + 0.5);
                    if (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v) {
                        out[y * w + x] = facade(u, v);
                    }
                }
            }
        }
        out
    }

    #[test]
    fn detects_the_lines_of_a_facade() {
        let (w, h) = (600, 400);
        let img = photograph(w, h, &Geometry::default());
        let segs = detect_segments(&img, w, h);
        let vertical = segs.iter().filter(|s| (s.x2 - s.x1).abs() < 0.5 && s.len() > 20.0).count();
        let horizontal =
            segs.iter().filter(|s| (s.y2 - s.y1).abs() < 0.5 && s.len() > 20.0).count();
        assert!(
            vertical >= 10 && horizontal >= 10,
            "{vertical} verticais, {horizontal} horizontais de {}",
            segs.len()
        );
    }

    #[test]
    fn recovers_a_known_camera_rotation() {
        let (w, h) = (900, 600);
        for (v, hz, st, focal) in
            [(30.0, 0.0, 2.0, 24.0), (-22.0, 35.0, -3.5, 35.0), (45.0, -20.0, 1.0, 28.0)]
        {
            let truth = Geometry {
                vertical: v,
                horizontal: hz,
                straighten: st,
                focal35: focal,
                ..Default::default()
            };
            let img = photograph(w, h, &truth);
            let segs = detect_segments(&img, w, h);
            let r = solve_upright(&segs, w, h, Some(focal), UprightMode::Full).expect("upright");
            eprintln!("verdade ({v}, {hz}, {st}) → {r:?}");
            assert_eq!(r.mode, UprightMode::Full);
            assert!((r.vertical - v).abs() < 1.5, "vertical {} ≠ {v}", r.vertical);
            assert!((r.horizontal - hz).abs() < 1.5, "horizontal {} ≠ {hz}", r.horizontal);
            assert!((r.straighten - st).abs() < 0.3, "endireitar {} ≠ {st}", r.straighten);
            // Resíduo = precisão angular dos segmentos numa imagem sem antialiasing (~0,3°).
            assert!(r.rms_after < 0.5 && r.rms_after < r.rms_before * 0.15, "{r:?}");
            // Vertical: só as verticais (horizontal fica a zero) e corrige na mesma a inclinação.
            let rv = solve_upright(&segs, w, h, Some(focal), UprightMode::Vertical).unwrap();
            assert_eq!(rv.horizontal, 0.0);
            assert!(rv.rms_after < 0.5, "{rv:?}");
        }
    }

    #[test]
    fn level_only_rotates() {
        let (w, h) = (900, 600);
        let img = photograph(w, h, &Geometry { straighten: -4.0, ..Default::default() });
        let segs = detect_segments(&img, w, h);
        let r = solve_upright(&segs, w, h, None, UprightMode::Level).unwrap();
        assert_eq!((r.vertical, r.horizontal), (0.0, 0.0));
        assert!((r.straighten + 4.0).abs() < 0.2, "{r:?}");
    }

    #[test]
    fn guided_uses_only_the_drawn_lines() {
        let (w, h) = (900, 600);
        let truth =
            Geometry { vertical: 25.0, horizontal: -15.0, straighten: 2.0, ..Default::default() };
        // Guias: duas verticais e duas horizontais da fachada, projetadas na fotografia.
        let m = PerspectiveMap::new(&truth, 1.5, 1.0);
        let to_px = |a: f32, b: f32| {
            let (x, y) = m.backward(a, b);
            ((x / 1.5 + 0.5) * w as f32, (y + 0.5) * h as f32)
        };
        let seg = |a: (f32, f32), b: (f32, f32)| {
            let (p, q) = (to_px(a.0, a.1), to_px(b.0, b.1));
            Segment { x1: p.0, y1: p.1, x2: q.0, y2: q.1 }
        };
        let guides = [
            seg((-0.5, -0.3), (-0.5, 0.3)),
            seg((0.45, -0.3), (0.45, 0.3)),
            seg((-0.4, -0.35), (0.4, -0.35)),
            seg((-0.4, 0.3), (0.4, 0.3)),
        ];
        let r = solve_upright(&guides, w, h, None, UprightMode::Guided).unwrap();
        assert!(
            (r.vertical - 25.0).abs() < 0.5
                && (r.horizontal + 15.0).abs() < 0.5
                && (r.straighten - 2.0).abs() < 0.2,
            "{r:?}"
        );
    }

    #[test]
    fn no_lines_no_correction() {
        let (w, h) = (300, 200);
        let img: Vec<f32> = (0..w * h).map(|i| ((i * 7919) % 97) as f32 / 400.0 + 0.3).collect();
        let segs = detect_segments(&img, w, h);
        assert!(solve_upright(&segs, w, h, None, UprightMode::Vertical).is_none());
    }
}
