//! Iluminação a partir da profundidade (protótipo para avaliação).
//!
//! Com a profundidade relativa de um modelo monocular (Depth Anything V2), reconstrói-se uma
//! superfície aproximada: posição 3D por píxel e normal pela variação da profundidade (numa
//! versão suavizada guiada pela imagem, para as arestas coincidirem). A luz nova é calculada em
//! luz linear e aplicada como **multiplicador** da própria fotografia (a fotografia faz de
//! material): onde a superfície está virada para a luz e perto dela, fica mais clara e com a cor
//! da luz; o resto pode escurecer. Assim a textura e o detalhe são sempre os da fotografia.
//!
//! * Foco de luz: ponto de luz colocado na imagem (u, v) à frente do que lá estiver.
//! * Contraluz: luz atrás do sujeito → realce nas arestas (normais quase perpendiculares à
//!   câmara) dos planos próximos, com um halo suave.
//! * Hora do dia: sol com direção, cor e intensidade conforme a hora (nascer → meio-dia →
//!   pôr do sol → hora azul), sombras projetadas aproximadas (marcha na profundidade) e luz do
//!   céu ambiente.

use super::source::Plane;

/// Profundidade preparada para iluminar: distância (maior = mais longe) e normais.
pub struct Surface {
    pub w: usize,
    pub h: usize,
    /// Distância relativa à câmara (1 = mais perto; cresce para o fundo).
    pub dist: Vec<f32>,
    pub normal: Vec<[f32; 3]>,
    /// Proximidade 0–1 (1 = o que está mais perto): para separar sujeito e fundo.
    pub near: Vec<f32>,
}

/// `inv_depth`: saída do modelo (profundidade inversa relativa, maior = mais perto), `dw`×`dh`;
/// `guide`: a fotografia (linear) no tamanho de trabalho `w`×`h`.
pub fn surface(inv_depth: &[f32], dw: usize, dh: usize, guide: &Plane) -> Surface {
    let (w, h) = (guide.w, guide.h);
    let (lo, hi) = inv_depth.iter().fold((f32::MAX, f32::MIN), |a, &v| (a.0.min(v), a.1.max(v)));
    let span = (hi - lo).max(1e-6);
    // Reamostragem bilinear para o tamanho de trabalho.
    let mut d = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let fx = (x as f32 + 0.5) / w as f32 * dw as f32 - 0.5;
            let fy = (y as f32 + 0.5) / h as f32 * dh as f32 - 0.5;
            let (x0, y0) = (
                fx.floor().clamp(0.0, dw as f32 - 1.0) as usize,
                fy.floor().clamp(0.0, dh as f32 - 1.0) as usize,
            );
            let (x1, y1) = ((x0 + 1).min(dw - 1), (y0 + 1).min(dh - 1));
            let (tx, ty) = ((fx - x0 as f32).clamp(0.0, 1.0), (fy - y0 as f32).clamp(0.0, 1.0));
            let a = inv_depth[y0 * dw + x0] * (1.0 - tx) + inv_depth[y0 * dw + x1] * tx;
            let b = inv_depth[y1 * dw + x0] * (1.0 - tx) + inv_depth[y1 * dw + x1] * tx;
            d[y * w + x] = ((a * (1.0 - ty) + b * ty) - lo) / span;
        }
    }
    // Suavização guiada pela imagem (bilateral conjunta separável): arestas da profundidade
    // alinhadas com as da fotografia, superfícies sem o "degrau" do modelo.
    let lum: Vec<f32> = guide
        .px
        .iter()
        .map(|p| (0.2627 * p[0] + 0.678 * p[1] + 0.0593 * p[2]).max(0.0).powf(1.0 / 2.2))
        .collect();
    let r = (w.max(h) / 160).max(2);
    let d = joint_bilateral(&d, &lum, w, h, r, 0.08);
    let d = joint_bilateral(&d, &lum, w, h, r, 0.08);
    // Distância: 1 no mais próximo, ~4 no fundo (perspetiva moderada, estável).
    let dist: Vec<f32> = d.iter().map(|v| 1.0 / (0.25 + 0.75 * v.clamp(0.0, 1.0))).collect();
    // Normais pela variação da distância (em unidades da altura da imagem).
    let scale = h as f32 * 0.6;
    let mut normal = vec![[0f32, 0.0, 1.0]; w * h];
    for y in 0..h {
        for x in 0..w {
            let at = |xx: isize, yy: isize| {
                dist[(yy.clamp(0, h as isize - 1) as usize) * w
                    + xx.clamp(0, w as isize - 1) as usize]
            };
            let (xi, yi) = (x as isize, y as isize);
            let s = 2;
            let dx = (at(xi + s, yi) - at(xi - s, yi)) * scale / (2.0 * s as f32);
            let dy = (at(xi, yi + s) - at(xi, yi - s)) * scale / (2.0 * s as f32);
            let n = [-dx, -dy, 1.0];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            normal[y * w + x] = [n[0] / len, n[1] / len, n[2] / len];
        }
    }
    Surface { w, h, dist, normal, near: d }
}

