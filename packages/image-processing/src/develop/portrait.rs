//! **Retrato** — retoque subtil, só nas regiões de cada rosto (malha facial local):
//! suavização de pele por separação de frequências (mantém os poros), olheiras, brilho,
//! dentes e olhos. Não muda formas nem proporções (nada é deformado) e todas as
//! quantidades estão limitadas; com 0 o resultado é exatamente o original.

use super::color::luma;
use super::filters::gaussian;
use super::local::{cached, fill_polygon, hash_of, Raster};
use super::settings::{FaceRegions, Portrait};
use std::sync::Arc;

/// Regiões de um rosto rasterizadas (grelha local à volta do rosto).
pub struct PFace {
    pub skin: Arc<Raster>,
    pub under: Arc<Raster>,
    pub teeth: Arc<Raster>,
    pub sclera: Arc<Raster>,
    pub iris: Arc<Raster>,
    /// Largura do rosto (unidades do lado maior).
    pub size: f32,
}

pub struct PreparedPortrait {
    pub p: Portrait,
    pub faces: Vec<PFace>,
}

fn bbox(pts: &[[f32; 2]]) -> [f32; 4] {
    pts.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |b, p| {
        [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
    })
}

fn region(f: &FaceRegions, which: &str, ru: f32, rv: f32) -> Raster {
    let b = bbox(&f.oval);
    let (mw, mh) = ((b[2] - b[0]) * 0.15, (b[3] - b[1]) * 0.15);
    let bounds = [b[0] - mw, b[1] - mh, b[2] + mw, b[3] + mh];
    // ~256 células na largura do rosto.
    let aspect = ((bounds[2] - bounds[0]) * ru) / ((bounds[3] - bounds[1]) * rv).max(1e-6);
    let w = 256usize;
    let h = ((w as f32 / aspect).round() as usize).clamp(16, 1024);
    let mut r = Raster::new(bounds, w, h);
    let circle = |r: &mut Raster, c: [f32; 3], scale: f32, val: f32| {
        for j in 0..r.h {
            for i in 0..r.w {
                let (u, v) = r.cell_uv(i, j);
                if (((u - c[0]) * ru).powi(2) + ((v - c[1]) * rv).powi(2)).sqrt() < c[2] * scale {
                    r.data[j * r.w + i] = val;
                }
            }
        }
    };
    let feather = w as f32 * 0.012;
    match which {
        "skin" => {
            fill_polygon(&mut r, &f.oval, 1.0);
            for p in [&f.left_eye, &f.right_eye, &f.left_brow, &f.right_brow, &f.lips_outer] {
                fill_polygon(&mut r, p, 0.0);
            }
            // Sem as íris (aumentadas): a borda dos olhos fica intacta.
            circle(&mut r, f.left_iris, 1.8, 0.0);
            circle(&mut r, f.right_iris, 1.8, 0.0);
            // Transição só para dentro: o contorno do rosto, olhos e lábios nunca é tocado.
            let mut r = r.blurred(feather * 1.5);
            r.data.iter_mut().for_each(|v| *v = smooth(0.6, 1.0, *v));
            r
        }
        "under" => {
            fill_polygon(&mut r, &f.under_left, 1.0);
            fill_polygon(&mut r, &f.under_right, 1.0);
            r.blurred(feather * 2.0)
        }
        "teeth" => {
            fill_polygon(&mut r, &f.lips_inner, 1.0);
            r.blurred(feather * 0.5)
        }
        "sclera" => {
            fill_polygon(&mut r, &f.left_eye, 1.0);
            fill_polygon(&mut r, &f.right_eye, 1.0);
            circle(&mut r, f.left_iris, 1.0, 0.0);
            circle(&mut r, f.right_iris, 1.0, 0.0);
            r.blurred(feather * 0.4)
        }
        _ => {
            circle(&mut r, f.left_iris, 0.95, 1.0);
            circle(&mut r, f.right_iris, 0.95, 1.0);
            // Só dentro do contorno visível dos olhos (pálpebras).
            let mut eye = Raster::new(bounds, w, h);
            fill_polygon(&mut eye, &f.left_eye, 1.0);
            fill_polygon(&mut eye, &f.right_eye, 1.0);
            for (a, e) in r.data.iter_mut().zip(&eye.data) {
                *a *= e;
            }
            r.blurred(feather * 0.4)
        }
    }
}

