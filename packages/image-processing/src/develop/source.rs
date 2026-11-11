//! Imagem-fonte da edição: RGB linear Rec. 2020 (f32), já orientada, com uma pirâmide de
//! resoluções em memória para renders rápidos durante a interação.

use super::color::{eotf, mul3, SRGB_TO_REC2020};
use image::RgbImage;

#[derive(Debug, Clone)]
pub struct Plane {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[f32; 3]>,
}

impl Plane {
    #[inline]
    pub fn at(&self, x: usize, y: usize) -> [f32; 3] {
        self.px[y * self.w + x]
    }

    /// Amostragem bilinear em coordenadas normalizadas; fora da imagem → `None`.
    #[inline]
    pub fn sample(&self, u: f32, v: f32) -> Option<[f32; 3]> {
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            return None;
        }
        let x = (u * self.w as f32 - 0.5).clamp(0.0, (self.w - 1) as f32);
        let y = (v * self.h as f32 - 0.5).clamp(0.0, (self.h - 1) as f32);
        let (x0, y0) = (x as usize, y as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let (a, b, c, d) = (self.at(x0, y0), self.at(x1, y0), self.at(x0, y1), self.at(x1, y1));
        Some(std::array::from_fn(|i| {
            (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy
        }))
    }

    /// Canal isolado (aberração cromática).
    #[inline]
    pub fn sample_channel(&self, u: f32, v: f32, ch: usize) -> f32 {
        // Fora da imagem (ex.: vermelho deslocado pela aberração cromática junto à borda):
        // repete a borda em vez de preto, para não criar uma linha colorida.
        self.sample(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)).map(|p| p[ch]).unwrap_or(0.0)
    }

    fn half(&self) -> Plane {
        let (w, h) = ((self.w / 2).max(1), (self.h / 2).max(1));
        let mut px = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let (x0, y0) = (x * 2, y * 2);
                let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
                let (a, b, c, d) =
                    (self.at(x0, y0), self.at(x1, y0), self.at(x0, y1), self.at(x1, y1));
                px.push(std::array::from_fn(|i| (a[i] + b[i] + c[i] + d[i]) * 0.25));
            }
        }
        Plane { w, h, px }
    }
}

#[derive(Debug, Clone)]
pub struct Source {
    /// Do maior para o menor; níveis sucessivos com metade da resolução.
    pub levels: Vec<Plane>,
    /// Resolução do original (para converter raios em píxeis “reais”).
    pub full_width: u32,
    pub full_height: u32,
    /// Exposição base aplicada (EV), ex.: RAW lineares escurecidos.
    pub baseline_ev: f32,
    /// Temperatura (K) do balanço da câmara (RAW): referência do balanço de brancos, para a
    /// temperatura poder ser mostrada e aplicada em Kelvin. `None` = imagem já revelada.
    pub as_shot_k: Option<f32>,
}

impl Source {
    /// Temperatura de referência do balanço de brancos desta fonte.
    pub fn wb_reference_k(&self) -> f32 {
        self.as_shot_k.unwrap_or(super::color::WB_REFERENCE_K)
    }

    pub fn from_plane(base: Plane, full_width: u32, full_height: u32, baseline_ev: f32) -> Self {
        let mut levels = vec![base];
        while levels.last().unwrap().w.max(levels.last().unwrap().h) > 320 {
            let next = levels.last().unwrap().half();
            levels.push(next);
        }
        Source { levels, full_width, full_height, baseline_ev, as_shot_k: None }
    }

    /// A partir de uma imagem sRGB 8 bits (JPEG, PNG, previews…), já orientada.
    pub fn from_srgb(img: &RgbImage, full_width: u32, full_height: u32) -> Self {
        let lut: Vec<f32> = (0..256).map(|v| eotf(v as f32 / 255.0)).collect();
        let px = img
            .pixels()
            .map(|p| {
                mul3(&SRGB_TO_REC2020, [lut[p[0] as usize], lut[p[1] as usize], lut[p[2] as usize]])
            })
            .collect();
        Source::from_plane(
            Plane { w: img.width() as usize, h: img.height() as usize, px },
            full_width,
            full_height,
            0.0,
        )
    }