fn joint_bilateral(
    src: &[f32],
    guide: &[f32],
    w: usize,
    h: usize,
    r: usize,
    sigma_g: f32,
) -> Vec<f32> {
    let pass = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let mut out = vec![0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let (mut acc, mut wsum) = (0f32, 0f32);
                for k in -(r as isize)..=(r as isize) {
                    let (xx, yy) = if horizontal {
                        (x as isize + k, y as isize)
                    } else {
                        (x as isize, y as isize + k)
                    };
                    if xx < 0 || yy < 0 || xx >= w as isize || yy >= h as isize {
                        continue;
                    }
                    let j = yy as usize * w + xx as usize;
                    let ds = k as f32 / r as f32;
                    let dg = (guide[j] - guide[i]) / sigma_g;
                    let wt = (-0.5 * (ds * ds * 4.0 + dg * dg)).exp();
                    acc += src[j] * wt;
                    wsum += wt;
                }
                out[i] = acc / wsum.max(1e-6);
            }
        }
        out
    };
    let a = pass(src, true);
    pass(&a, false)
}

/// Posição 3D (unidades da altura da imagem) do píxel (x, y).
#[allow(dead_code)]
#[inline]
fn pos(s: &Surface, x: usize, y: usize) -> [f32; 3] {
    let aspect = s.w as f32 / s.h as f32;
    let d = s.dist[y * s.w + x];
    // Projeção em perspetiva (campo de visão moderado).
    [
        ((x as f32 + 0.5) / s.w as f32 - 0.5) * aspect * d,
        ((y as f32 + 0.5) / s.h as f32 - 0.5) * d,
        d,
    ]
}

/// Temperatura (K) → cor RGB linear normalizada (aproximação de Planck, luminância 1).
pub fn kelvin_rgb(k: f32) -> [f32; 3] {
    // Ajuste de Tanner Helland (sRGB 0–255) passado a linear.
    let t = (k / 100.0).clamp(10.0, 400.0);
    let r = if t <= 66.0 { 255.0 } else { 329.698_73 * (t - 60.0).powf(-0.133_204_76) };
    let g = if t <= 66.0 {
        99.470_8 * t.ln() - 161.119_57
    } else {
        288.122_16 * (t - 60.0).powf(-0.075_514_85)
    };
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.517_73 * (t - 10.0).ln() - 305.044_8
    };
    let lin = |v: f32| (v.clamp(0.0, 255.0) / 255.0).powf(2.2);
    let c = [lin(r), lin(g), lin(b)];
    let l = 0.2627 * c[0] + 0.678 * c[1] + 0.0593 * c[2];
    c.map(|v| v / l.max(1e-4))
}

/// Resultado de uma luz: fator multiplicativo e luz acrescentada (linear), por píxel.
pub struct Light {
    pub gain: Vec<[f32; 3]>,
    pub add: Vec<[f32; 3]>,
}

fn mean_lum(img: &Plane) -> f32 {
    img.px.iter().map(|p| 0.2627 * p[0] + 0.678 * p[1] + 0.0593 * p[2]).sum::<f32>()
        / img.px.len().max(1) as f32
}

