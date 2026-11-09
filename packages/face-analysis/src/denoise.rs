//! **AI Denoise** local: FFDNet (Zhang, Zuo & Zhang, IEEE TIP 2018), pesos oficiais
//! `ffdnet_color_clip` do projeto KAIR (MIT). Rede de 12 convoluções (96 canais) a meia resolução com um
//! **mapa do nível de ruído** como entrada — o nível medido na fotografia (e ajustado pela
//! intensidade) controla quanto se suaviza. Treinada com ruído gaussiano em sRGB (não com
//! RAW reais): é uma redução de ruído de qualidade superior ao filtro rápido, não uma
//! reconstrução a partir do sensor.
//!
//! Executada por mosaicos de tamanho fixo (memória constante) com sobreposição, em paralelo.

use momento_core_types::{CoreError, CoreResult};
use rayon::prelude::*;
use std::path::Path;
use tract_onnx::prelude::*;

pub const DENOISE_MODEL_FILE: &str = "denoise_ffdnet.onnx";

/// Lado do mosaico em píxeis da imagem (o modelo trabalha a metade).
const TILE: usize = 256;
/// Sobreposição de cada lado (o campo recetivo da rede é ≈ 26 px).
const OVERLAP: usize = 24;

/// Núcleo de um mosaico: origem (x, y), largura, altura e píxeis.
type TileCore = (usize, usize, usize, usize, Vec<[f32; 3]>);

pub struct Denoiser {
    plan: crate::Plan,
}

impl Denoiser {
    /// Carrega `denoise_ffdnet.onnx` da pasta dos modelos (`None` se não existir).
    pub fn load(dir: &Path) -> CoreResult<Option<Self>> {
        let path = dir.join(DENOISE_MODEL_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let err = |e: TractError| CoreError::Other(format!("modelo {}: {e:#}", path.display()));
        let h = TILE / 2;
        let plan = tract_onnx::onnx()
            .model_for_path(&path)
            .map_err(err)?
            .with_input_fact(0, f32::fact([1, 12, h, h]).into())
            .map_err(err)?
            .with_input_fact(1, f32::fact([1, 1, h, h]).into())
            .map_err(err)?
            .into_optimized()
            .map_err(err)?
            .into_runnable()
            .map_err(err)?;
        Ok(Some(Denoiser { plan }))
    }

    /// Reduz o ruído de uma imagem RGB **codificada para visualização** (valores 0–1, curva
    /// sRGB), no lugar. `sigma` é o desvio-padrão do ruído na mesma escala (ex.: 0,04 ≈ 10/255).
    /// `progress(feitos, total)` é chamado por mosaico; devolve `false` para cancelar.
    pub fn denoise(
        &self,
        px: &mut [[f32; 3]],
        w: usize,
        h: usize,
        sigma: f32,
        progress: &(dyn Fn(usize, usize) -> bool + Sync),
    ) -> CoreResult<bool> {
        let step = TILE - 2 * OVERLAP;
        let (nx, ny) = (w.div_ceil(step), h.div_ceil(step));
        let total = nx * ny;
        let done = std::sync::atomic::AtomicUsize::new(0);
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let src: &[[f32; 3]] = px;
        let sigma = sigma.clamp(0.0, 75.0 / 255.0);
        // Cada mosaico produz o seu núcleo (sem a sobreposição) — sem costuras.
        let tiles: Vec<CoreResult<Option<TileCore>>> = (0..total)
            .into_par_iter()
            .map(|t| {
                if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    return Ok(None);
                }
                let (tx, ty) = (t % nx, t / nx);
                let (x0, y0) = (tx * step, ty * step);
                let (cw, ch) = (step.min(w - x0), step.min(h - y0));
                let out = self.run_tile(
                    src,
                    w,
                    h,
                    x0 as isize - OVERLAP as isize,
                    y0 as isize - OVERLAP as isize,
                    sigma,
                )?;
                let mut core = Vec::with_capacity(cw * ch);
                for y in 0..ch {
                    for x in 0..cw {
                        core.push(out[(y + OVERLAP) * TILE + x + OVERLAP]);
                    }
                }
                let d = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if !progress(d, total) {
                    cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                Ok(Some((x0, y0, cw, ch, core)))
            })
            .collect();
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(false);
        }
        for t in tiles {
            let Some((x0, y0, cw, ch, core)) = t? else { return Ok(false) };
            for y in 0..ch {
                px[(y0 + y) * w + x0..(y0 + y) * w + x0 + cw]
                    .copy_from_slice(&core[y * cw..(y + 1) * cw]);
            }
        }
        Ok(true)
    }