    /// A partir de RGB linear 16 bits em Rec. 2020 (RAW revelado pela LibRaw), sem rotação,
    /// com a orientação EXIF aplicada aqui. Aplica uma exposição base automática (os dados
    /// do sensor sem “brilho automático” ficam escuros), limitada a +2 EV.
    pub fn from_linear16(
        w: u32,
        h: u32,
        rgb: &[u16],
        orientation: u8,
        full_width: u32,
        full_height: u32,
    ) -> Self {
        let (w, h) = (w as usize, h as usize);
        let px: Vec<[f32; 3]> = rgb
            .chunks_exact(3)
            .map(|c| [c[0] as f32 / 65535.0, c[1] as f32 / 65535.0, c[2] as f32 / 65535.0])
            .collect();
        let plane = orient(Plane { w, h, px }, orientation);
        // Exposição base: o percentil 99 da luminância fica em ~0,9.
        let mut lum: Vec<f32> = plane
            .px
            .iter()
            .step_by(7)
            .map(|p| 0.2627 * p[0] + 0.678 * p[1] + 0.0593 * p[2])
            .collect();
        lum.sort_by(f32::total_cmp);
        let p99 = lum.get(lum.len() * 99 / 100).copied().unwrap_or(1.0).max(1e-4);
        let baseline = (0.9 / p99).log2().clamp(0.0, 2.0);
        let g = 2f32.powf(baseline);
        let px = plane.px.iter().map(|p| [p[0] * g, p[1] * g, p[2] * g]).collect();
        let (fw, fh) = if (5..=8).contains(&orientation) {
            (full_height, full_width)
        } else {
            (full_width, full_height)
        };
        Source::from_plane(Plane { w: plane.w, h: plane.h, px }, fw, fh, baseline)
    }

    pub fn width(&self) -> usize {
        self.levels[0].w
    }
    pub fn height(&self) -> usize {
        self.levels[0].h
    }

    /// Nível com pelo menos `needed` píxeis no lado maior (o mais pequeno que chega).
    pub fn level_for(&self, needed: f32) -> &Plane {
        self.levels.iter().rev().find(|l| l.w.max(l.h) as f32 >= needed).unwrap_or(&self.levels[0])
    }
}

/// Aplica a orientação EXIF (1–8) a um plano.
pub fn orient(p: Plane, o: u8) -> Plane {
    if !(2..=8).contains(&o) {
        return p;
    }
    let (w, h) = (p.w, p.h);
    let swap = (5..=8).contains(&o);
    let (nw, nh) = if swap { (h, w) } else { (w, h) };
    let mut px = vec![[0.0; 3]; nw * nh];
    for y in 0..nh {
        for x in 0..nw {
            // Para cada píxel de saída, o píxel de origem (mesmas convenções do EXIF).
            let (sx, sy) = match o {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (y, h - 1 - x),
                7 => (w - 1 - y, h - 1 - x),
                8 => (w - 1 - y, x),
                _ => (x, y),
            };
            px[y * nw + x] = p.px[sy * w + sx];
        }
    }
    Plane { w: nw, h: nh, px }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pyramid_halves_until_small() {
        let img = RgbImage::from_pixel(1300, 900, image::Rgb([128, 64, 32]));
        let s = Source::from_srgb(&img, 1300, 900);
        assert_eq!((s.width(), s.height()), (1300, 900));
        assert!(s.levels.last().unwrap().w <= 320);
        assert_eq!(s.level_for(500.0).w, 650);
        let p = s.levels[2].at(10, 10);
        assert!((p[0] - s.levels[0].at(10, 10)[0]).abs() < 1e-5, "média de cor constante");
    }

    #[test]
    fn orientation_matches_the_exif_convention() {
        // Plano 3×2 com valores 0..6; orientação 6 (rodar 90° horário) dá 2×3.
        let p = Plane { w: 3, h: 2, px: (0..6).map(|i| [i as f32; 3]).collect() };
        let o = orient(p.clone(), 6);
        assert_eq!((o.w, o.h), (2, 3));
        // Topo da saída = coluna esquerda do original, de baixo para cima: 3, 0.
        assert_eq!((o.at(0, 0)[0], o.at(1, 0)[0]), (3.0, 0.0));
        let o8 = orient(p, 8);
        assert_eq!((o8.at(0, 0)[0], o8.at(1, 0)[0]), (2.0, 5.0));
    }
}