#[derive(Debug, Clone, Copy)]
pub struct Spot {
    /// Ponto iluminado na imagem (0–1).
    pub u: f32,
    pub v: f32,
    /// Raio do círculo de luz (fração da altura da imagem).
    pub radius: f32,
    /// Intensidade (1 = dobra a luz no centro).
    pub intensity: f32,
    pub kelvin: f32,
    /// Quanto escurece o resto (0–1).
    pub ambient_cut: f32,
}

/// Foco de luz: círculo de luz de bordo suave no plano do ponto escolhido (o que está muito à
/// frente ou atrás desse plano recebe menos luz), mais forte nas superfícies viradas para a
/// luz; o resto escurece. Um pouco de luz acrescentada para iluminar também o que era escuro.
pub fn spot(s: &Surface, img: &Plane, sp: &Spot) -> Light {
    let (lx, ly) =
        (((sp.u * s.w as f32) as usize).min(s.w - 1), ((sp.v * s.h as f32) as usize).min(s.h - 1));
    let d_t = s.dist[ly * s.w + lx];
    let color = kelvin_rgb(sp.kelvin);
    let aspect = s.w as f32 / s.h as f32;
    let mean = mean_lum(img);
    let r = sp.radius.max(0.03);
    let n = s.w * s.h;
    let (mut gain, mut add) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for i in 0..n {
        let (x, y) = (i % s.w, i / s.w);
        let dx = ((x as f32 + 0.5) / s.w as f32 - sp.u) * aspect;
        let dy = (y as f32 + 0.5) / s.h as f32 - sp.v;
        let dd = (s.dist[i] - d_t) / d_t;
        let r2 = (dx * dx + dy * dy) / (r * r) + (dd / 0.45).powi(2);
        let pool = (-r2).exp();
        // Superfície virada para a luz (luz à frente do ponto, ligeiramente acima).
        let l = [-dx, -dy - 0.15, 0.6];
        let ll = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        let nn = s.normal[i];
        let facing = 0.55 + 0.45 * ((nn[0] * l[0] + nn[1] * l[1] + nn[2] * l[2]) / ll).max(0.0);
        let keep = 1.0 - sp.ambient_cut * (1.0 - pool).powf(0.6);
        let e = sp.intensity * pool * facing;
        gain.push(color.map(|c| keep + e * c));
        add.push(color.map(|c| c * e * mean * 0.08));
    }
    Light { gain, add }
}

#[derive(Debug, Clone, Copy)]
pub struct Backlight {
    pub intensity: f32,
    pub kelvin: f32,
    /// De onde vem a luz (−1 esquerda … 1 direita; 0 = atrás, ao centro).
    pub side: f32,
}

/// Contraluz: contorno luminoso no sujeito (o que está perto), do lado da luz, com halo
/// suave e um véu de luz no lado de onde ela vem.
pub fn backlight(s: &Surface, img: &Plane, b: &Backlight) -> Light {
    let (w, h) = (s.w, s.h);
    let color = kelvin_rgb(b.kelvin);
    let mean = mean_lum(img);
    // Máscara do sujeito e o seu contorno (variação da máscara suavizada).
    let subject = subject_mask(s);
    let soft = box_blur(&box_blur(&subject, w, h, (h / 200).max(1)), w, h, (h / 200).max(1));
    let mut band = vec![0f32; w * h];
    let mut halo_src = vec![0f32; w * h];
    let (lxv, lyv) = (b.side, -0.8f32);
    let ln = (lxv * lxv + lyv * lyv).sqrt();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let gx = soft[i + 1] - soft[i - 1];
            let gy = soft[i + w] - soft[i - w];
            let g = (gx * gx + gy * gy).sqrt();
            // O contorno do lado da luz: o gradiente aponta para dentro do sujeito.
            let toward = (-(gx * lxv + gy * lyv) / (g.max(1e-6) * ln)).max(0.0);
            let rim = (g * h as f32 / 6.0).min(1.0) * (0.25 + 0.75 * toward) * subject[i].max(0.3);
            band[i] = rim;
            halo_src[i] = rim;
        }
    }
    let halo = box_blur(&box_blur(&halo_src, w, h, (h / 60).max(2)), w, h, (h / 60).max(2));
    let mut add = Vec::with_capacity(w * h);
    for (i, (bd, hl)) in band.iter().zip(&halo).enumerate() {
        let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
        // Véu: mais forte no lado e no topo de onde vem a luz, só no fundo.
        let veil = ((0.5 + 0.5 * b.side * (2.0 * x - 1.0)) * (1.0 - y)).max(0.0).powf(1.5)
            * (1.0 - subject[i]);
        let e = b.intensity * mean * (5.0 * bd + 2.0 * hl + 0.35 * veil);
        add.push(color.map(|c| c * e));
    }
    Light { gain: vec![[1.0; 3]; w * h], add }
}

