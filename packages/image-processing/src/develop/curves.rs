//! Curvas no domínio de visualização (0–1): pretos/brancos, contraste, curva paramétrica e
//! curvas de pontos (RGB e por canal), compostas numa única tabela por canal.

use super::settings::{Curve, Light};

/// Interpolação cúbica monótona (Fritsch–Carlson): nunca ultrapassa os pontos.
pub struct Monotone {
    x: Vec<f32>,
    y: Vec<f32>,
    m: Vec<f32>,
}

impl Monotone {
    /// `None` se a curva for a identidade (vazia ou só (0,0)–(1,1)).
    pub fn new(points: &[[f32; 2]]) -> Option<Self> {
        let mut p: Vec<[f32; 2]> =
            points.iter().map(|q| [q[0].clamp(0.0, 1.0), q[1].clamp(0.0, 1.0)]).collect();
        p.sort_by(|a, b| a[0].total_cmp(&b[0]));
        p.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-4);
        if p.len() < 2 || p.iter().all(|q| (q[0] - q[1]).abs() < 1e-4) {
            return None;
        }
        let (x, y): (Vec<f32>, Vec<f32>) = p.iter().map(|q| (q[0], q[1])).unzip();
        let n = x.len();
        let d: Vec<f32> =
            (0..n - 1).map(|i| (y[i + 1] - y[i]) / (x[i + 1] - x[i]).max(1e-6)).collect();
        let mut m = vec![0.0; n];
        m[0] = d[0];
        m[n - 1] = d[n - 2];
        for i in 1..n - 1 {
            m[i] = if d[i - 1] * d[i] <= 0.0 { 0.0 } else { (d[i - 1] + d[i]) / 2.0 };
        }
        for i in 0..n - 1 {
            if d[i] == 0.0 {
                m[i] = 0.0;
                m[i + 1] = 0.0;
            } else {
                let (a, b) = (m[i] / d[i], m[i + 1] / d[i]);
                let s = a * a + b * b;
                if s > 9.0 {
                    let t = 3.0 / s.sqrt();
                    m[i] = t * a * d[i];
                    m[i + 1] = t * b * d[i];
                }
            }
        }
        Some(Monotone { x, y, m })
    }

    pub fn eval(&self, v: f32) -> f32 {
        let n = self.x.len();
        if v <= self.x[0] {
            return self.y[0];
        }
        if v >= self.x[n - 1] {
            return self.y[n - 1];
        }
        let i = self.x.partition_point(|&q| q <= v).saturating_sub(1).min(n - 2);
        let h = self.x[i + 1] - self.x[i];
        let t = (v - self.x[i]) / h;
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * self.y[i]
            + (t3 - 2.0 * t2 + t) * h * self.m[i]
            + (-2.0 * t3 + 3.0 * t2) * self.y[i + 1]
            + (t3 - t2) * h * self.m[i + 1]
    }
}

pub(crate) fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// “Lomba” suave entre a e b (0 fora, 1 no centro).
fn bump(a: f32, b: f32, x: f32) -> f32 {
    if x <= a || x >= b {
        return 0.0;
    }
    let t = (x - a) / (b - a);
    (std::f32::consts::PI * t).sin().powi(2)
}

/// Curva principal (sem as curvas por canal) aplicada a um valor de visualização.
pub fn master(light: &Light, c: &Curve, points: Option<&Monotone>, d: f32) -> f32 {
    let mut v = d;
    // Pretos e brancos: deslocam as pontas com transição suave.
    v += light.blacks / 100.0 * 0.12 * (1.0 - smoothstep(0.0, 0.5, v));
    v += light.whites / 100.0 * 0.12 * smoothstep(0.5, 1.0, v);
    v = v.clamp(0.0, 1.0);
    // Contraste: curva S com as pontas fixas (identidade em 0).
    let k = (light.contrast / 100.0).clamp(-1.0, 1.0);
    if k != 0.0 {
        let s = v * v * (3.0 - 2.0 * v);
        v = if k > 0.0 { v + k * 0.7 * (s - v) } else { v + k * 0.7 * (s - v) * 1.0 };
    }
    // Curva paramétrica: quatro zonas delimitadas pelas divisões.
    let (s1, s2, s3) =
        (c.split_shadows / 100.0, c.split_midtones / 100.0, c.split_highlights / 100.0);
    let amp = 0.18;
    v += c.shadows / 100.0 * amp * bump(0.0, s2.max(s1 + 0.01), v) * if v < s1 { 1.0 } else { 0.7 };
    v += c.darks / 100.0 * amp * bump(s1 * 0.5, s2 + (s3 - s2) * 0.5, v);
    v += c.lights / 100.0 * amp * bump(s2 - (s2 - s1) * 0.5, (s3 + 1.0) * 0.5, v);
    v += c.highlights / 100.0
        * amp
        * bump(s2.min(s3 - 0.01), 1.0, v)
        * if v > s3 { 1.0 } else { 0.7 };
    v = v.clamp(0.0, 1.0);
    match points {
        Some(m) => m.eval(v),
        None => v,
    }
}