pub fn prepare(p: &Portrait, ru: f32, rv: f32) -> Option<PreparedPortrait> {
    let active =
        p.skin > 0.0 || p.dark_circles > 0.0 || p.shine > 0.0 || p.teeth > 0.0 || p.eyes > 0.0;
    if !active || p.faces.is_empty() {
        return None;
    }
    let faces = p
        .faces
        .iter()
        .filter(|f| f.oval.len() >= 3)
        .map(|f| {
            let key = |w: &str| hash_of(f, &format!("{w}{ru}{rv}"));
            let b = bbox(&f.oval);
            PFace {
                skin: cached(key("skin"), || region(f, "skin", ru, rv)),
                under: cached(key("under"), || region(f, "under", ru, rv)),
                teeth: cached(key("teeth"), || region(f, "teeth", ru, rv)),
                sclera: cached(key("sclera"), || region(f, "sclera", ru, rv)),
                iris: cached(key("iris"), || region(f, "iris", ru, rv)),
                size: (b[2] - b[0]) * ru,
            }
        })
        .collect();
    Some(PreparedPortrait { p: p.clone(), faces })
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// Aplica o retrato ao render (`disp` em visualização). `uv` = posição na fotografia de cada
/// píxel (None fora da imagem); `px_per_unit` = píxeis do render por unidade do lado maior.
pub fn apply(
    pp: &PreparedPortrait,
    disp: &mut [[f32; 3]],
    w: usize,
    h: usize,
    uv: &[Option<[f32; 2]>],
    px_per_unit: f32,
) {
    let k = |v: f32| (v / 100.0).clamp(0.0, 1.0);
    let (ks, kd, kh, kt, ke) =
        (k(pp.p.skin), k(pp.p.dark_circles), k(pp.p.shine), k(pp.p.teeth), k(pp.p.eyes));
    let n = w * h;
    // Planos (máximo entre rostos).
    let mut skin = vec![0.0f32; n];
    let mut under = vec![0.0f32; n];
    let mut teeth = vec![0.0f32; n];
    let mut sclera = vec![0.0f32; n];
    let mut iris = vec![0.0f32; n];
    for (i, p) in uv.iter().enumerate() {
        let Some([u, v]) = *p else { continue };
        for f in &pp.faces {
            let s = &f.skin;
            if u < s.x0 || u > s.x1 || v < s.y0 || v > s.y1 {
                continue;
            }
            skin[i] = skin[i].max(f.skin.sample(u, v));
            under[i] = under[i].max(f.under.sample(u, v));
            teeth[i] = teeth[i].max(f.teeth.sample(u, v));
            sclera[i] = sclera[i].max(f.sclera.sample(u, v));
            iris[i] = iris[i].max(f.iris.sample(u, v));
        }
    }
    let face_px = pp.faces.iter().map(|f| f.size).fold(0.0f32, f32::max) * px_per_unit;
    if face_px < 8.0 {
        return;
    }
    let y: Vec<f32> = disp.iter().map(|p| luma(*p)).collect();
    // Referências da pele: luminância e cor médias (sem as zonas brilhantes).
    let (mut sl, mut sc, mut sn) = (0.0, [0.0f32; 3], 0.0);
    for i in 0..n {
        if skin[i] > 0.5 && y[i] < 0.75 && y[i] > 0.1 {
            sl += y[i];
            for c in 0..3 {
                sc[c] += disp[i][c] - y[i];
            }
            sn += 1.0;
        }
    }
    let (skin_l, skin_chroma) =
        if sn > 0.0 { (sl / sn, sc.map(|v| v / sn)) } else { (0.5, [0.0; 3]) };
    let need_blur = ks > 0.0 || ke > 0.0;
    let (fine, mid) = if need_blur {
        (
            gaussian(&y, w, h, (face_px * 0.006).max(0.8)),
            gaussian(&y, w, h, (face_px * 0.035).max(2.0)),
        )
    } else {
        (vec![], vec![])
    };
    // Croma desfocada para uniformizar manchas vermelhas da pele.
    let chroma_blur: Vec<Vec<f32>> = if ks > 0.0 {
        (0..3)
            .map(|c| {
                gaussian(
                    &disp.iter().zip(&y).map(|(p, l)| p[c] - l).collect::<Vec<_>>(),
                    w,
                    h,
                    (face_px * 0.03).max(2.0),
                )
            })
            .collect()
    } else {
        vec![]
    };
    for i in 0..n {
        if skin[i] == 0.0
            && under[i] == 0.0
            && teeth[i] == 0.0
            && sclera[i] == 0.0
            && iris[i] == 0.0
        {
            continue;
        }
        let mut d = disp[i];
        let mut l = y[i];
        let mut ch: [f32; 3] = std::array::from_fn(|c| d[c] - l);
        if ks > 0.0 && skin[i] > 0.0 {
            // Separação de frequências: atenua a banda média (manchas, irregularidades) e
            // mantém a fina (poros) — no máximo 75 % da banda média.
            let band = fine[i] - mid[i];
            l -= band * 0.75 * ks * skin[i];
            ch = std::array::from_fn(|c| ch[c] + (chroma_blur[c][i] - ch[c]) * 0.4 * ks * skin[i]);
        }
        if kh > 0.0 && skin[i] > 0.0 {
            let sat = ch.iter().map(|v| v.abs()).fold(0.0, f32::max) / l.max(0.05);
            let spec = smooth(0.72, 0.95, l) * (1.0 - sat * 2.0).clamp(0.0, 1.0);
            let m = spec * kh * skin[i];
            l -= (l - 0.62).max(0.0) * 0.45 * m;
            ch = mix(ch, skin_chroma, 0.6 * m);
        }
        if kd > 0.0 && under[i] > 0.0 {
            let m = kd * under[i];
            l += (skin_l - l).max(0.0) * 0.55 * m;
            ch = ch.map(|v| v * (1.0 - 0.3 * m));
        }
        if kt > 0.0 && teeth[i] > 0.0 && l > 0.3 {
            // Branquear: menos amarelo e um pouco mais de luz, só nos píxeis claros da boca.
            let m = kt * teeth[i] * smooth(0.3, 0.45, l);
            let yellow = ((d[0] + d[1]) * 0.5 - d[2]).max(0.0);
            ch[2] += yellow * 0.55 * m;
            ch[0] -= yellow * 0.2 * m;
            ch[1] -= yellow * 0.2 * m;
            l = (l + 0.05 * m).min(l.max(0.92));
        }
        if ke > 0.0 {
            if iris[i] > 0.0 {
                let m = ke * iris[i];
                l += (0.03 + 0.5 * (l - fine[i]).max(-0.05)) * m * 0.5;
            }
            if sclera[i] > 0.0 {
                let m = ke * sclera[i] * smooth(0.35, 0.55, l);
                ch = ch.map(|v| v * (1.0 - 0.45 * m));
                l = (l + 0.03 * m).min(0.9_f32.max(y[i]));
            }
        }
        d = std::array::from_fn(|c| (l + ch[c]).max(0.0));
        disp[i] = d;
    }
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
