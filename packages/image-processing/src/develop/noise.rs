//! Modelo de ruído do sensor estimado na própria fotografia.
//!
//! O ruído de um sensor é dependente do sinal (fotões: Poisson; leitura: gaussiano): a
//! variância em linear é `a·x + b` por canal (Foi et al., "Practical Poissonian-Gaussian noise
//! modeling", IEEE TIP 2008). Estima-se em blocos 8×8 pouco texturados: para cada bloco a média
//! e a variância do resíduo de alta frequência (pixel − média dos 4 vizinhos); por classe de
//! intensidade guarda-se um quantil baixo (os blocos lisos) e ajusta-se a reta por mínimos
//! quadrados ponderados.
//!
//! Depois de passar para o domínio de visualização (sRGB), o ruído deixa de ser uniforme: é
//! muito mais forte nas sombras. `display_sigma` dá o desvio padrão esperado em cada píxel,
//! usado como mapa do nível de ruído do AI Denoise (em vez de um valor único).

use super::source::Plane;

/// Fator sobre o desvio padrão estimado para o mapa do AI Denoise (FFDNet rende mais com
/// um nível ligeiramente acima do real; calibrado em `tests/denoise_quality.rs`).
pub const AI_MAP_FACTOR: f32 = 1.25;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseModel {
    /// Variância = a·x + b (x linear 0–1), por canal.
    pub a: [f32; 3],
    pub b: [f32; 3],
}

impl NoiseModel {
    /// Desvio padrão em linear do canal `c` no nível `x`.
    #[inline]
    pub fn sigma(&self, c: usize, x: f32) -> f32 {
        (self.a[c] * x.max(0.0) + self.b[c]).max(0.0).sqrt()
    }
}

/// Fator de correção do quantil baixo (20 %) da variância amostral com 64 amostras
/// (χ²(63)/63 → ≈ 0,86 no quantil 20 %).
const LOW_QUANTILE: f32 = 0.20;
const QUANTILE_BIAS: f32 = 0.86;

/// Estima o modelo de ruído (variância `a·x + b`) de um plano linear. `None` se a imagem for
/// pequena demais ou não tiver zonas lisas suficientes.
pub fn estimate(p: &Plane) -> Option<NoiseModel> {
    estimate_at(p, 1)
}

/// Como `estimate`, com os vizinhos a `d` píxeis: com `d = 2` mede também o ruído
/// espacialmente correlacionado (grão de 2–3 píxeis que a desmatrização cria nos RAW reais),
/// que a diferença entre vizinhos imediatos subestima.
pub fn estimate_at(p: &Plane, d: usize) -> Option<NoiseModel> {
    const B: usize = 8;
    let d = d.clamp(1, 3);
    if p.w < 4 * B || p.h < 4 * B {
        return None;
    }
    // Classes de intensidade (escala logarítmica).
    const BINS: usize = 20;
    let bin_of = |m: f32| -> Option<usize> {
        if m <= 1e-4 || m >= 0.95 {
            return None;
        }
        Some((((m.ln() - 1e-4f32.ln()) / (0.95f32.ln() - 1e-4f32.ln())) * BINS as f32) as usize)
            .map(|b| b.min(BINS - 1))
    };
    let mut model = NoiseModel { a: [0.0; 3], b: [0.0; 3] };
    // Passo entre blocos para imagens grandes (no máximo ~40 000 blocos).
    let stride = (((p.w / B) * (p.h / B)) as f32 / 40_000.0).sqrt().ceil().max(1.0) as usize;
    let mut ok = false;
    for c in 0..3 {
        let mut bins: Vec<Vec<(f32, f32)>> = vec![Vec::new(); BINS];
        for by in (1..p.h / B - 1).step_by(stride) {
            for bx in (1..p.w / B - 1).step_by(stride) {
                if (bx + 1) * B + d > p.w || (by + 1) * B + d > p.h || bx * B < d || by * B < d {
                    continue;
                }
                let (x0, y0) = (bx * B, by * B);
                let (mut s, mut sr, mut sr2) = (0f32, 0f32, 0f32);
                for y in y0..y0 + B {
                    for x in x0..x0 + B {
                        let v = p.at(x, y)[c];
                        let nb = 0.25
                            * (p.at(x - d, y)[c]
                                + p.at(x + d, y)[c]
                                + p.at(x, y - d)[c]
                                + p.at(x, y + d)[c]);
                        let r = v - nb;
                        s += v;
                        sr += r;
                        sr2 += r * r;
                    }
                }
                let n = (B * B) as f32;
                let mean = s / n;
                // Var(x − média de 4 vizinhos) = σ²·(1 + 1/4) para ruído branco.
                let var = ((sr2 - sr * sr / n) / (n - 1.0)) / 1.25;
                if let Some(b) = bin_of(mean) {
                    bins[b].push((mean, var));
                }
            }
        }
        // Por classe: quantil baixo da variância (blocos lisos) e a média correspondente.
        let mut pts: Vec<(f32, f32, f32)> = Vec::new(); // (média, variância, peso)
        for b in bins.iter_mut().filter(|b| b.len() >= 12) {
            b.sort_by(|x, y| x.1.total_cmp(&y.1));
            let k = ((b.len() as f32 * LOW_QUANTILE) as usize).min(b.len() - 1);
            let lo = &b[..=k];
            let mean = lo.iter().map(|v| v.0).sum::<f32>() / lo.len() as f32;
            pts.push((mean, b[k].1 / QUANTILE_BIAS, (b.len() as f32).sqrt()));
        }
        if pts.len() < 3 {
            continue;
        }
        let (a, bb) = fit_line(&pts);
        model.a[c] = a;
        model.b[c] = bb;
        ok = true;
    }
    ok.then_some(model)
}