    /// Um mosaico `TILE`×`TILE` com origem (x0, y0) (pode sair da imagem: bordas espelhadas).
    fn run_tile(
        &self,
        src: &[[f32; 3]],
        w: usize,
        h: usize,
        x0: isize,
        y0: isize,
        sigma: f32,
    ) -> CoreResult<Vec<[f32; 3]>> {
        let half = TILE / 2;
        let reflect = |v: isize, n: usize| -> usize {
            let n = n as isize;
            let mut v = v;
            if n == 1 {
                return 0;
            }
            loop {
                if v < 0 {
                    v = -v;
                } else if v >= n {
                    v = 2 * n - 2 - v;
                } else {
                    return v as usize;
                }
            }
        };
        // Pixel unshuffle (ordem do KAIR): canal = c·4 + i·2 + j.
        let mut x = vec![0f32; 12 * half * half];
        for yy in 0..TILE {
            let sy = reflect(y0 + yy as isize, h);
            for xx in 0..TILE {
                let sx = reflect(x0 + xx as isize, w);
                let p = src[sy * w + sx];
                let (i, j) = (yy % 2, xx % 2);
                for (c, v) in p.iter().enumerate() {
                    x[((c * 4 + i * 2 + j) * half + yy / 2) * half + xx / 2] = v.clamp(0.0, 1.0);
                }
            }
        }
        let err = |e: TractError| CoreError::Other(format!("AI Denoise: {e:#}"));
        let xt = Tensor::from_shape(&[1, 12, half, half], &x).map_err(err)?;
        let st = Tensor::from_shape(&[1, 1, half, half], &vec![sigma; half * half]).map_err(err)?;
        let out = self.plan.run(tvec!(xt.into(), st.into())).map_err(err)?;
        let y = out[0].to_plain_array_view::<f32>().map_err(err)?;
        let y = y.as_slice().ok_or_else(|| CoreError::Other("AI Denoise: saída".into()))?;
        let mut res = vec![[0f32; 3]; TILE * TILE];
        for yy in 0..TILE {
            for xx in 0..TILE {
                let (i, j) = (yy % 2, xx % 2);
                for c in 0..3 {
                    res[yy * TILE + xx][c] =
                        y[((c * 4 + i * 2 + j) * half + yy / 2) * half + xx / 2];
                }
            }
        }
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Imagem limpa com bordas e degradês + ruído gaussiano σ = 25/255: o AI Denoise tem de
    /// subir muito o PSNR e preservar as arestas. Requer o modelo (`prepare-models.py`).
    #[test]
    fn ffdnet_removes_gaussian_noise() {
        let Some(dir) = crate::find_models_dir(&[]) else {
            eprintln!("modelos em falta: teste ignorado");
            return;
        };
        let Some(d) = Denoiser::load(&dir).unwrap() else {
            eprintln!("{DENOISE_MODEL_FILE} em falta: teste ignorado");
            return;
        };
        let (w, h) = (600, 400);
        let clean: Vec<[f32; 3]> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f32, (i / w) as f32);
                let edge =
                    if ((x / 60.0) as i32 + (y / 60.0) as i32) % 2 == 0 { 0.25 } else { 0.0 };
                [0.2 + x / w as f32 * 0.5 + edge, 0.3 + y / h as f32 * 0.4, 0.5 + edge]
            })
            .collect();
        // Ruído pseudo-aleatório determinístico (Box–Muller).
        let mut s = 12345u64;
        let mut rnd = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f32 / (1u64 << 53) as f32
        };
        let sigma = 25.0 / 255.0;
        let mut noisy = clean.clone();
        for p in &mut noisy {
            for c in p.iter_mut() {
                let (u1, u2) = (rnd().max(1e-7), rnd());
                *c += sigma * (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos();
            }
        }
        let psnr = |a: &[[f32; 3]]| {
            let mse: f32 = a
                .iter()
                .zip(&clean)
                .map(|(p, q)| (0..3).map(|c| (p[c].clamp(0.0, 1.0) - q[c]).powi(2)).sum::<f32>())
                .sum::<f32>()
                / (a.len() * 3) as f32;
            10.0 * (1.0 / mse).log10()
        };
        let before = psnr(&noisy);
        let mut out = noisy.clone();
        let t = std::time::Instant::now();
        assert!(d.denoise(&mut out, w, h, sigma, &|_, _| true).unwrap());
        let mpx = (w * h) as f32 / 1e6;
        eprintln!(
            "AI Denoise {w}×{h}: {:.1?} ({:.2} s/MP); PSNR {before:.1} → {:.1} dB",
            t.elapsed(),
            t.elapsed().as_secs_f32() / mpx,
            psnr(&out)
        );
        assert!(psnr(&out) > before + 8.0);
        // Cancelar devolve false e não altera a imagem.
        let mut c = noisy.clone();
        assert!(!d.denoise(&mut c, w, h, sigma, &|_, _| false).unwrap());
    }
}