/// Sujeito (o que se destaca à frente do que o rodeia), 0–1. Compara a proximidade de cada
/// ponto com a do chão/fundo na mesma linha (mediana da linha): um chão que se aproxima da
/// câmara não é sujeito; uma pessoa de pé à frente dele é.
pub fn subject_mask(s: &Surface) -> Vec<f32> {
    let (w, h) = (s.w, s.h);
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        let mut row: Vec<f32> = s.near[y * w..(y + 1) * w].to_vec();
        row.sort_by(f32::total_cmp);
        let base = row[w / 2];
        for x in 0..w {
            out[y * w + x] = smooth(0.06, 0.18, s.near[y * w + x] - base);
        }
    }
    // Pequenas manchas isoladas não são sujeito: abertura suave.
    let r = (h / 150).max(1);
    let b = box_blur(&out, w, h, r);
    b.iter().map(|v| smooth(0.3, 0.7, *v)).collect()
}

#[derive(Debug, Clone, Copy)]
pub struct TimeOfDay {
    /// 0 = nascer do sol, 0,5 = meio-dia, 1 = pôr do sol, 1,2 = hora azul.
    pub t: f32,
    /// Quanto muda (0–1): 0 = fotografia original.
    pub amount: f32,
    /// Lado de onde vem o sol (−1 esquerda, 1 direita).
    pub side: f32,
}

/// Parâmetros do sol para uma hora: (direção para o sol, cor do sol, força do sol, cor do céu,
/// exposição em EV, névoa).
pub fn sun_at(tod: &TimeOfDay) -> ([f32; 3], [f32; 3], f32, [f32; 3], f32, f32) {
    let t = tod.t.clamp(0.0, 1.2);
    let day = t.min(1.0);
    let height = (std::f32::consts::PI * day).sin();
    let low = 1.0 - height;
    let elev = height * 55f32.to_radians() + 4f32.to_radians();
    let side = tod.side.clamp(-1.0, 1.0);
    let to_sun = {
        let v = [side * elev.cos() * 0.9, -elev.sin(), 0.35];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        v.map(|c| c / l)
    };
    let mut sun_col = kelvin_rgb(5600.0 - 3200.0 * low.powf(1.4));
    let mut sun = 0.4 + 0.6 * height;
    let mut sky = kelvin_rgb(7500.0 + 4500.0 * low);
    let mut ev = -0.25 * low;
    let mut haze = 0.12 + 0.35 * low;
    if t > 1.0 {
        let k = ((t - 1.0) / 0.2).clamp(0.0, 1.0);
        sun *= 1.0 - k;
        sun_col = sun_col.map(|c| c * (1.0 - k) + k);
        sky = std::array::from_fn(|c| sky[c] * (1.0 - k) + [0.55, 0.8, 1.9][c] * k);
        ev -= 1.1 * k;
        haze = haze * (1.0 - k) + 0.3 * k;
    }
    (to_sun, sun_col, sun, sky, ev, haze)
}