/// Reta var = a·m + b por mínimos quadrados ponderados (pesos relativos à variância, para
/// as sombras contarem tanto como os realces), com a, b ≥ 0.
fn fit_line(pts: &[(f32, f32, f32)]) -> (f32, f32) {
    let solve = |use_b: bool| -> (f32, f32) {
        let (mut sww, mut swm, mut swmm, mut swv, mut swmv) = (0f64, 0f64, 0f64, 0f64, 0f64);
        for &(m, v, w) in pts {
            let w = w as f64 / (v as f64).max(1e-12).powi(2).sqrt().max(1e-9);
            let (m, v) = (m as f64, v as f64);
            sww += w;
            swm += w * m;
            swmm += w * m * m;
            swv += w * v;
            swmv += w * m * v;
        }
        if use_b {
            let det = sww * swmm - swm * swm;
            if det.abs() < 1e-30 {
                return (0.0, (swv / sww.max(1e-30)) as f32);
            }
            let a = (sww * swmv - swm * swv) / det;
            let b = (swmm * swv - swm * swmv) / det;
            (a as f32, b as f32)
        } else {
            ((swmv / swmm.max(1e-30)) as f32, 0.0)
        }
    };
    let (a, b) = solve(true);
    if a >= 0.0 && b >= 0.0 {
        return (a, b);
    }
    if b < 0.0 {
        let (a, _) = solve(false);
        return (a.max(0.0), 0.0);
    }
    // a < 0: ruído constante.
    let mean_v = pts.iter().map(|p| p.1).sum::<f32>() / pts.len() as f32;
    (0.0, mean_v.max(0.0))
}

/// Derivada da curva sRGB (linear → visualização).
#[inline]
fn oetf_slope(x: f32) -> f32 {
    if x <= 0.003_130_8 {
        12.92
    } else {
        1.055 / 2.4 * x.powf(1.0 / 2.4 - 1.0)
    }
}

/// Desvio padrão do ruído no domínio de visualização (sRGB 0–1) em cada píxel de `p`, a
/// partir de uma versão suavizada (o nível médio, não o próprio ruído). Média dos canais.
pub fn display_sigma(p: &Plane, model: &NoiseModel) -> Vec<f32> {
    let smooth = box_blur(p, 2);
    smooth
        .iter()
        .map(|q| {
            (0..3)
                .map(|c| {
                    let x = q[c].clamp(0.0, 1.0);
                    model.sigma(c, x) * oetf_slope(x.max(1e-4)).min(12.92)
                })
                .sum::<f32>()
                / 3.0
        })
        .collect()
}

fn box_blur(p: &Plane, r: usize) -> Vec<[f32; 3]> {
    let (w, h) = (p.w, p.h);
    let mut tmp = vec![[0f32; 3]; w * h];
    for y in 0..h {
        for x in 0..w {
            let (x0, x1) = (x.saturating_sub(r), (x + r).min(w - 1));
            let mut s = [0f32; 3];
            for xx in x0..=x1 {
                let q = p.px[y * w + xx];
                for c in 0..3 {
                    s[c] += q[c];
                }
            }
            let n = (x1 - x0 + 1) as f32;
            tmp[y * w + x] = s.map(|v| v / n);
        }
    }
    let mut out = vec![[0f32; 3]; w * h];
    for y in 0..h {
        let (y0, y1) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            let mut s = [0f32; 3];
            for yy in y0..=y1 {
                let q = tmp[yy * w + x];
                for c in 0..3 {
                    s[c] += q[c];
                }
            }
            let n = (y1 - y0 + 1) as f32;
            out[y * w + x] = s.map(|v| v / n);
        }
    }
    out
}

/// Ruído de Poisson–Gauss simulado (determinístico): variância `a·x + b` por canal.
pub fn add_noise(p: &Plane, a: f32, b: f32, seed: u64) -> Plane {
    let mut s = seed | 1;
    let mut gauss = move || {
        // Box–Muller com xorshift.
        let mut u = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 11) as f64 / (1u64 << 53) as f64).max(1e-12)
        };
        let (u1, u2) = (u(), u());
        ((-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()) as f32
    };
    Plane {
        w: p.w,
        h: p.h,
        px: p.px.iter().map(|q| q.map(|x| x + (a * x.max(0.0) + b).sqrt() * gauss())).collect(),
    }
}