/// Tabelas por canal (R, G, B) que já incluem a curva principal.
pub struct CurveLuts {
    pub rgb: [Vec<f32>; 3],
    pub identity: bool,
}

pub const CURVE_N: usize = 2048;

impl CurveLuts {
    pub fn new(light: &Light, c: &Curve) -> Self {
        let pts = Monotone::new(&c.rgb);
        let per = [Monotone::new(&c.red), Monotone::new(&c.green), Monotone::new(&c.blue)];
        let identity = light.contrast == 0.0
            && light.whites == 0.0
            && light.blacks == 0.0
            && c.highlights == 0.0
            && c.lights == 0.0
            && c.darks == 0.0
            && c.shadows == 0.0
            && pts.is_none()
            && per.iter().all(Option::is_none);
        let base: Vec<f32> = (0..=CURVE_N)
            .map(|i| master(light, c, pts.as_ref(), i as f32 / CURVE_N as f32))
            .collect();
        let rgb = std::array::from_fn(|ch| match &per[ch] {
            Some(m) => base.iter().map(|&v| m.eval(v)).collect(),
            None => base.clone(),
        });
        CurveLuts { rgb, identity }
    }

    #[inline]
    pub fn apply(&self, ch: usize, v: f32) -> f32 {
        let t = &self.rgb[ch];
        let p = v.clamp(0.0, 1.0) * CURVE_N as f32;
        let i = (p as usize).min(CURVE_N - 1);
        t[i] + (t[i + 1] - t[i]) * (p - i as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotone_curve_passes_points_without_overshoot() {
        let m =
            Monotone::new(&[[0.0, 0.0], [0.25, 0.1], [0.5, 0.5], [0.75, 0.9], [1.0, 1.0]]).unwrap();
        assert!((m.eval(0.25) - 0.1).abs() < 1e-5 && (m.eval(0.75) - 0.9).abs() < 1e-5);
        let mut last = 0.0;
        for i in 0..=100 {
            let v = m.eval(i as f32 / 100.0);
            assert!(v >= last - 1e-6 && (0.0..=1.0).contains(&v));
            last = v;
        }
        assert!(Monotone::new(&[[0.0, 0.0], [1.0, 1.0]]).is_none(), "identidade");
    }

    #[test]
    fn neutral_settings_are_identity_and_contrast_is_an_s_curve() {
        let l = Light::default();
        let c = Curve::default();
        let luts = CurveLuts::new(&l, &c);
        assert!(luts.identity);
        assert!((luts.apply(0, 0.37) - 0.37).abs() < 1e-4);
        let hi = Light { contrast: 60.0, ..l };
        let s = CurveLuts::new(&hi, &c);
        assert!(
            s.apply(0, 0.25) < 0.25
                && s.apply(0, 0.75) > 0.75
                && (s.apply(0, 0.5) - 0.5).abs() < 1e-3
        );
        let inv = Curve { red: vec![[0.0, 1.0], [1.0, 0.0]], ..c };
        let r = CurveLuts::new(&l, &inv);
        assert!(
            (r.apply(0, 0.2) - 0.8).abs() < 1e-3 && (r.apply(1, 0.2) - 0.2).abs() < 1e-3,
            "só o vermelho"
        );
    }
}