/// Hora do dia: luz do sol direcional nas superfícies viradas para ele (com sombras
/// aproximadas), realces com a cor do sol e sombras com a cor do céu, exposição da hora e
/// névoa atmosférica pela profundidade (os planos distantes ganham a cor do céu).
#[allow(clippy::needless_range_loop)] // vários vetores indexados pelo mesmo píxel
pub fn time_of_day(s: &Surface, img: &Plane, tod: &TimeOfDay) -> Light {
    let (to_sun, sun_col, sun, sky, ev, haze) = sun_at(tod);
    let shadow = shadows(s, to_sun);
    let mean = mean_lum(img);
    let a = tod.amount.clamp(0.0, 1.0);
    let expo = 2f32.powf(ev);
    let n = s.w * s.h;
    let (mut gain, mut add) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for i in 0..n {
        let x = (i % s.w) as f32 / s.w as f32;
        let p = img.px[i];
        let lum = 0.2627 * p[0] + 0.678 * p[1] + 0.0593 * p[2];
        let nn = s.normal[i];
        let lambert = (nn[0] * to_sun[0] + nn[1] * to_sun[1] + nn[2] * to_sun[2]).max(0.0);
        // Luz direta: superfícies viradas para o sol e o lado da imagem de onde ele vem.
        let side_light = 1.0 + 0.35 * sun * tod.side * (2.0 * x - 1.0);
        let key = (0.55 + 0.9 * sun * lambert * shadow[i]) * side_light;
        // Realces → cor do sol; sombras → cor do céu.
        let hl = smooth(0.02, 0.45, lum / (mean * 2.5).max(1e-4));
        let tint: [f32; 3] =
            std::array::from_fn(|c| sky[c] * (1.0 - hl) * 0.9 + sun_col[c] * hl * 1.05);
        let g: [f32; 3] = std::array::from_fn(|c| expo * key * tint[c] / 1.0);
        gain.push(g.map(|v| 1.0 + (v - 1.0) * a));
        // Névoa: mais forte ao longe.
        // Névoa só no que está mesmo longe (céu, horizonte): os planos médios ficam limpos.
        let far = smooth(0.6, 0.95, 1.0 - s.near[i]);
        let hz = haze * far * a * mean * expo * 0.8;
        add.push(std::array::from_fn(|c| hz * (sky[c] * 0.6 + sun_col[c] * 0.4)));
    }
    Light { gain, add }
}

/// Sombras projetadas aproximadas: marcha no espaço da imagem em direção ao sol; sombra se a
/// superfície à frente estiver mais perto da câmara do que o raio (com tolerância), suavizada.
fn shadows(s: &Surface, to_sun: [f32; 3]) -> Vec<f32> {
    let (w, h) = (s.w, s.h);
    let step_xy = [to_sun[0], to_sun[1]];
    let len = (step_xy[0] * step_xy[0] + step_xy[1] * step_xy[1]).sqrt();
    if len < 1e-3 {
        return vec![1.0; w * h];
    }
    let (sx, sy) = (step_xy[0] / len, step_xy[1] / len);
    let steps = (w.max(h) / 6).clamp(16, 120);
    let mut out = vec![1f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let d0 = s.dist[i];
            let mut occl = 0f32;
            for k in 1..steps {
                let fx = x as f32 + sx * k as f32 * 2.0;
                let fy = y as f32 + sy * k as f32 * 2.0;
                if fx < 0.0 || fy < 0.0 || fx >= w as f32 || fy >= h as f32 {
                    break;
                }
                // O raio sobe em direção à câmara (z diminui) conforme a elevação do sol.
                let ray = d0 + to_sun[2] * (k as f32 * 2.0 / h as f32) * d0 * 1.5;
                let dz = ray - s.dist[fy as usize * w + fx as usize];
                if dz > 0.04 * d0 {
                    occl = occl.max(smooth(0.04, 0.2, dz / d0));
                }
            }
            out[i] = 1.0 - 0.85 * occl;
        }
    }
    // Penumbra: desfoque curto.
    let blurred = box_blur(&out, w, h, (w.max(h) / 300).max(1));
    box_blur(&blurred, w, h, (w.max(h) / 300).max(1))
}

fn box_blur(v: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut t = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            t[y * w + x] = v[y * w + a..=y * w + b].iter().sum::<f32>() / (b - a + 1) as f32;
        }
    }
    let mut o = vec![0f32; w * h];
    for y in 0..h {
        let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            o[y * w + x] = (a..=b).map(|yy| t[yy * w + x]).sum::<f32>() / (b - a + 1) as f32;
        }
    }
    o
}

#[inline]
fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