/// Plano do AI Denoise para uma imagem linear: mapa do nível de ruído (sRGB 0–1, por píxel)
/// e se o ruído é espacialmente correlacionado (grão da desmatrização) — nesse caso a rede é
/// aplicada às sub-imagens intercaladas e o nível é medido a 2 píxeis.
#[derive(Debug, Clone)]
pub struct DenoisePlan {
    pub map: Vec<f32>,
    pub shuffled: bool,
    pub model: NoiseModel,
}

/// `strength` 0–100 (50 = o nível medido).
pub fn denoise_plan(p: &Plane, strength: f32) -> Option<DenoisePlan> {
    let near = estimate_at(p, 1)?;
    let far = estimate_at(p, 2).unwrap_or(near);
    // Correlação: o ruído medido a 2 píxeis é claramente maior que o medido entre vizinhos.
    let mid = |m: &NoiseModel| (0..3).map(|c| m.sigma(c, 0.18)).sum::<f32>();
    let shuffled = mid(&far) > 1.25 * mid(&near);
    let model = if shuffled { far } else { near };
    let k = AI_MAP_FACTOR * strength.clamp(1.0, 100.0) / 50.0;
    let map = display_sigma(p, &model)
        .into_iter()
        .map(|v| (v * k).clamp(0.5 / 255.0, 75.0 / 255.0))
        .collect();
    Some(DenoisePlan { map, shuffled, model })
}

/// Ruído correlacionado como o de um RAW desmatrizado: ruído branco filtrado por um núcleo
/// 3×3 [1 2 1]ᵀ[1 2 1] (grão de ~2 píxeis), com a mesma variância `a·x + b` por píxel.
pub fn add_correlated_noise(p: &Plane, a: f32, b: f32, seed: u64) -> Plane {
    let white = add_noise(&Plane { w: p.w, h: p.h, px: vec![[0.0; 3]; p.w * p.h] }, 0.0, 1.0, seed);
    let k = [1.0f32, 2.0, 1.0];
    // Norma do núcleo: Σk² = (1+4+1)² / 16² → para manter a variância 1, divide por √(36)/16.
    let norm = 16.0 / 6.0;
    let at = |x: isize, y: isize| -> [f32; 3] {
        let x = x.clamp(0, p.w as isize - 1) as usize;
        let y = y.clamp(0, p.h as isize - 1) as usize;
        white.px[y * p.w + x]
    };
    Plane {
        w: p.w,
        h: p.h,
        px: (0..p.w * p.h)
            .map(|i| {
                let (x, y) = ((i % p.w) as isize, (i / p.w) as isize);
                let mut n = [0f32; 3];
                for (j, kj) in k.iter().enumerate() {
                    for (i2, ki) in k.iter().enumerate() {
                        let q = at(x + i2 as isize - 1, y + j as isize - 1);
                        for c in 0..3 {
                            n[c] += q[c] * ki * kj / 16.0 * norm;
                        }
                    }
                }
                let v = p.px[i];
                std::array::from_fn(|c| v[c] + (a * v[c].max(0.0) + b).sqrt() * n[c])
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_a_known_noise_model() {
        // Imagem lisa com gradientes (todas as intensidades) + ruído de Poisson–Gauss conhecido.
        let (w, h) = (900, 600);
        let clean = Plane {
            w,
            h,
            px: (0..w * h)
                .map(|i| {
                    let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
                    let v = 0.002 + 0.85 * x * x;
                    [v, v * (0.6 + 0.4 * y), v * 0.8]
                })
                .collect(),
        };
        for (a, b) in [(1.5e-3f32, 1e-5f32), (3e-4, 2e-6)] {
            let noisy = add_noise(&clean, a, b, 99);
            let m = estimate(&noisy).expect("modelo");
            eprintln!("verdade a={a:e} b={b:e} → {m:?}");
            for c in 0..3 {
                // O desvio padrão é o que conta: ±20 % em sombras (x=0,01) e realces (x=0,5).
                for x in [0.01f32, 0.5] {
                    let truth = (a * x + b).sqrt();
                    let est = m.sigma(c, x);
                    assert!((est / truth - 1.0).abs() < 0.2, "canal {c}, x={x}: {est} vs {truth}");
                }
            }
        }
    }

    #[test]
    fn display_noise_is_stronger_in_the_shadows() {
        let p = Plane {
            w: 64,
            h: 64,
            px: (0..64 * 64).map(|i| [((i % 64) as f32 / 64.0).powi(2) + 0.002; 3]).collect(),
        };
        let m = NoiseModel { a: [1e-3; 3], b: [1e-5; 3] };
        let s = display_sigma(&p, &m);
        assert!(
            s[32 * 64 + 2] > s[32 * 64 + 60] * 2.0,
            "{} vs {}",
            s[32 * 64 + 2],
            s[32 * 64 + 60]
        );
    }
}
