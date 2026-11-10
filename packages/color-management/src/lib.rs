//! # momento-color-management
//!
//! Gestão de cor da exportação (Fase 9), com a LittleCMS 2 para os perfis ICC.
//!
//! O motor de revelação entrega a imagem final em **Rec. 2020 linear** (D65, 0–1). Aqui
//! convertemos para o espaço de saída:
//!
//! - **espaços normalizados** (sRGB, Display P3, Adobe RGB, ProPhoto RGB, Rec. 709,
//!   Rec. 2020): conversão por matriz (+ adaptação Bradford D65→D50 para o ProPhoto, como
//!   o intento colorimétrico relativo de qualquer CMM) e a curva do espaço; o perfil ICC
//!   embutido é gerado pela LittleCMS a partir das mesmas primárias e curva;
//! - **ICC personalizado / perfil do laboratório**: transformação LittleCMS (intento
//!   percetual ou colorimétrico relativo, com compensação do ponto negro). Só perfis RGB.
//!
//! Fora da gama do destino, os valores são cortados por componente (como no ecrã).

use momento_core_types::{CoreError, CoreResult};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Espaço de cor de saída.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorSpace {
    Srgb,
    DisplayP3,
    AdobeRgb,
    ProPhoto,
    Rec709,
    Rec2020,
    /// Perfil ICC fornecido pelo utilizador (personalizado ou do laboratório).
    Icc,
}

impl ColorSpace {
    pub const BUILTIN: [ColorSpace; 6] = [
        ColorSpace::Srgb,
        ColorSpace::DisplayP3,
        ColorSpace::AdobeRgb,
        ColorSpace::ProPhoto,
        ColorSpace::Rec709,
        ColorSpace::Rec2020,
    ];

    pub fn id(self) -> &'static str {
        match self {
            ColorSpace::Srgb => "srgb",
            ColorSpace::DisplayP3 => "displayP3",
            ColorSpace::AdobeRgb => "adobeRgb",
            ColorSpace::ProPhoto => "proPhoto",
            ColorSpace::Rec709 => "rec709",
            ColorSpace::Rec2020 => "rec2020",
            ColorSpace::Icc => "icc",
        }
    }
}

/// Codificação rápida com tabela de 65 537 entradas e interpolação linear.
///
/// Medido na Fase 12: `powf` por canal era 17 % do tempo da exportação. O erro máximo da
/// tabela é < 2,5e-7 (≈ 1,6 % de um nível de 16 bits; medido no teste); perto de 0 nas curvas gama puras
/// (derivada infinita) e fora de [0, 1] usa a fórmula exata.
pub struct EncodeLut {
    curve: Curve,
    lo: f32,
    /// Ponto de quebra da curva por partes (a interpolação não o atravessa).
    kink: f32,
    t: Vec<f32>,
}

impl EncodeLut {
    const N: usize = 65_536;

    pub fn new(curve: Curve) -> Self {
        let lo = match curve {
            Curve::Gamma(_) => 0.004,
            _ => 0.0,
        };
        let kink = match curve {
            Curve::Srgb => 0.003_130_8,
            Curve::Bt709 => 0.018,
            _ => -1.0,
        };
        let t = if curve == Curve::Linear {
            Vec::new()
        } else {
            (0..=Self::N).map(|i| curve.encode(i as f32 / Self::N as f32)).collect()
        };
        EncodeLut { curve, lo, kink, t }
    }

    #[inline]
    pub fn encode(&self, x: f32) -> f32 {
        if self.t.is_empty()
            || !(self.lo..=1.0).contains(&x)
            || (x - self.kink).abs() < 2.0 / Self::N as f32
        {
            return self.curve.encode(x);
        }
        let p = x * Self::N as f32;
        let i = (p as usize).min(Self::N - 1);
        let f = p - i as f32;
        self.t[i] + (self.t[i + 1] - self.t[i]) * f
    }
}

/// Curva de transferência (linear → codificado).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Curve {
    Linear,
    Srgb,
    Gamma(f64),
    /// ITU-R BT.709 / BT.2020 (OETF).
    Bt709,
}

impl Curve {
    #[inline]
    pub fn encode(self, x: f32) -> f32 {
        match self {
            Curve::Linear => x,
            Curve::Srgb => {
                if x <= 0.003_130_8 {
                    12.92 * x
                } else {
                    1.055 * x.powf(1.0 / 2.4) - 0.055
                }
            }
            Curve::Gamma(g) => x.max(0.0).powf(1.0 / g as f32),
            Curve::Bt709 => {
                if x < 0.018 {
                    4.5 * x
                } else {
                    1.099 * x.powf(0.45) - 0.099
                }
            }
        }
    }

    #[inline]
    pub fn decode(self, v: f32) -> f32 {
        match self {
            Curve::Linear => v,
            Curve::Srgb => {
                if v <= 0.040_45 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            }
            Curve::Gamma(g) => v.max(0.0).powf(g as f32),
            Curve::Bt709 => {
                if v < 0.081 {
                    v / 4.5
                } else {
                    ((v + 0.099) / 1.099).powf(1.0 / 0.45)
                }
            }
        }
    }

    /// Tabela de codificação para muitos píxeis (ver [`EncodeLut`]).
    pub fn encode_lut(self) -> EncodeLut {
        EncodeLut::new(self)
    }

    fn tone_curve(self) -> CoreResult<lcms2::ToneCurve> {
        let t = match self {
            Curve::Linear => lcms2::ToneCurve::new(1.0),
            Curve::Gamma(g) => lcms2::ToneCurve::new(g),
            // Tipo 4: Y = (aX + b)^g se X ≥ d, senão cX.
            Curve::Srgb => lcms2::ToneCurve::new_parametric(
                4,
                &[2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.040_45],
            )
            .map_err(lcms_err)?,
            Curve::Bt709 => lcms2::ToneCurve::new_parametric(
                4,
                &[1.0 / 0.45, 1.0 / 1.099, 0.099 / 1.099, 1.0 / 4.5, 0.081],
            )
            .map_err(lcms_err)?,
        };
        Ok(t)
    }
}

/// Primárias e branco (cromaticidades xy).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Primaries {
    pub r: (f64, f64),
    pub g: (f64, f64),
    pub b: (f64, f64),
    pub white: (f64, f64),
}

pub const D65: (f64, f64) = (0.3127, 0.3290);
pub const D50: (f64, f64) = (0.3457, 0.3585);

pub const REC709: Primaries =
    Primaries { r: (0.64, 0.33), g: (0.30, 0.60), b: (0.15, 0.06), white: D65 };
pub const P3: Primaries =
    Primaries { r: (0.680, 0.320), g: (0.265, 0.690), b: (0.150, 0.060), white: D65 };
pub const ADOBE: Primaries =
    Primaries { r: (0.64, 0.33), g: (0.21, 0.71), b: (0.15, 0.06), white: D65 };
pub const PROPHOTO: Primaries =
    Primaries { r: (0.7347, 0.2653), g: (0.1596, 0.8404), b: (0.0366, 0.0001), white: D50 };
pub const REC2020: Primaries =
    Primaries { r: (0.708, 0.292), g: (0.170, 0.797), b: (0.131, 0.046), white: D65 };

/// Definição de um espaço normalizado.
pub fn definition(space: ColorSpace) -> Option<(Primaries, Curve, &'static str)> {
    Some(match space {
        ColorSpace::Srgb => (REC709, Curve::Srgb, "sRGB IEC61966-2.1"),
        ColorSpace::DisplayP3 => (P3, Curve::Srgb, "Display P3"),
        ColorSpace::AdobeRgb => (ADOBE, Curve::Gamma(563.0 / 256.0), "Adobe RGB (1998) compatible"),
        ColorSpace::ProPhoto => (PROPHOTO, Curve::Gamma(1.8), "ProPhoto RGB"),
        ColorSpace::Rec709 => (REC709, Curve::Bt709, "Rec. ITU-R BT.709"),
        ColorSpace::Rec2020 => (REC2020, Curve::Bt709, "Rec. ITU-R BT.2020"),
        ColorSpace::Icc => return None,
    })
}

type M3 = [[f64; 3]; 3];

fn mul(a: &M3, b: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

fn inv(m: &M3) -> M3 {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    [
        [(e * i - f * h) / det, (c * h - b * i) / det, (b * f - c * e) / det],
        [(f * g - d * i) / det, (a * i - c * g) / det, (c * d - a * f) / det],
        [(d * h - e * g) / det, (b * g - a * h) / det, (a * e - b * d) / det],
    ]
}

fn xyz(xy: (f64, f64)) -> [f64; 3] {
    [xy.0 / xy.1, 1.0, (1.0 - xy.0 - xy.1) / xy.1]
}

/// RGB linear → XYZ (branco do próprio espaço, Y = 1).
pub fn rgb_to_xyz(p: &Primaries) -> M3 {
    let (r, g, b) = (xyz(p.r), xyz(p.g), xyz(p.b));
    let m = [[r[0], g[0], b[0]], [r[1], g[1], b[1]], [r[2], g[2], b[2]]];
    let w = xyz(p.white);
    let mi = inv(&m);
    let s: [f64; 3] = std::array::from_fn(|i| (0..3).map(|k| mi[i][k] * w[k]).sum());
    std::array::from_fn(|i| std::array::from_fn(|j| m[i][j] * s[j]))
}

/// Adaptação cromática Bradford de `from` para `to`.
pub fn bradford(from: (f64, f64), to: (f64, f64)) -> M3 {
    const B: M3 = [[0.8951, 0.2664, -0.1614], [-0.7502, 1.7135, 0.0367], [0.0389, -0.0685, 1.0296]];
    let (s, d) = (xyz(from), xyz(to));
    let cs: [f64; 3] = std::array::from_fn(|i| (0..3).map(|k| B[i][k] * s[k]).sum());
    let cd: [f64; 3] = std::array::from_fn(|i| (0..3).map(|k| B[i][k] * d[k]).sum());
    let diag = [[cd[0] / cs[0], 0.0, 0.0], [0.0, cd[1] / cs[1], 0.0], [0.0, 0.0, cd[2] / cs[2]]];
    mul(&inv(&B), &mul(&diag, &B))
}

/// Rec. 2020 linear (D65) → RGB linear do destino.
pub fn from_rec2020(dst: &Primaries) -> [[f32; 3]; 3] {
    let mut to_xyz = rgb_to_xyz(&REC2020);
    if dst.white != D65 {
        to_xyz = mul(&bradford(D65, dst.white), &to_xyz);
    }
    let m = mul(&inv(&rgb_to_xyz(dst)), &to_xyz);
    m.map(|r| r.map(|v| v as f32))
}

fn lcms_err(e: lcms2::Error) -> CoreError {
    CoreError::Other(format!("gestão de cor: {e}"))
}

fn mlu(text: &str) -> lcms2::MLU {
    let mut m = lcms2::MLU::new(1);
    m.set_text_ascii(text, lcms2::Locale::new("en_US"));
    m
}

/// Perfil ICC (v2, matriz + curvas) de um espaço normalizado; `linear` usa a curva linear
/// (EXR, TIFF de vírgula flutuante).
pub fn builtin_profile(space: ColorSpace, linear: bool) -> CoreResult<lcms2::Profile> {
    let (p, curve, name) =
        definition(space).ok_or_else(|| CoreError::Other("espaço de cor sem definição".into()))?;
    let curve = if linear { Curve::Linear } else { curve };
    let c = |xy: (f64, f64)| lcms2::CIExyY { x: xy.0, y: xy.1, Y: 1.0 };
    let tc = curve.tone_curve()?;
    let mut prof = lcms2::Profile::new_rgb(
        &c(p.white),
        &lcms2::CIExyYTRIPLE { Red: c(p.r), Green: c(p.g), Blue: c(p.b) },
        &[&tc, &tc, &tc],
    )
    .map_err(lcms_err)?;
    let desc = if linear { format!("{name} (linear)") } else { name.to_string() };
    prof.write_tag(lcms2::TagSignature::ProfileDescriptionTag, lcms2::Tag::MLU(&mlu(&desc)));
    prof.write_tag(
        lcms2::TagSignature::CopyrightTag,
        lcms2::Tag::MLU(&mlu("No copyright, use freely (PFF ProMaster)")),
    );
    // v2: compatível com todos os programas e laboratórios (os v4 ainda falham nalguns).
    prof.set_version(2.1);
    Ok(prof)
}

/// Intento de reprodução para perfis ICC personalizados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Intent {
    #[default]
    Perceptual,
    RelativeColorimetric,
}

/// Descrição de um perfil ICC e validação (só RGB).
pub fn inspect_icc(icc: &[u8]) -> CoreResult<String> {
    let p = lcms2::Profile::new_icc(icc)
        .map_err(|_| CoreError::Corrupt("perfil ICC inválido".into()))?;
    if p.color_space() != lcms2::ColorSpaceSignature::RgbData {
        return Err(CoreError::UnsupportedFormat(
            "perfil ICC não RGB (os formatos de fotografia exportam em RGB)".into(),
        ));
    }
    Ok(p.info(lcms2::InfoType::Description, lcms2::Locale::none()).unwrap_or_default())
}

/// Perfil ICC embutido num ficheiro de entrada (JPEG/TIFF/PNG/WebP… em Adobe RGB, Display P3,
/// ProPhoto, perfis de câmara/scanner): converte os valores codificados [0, 1] para Rec. 2020
/// linear, o espaço de trabalho da revelação. `None` quando o perfil é sRGB (o caminho
/// rápido sem LittleCMS dá o mesmo resultado) ou não é RGB (o descodificador já entregou RGB
/// e o perfil não se aplica).
pub struct InputColor {
    pub description: String,
    t: lcms2::Transform<[f32; 3], [f32; 3], lcms2::GlobalContext, lcms2::DisallowCache>,
}

impl InputColor {
    pub fn new(icc: &[u8]) -> CoreResult<Option<Self>> {
        let src = lcms2::Profile::new_icc(icc)
            .map_err(|_| CoreError::Corrupt("perfil ICC embutido inválido".into()))?;
        if src.color_space() != lcms2::ColorSpaceSignature::RgbData {
            return Ok(None);
        }
        let description =
            src.info(lcms2::InfoType::Description, lcms2::Locale::none()).unwrap_or_default();
        if is_srgb_description(&description) {
            return Ok(None);
        }
        let dst = builtin_profile(ColorSpace::Rec2020, true)?;
        let t = lcms2::Transform::new_flags_context(
            lcms2::GlobalContext::new(),
            &src,
            lcms2::PixelFormat::RGB_FLT,
            &dst,
            lcms2::PixelFormat::RGB_FLT,
            lcms2::Intent::RelativeColorimetric,
            lcms2::Flags::NO_CACHE,
        )
        .map_err(lcms_err)?;
        Ok(Some(InputColor { description, t }))
    }

    /// Converte em paralelo, no lugar, valores codificados do perfil para Rec. 2020 linear.
    pub fn to_linear_rec2020(&self, px: &mut [[f32; 3]]) {
        px.par_chunks_mut(4096).for_each(|chunk| {
            let src = chunk.to_vec();
            self.t.transform_pixels(&src, chunk);
            for q in chunk.iter_mut() {
                *q = q.map(|c| if c.is_finite() { c.max(0.0) } else { 0.0 });
            }
        });
    }
}

fn is_srgb_description(d: &str) -> bool {
    let d = d.to_ascii_lowercase();
    d.starts_with("srgb") || d.contains("iec61966-2.1") || d.contains("iec 61966-2-1")
}

enum Kind {
    Matrix { m: [[f32; 3]; 3], curve: Curve },
    Icc(lcms2::Transform<[f32; 3], [f32; 3], lcms2::GlobalContext, lcms2::DisallowCache>),
}

/// Conversão de Rec. 2020 linear para o espaço de saída, com o perfil a embutir.
pub struct OutputColor {
    pub space: ColorSpace,
    pub linear: bool,
    pub description: String,
    icc: Vec<u8>,
    kind: Kind,
}

impl OutputColor {
    pub fn builtin(space: ColorSpace, linear: bool) -> CoreResult<Self> {
        let (p, curve, _) = definition(space)
            .ok_or_else(|| CoreError::Other("espaço de cor sem definição".into()))?;
        let prof = builtin_profile(space, linear)?;
        let description =
            prof.info(lcms2::InfoType::Description, lcms2::Locale::none()).unwrap_or_default();
        Ok(OutputColor {
            space,
            linear,
            description,
            icc: prof.icc().map_err(lcms_err)?,
            kind: Kind::Matrix {
                m: from_rec2020(&p),
                curve: if linear { Curve::Linear } else { curve },
            },
        })
    }

    /// Perfil ICC personalizado (RGB). A imagem é convertida pela LittleCMS.
    pub fn custom(icc: &[u8], intent: Intent) -> CoreResult<Self> {
        let description = inspect_icc(icc)?;
        let dst = lcms2::Profile::new_icc(icc).map_err(lcms_err)?;
        let src = builtin_profile(ColorSpace::Rec2020, true)?;
        let (intent, flags) = match intent {
            Intent::Perceptual => (lcms2::Intent::Perceptual, lcms2::Flags::NO_CACHE),
            Intent::RelativeColorimetric => (
                lcms2::Intent::RelativeColorimetric,
                lcms2::Flags::NO_CACHE | lcms2::Flags::BLACKPOINT_COMPENSATION,
            ),
        };
        let t = lcms2::Transform::new_flags_context(
            lcms2::GlobalContext::new(),
            &src,
            lcms2::PixelFormat::RGB_FLT,
            &dst,
            lcms2::PixelFormat::RGB_FLT,
            intent,
            flags,
        )
        .map_err(lcms_err)?;
        Ok(OutputColor {
            space: ColorSpace::Icc,
            linear: false,
            description,
            icc: icc.to_vec(),
            kind: Kind::Icc(t),
        })
    }

    /// Perfil a embutir no ficheiro.
    pub fn icc(&self) -> &[u8] {
        &self.icc
    }

    /// Primárias/curva (para formatos que as sinalizam sem ICC: AVIF, JPEG XL, EXR).
    pub fn definition(&self) -> Option<(Primaries, Curve)> {
        let (p, c, _) = definition(self.space)?;
        Some((p, if self.linear { Curve::Linear } else { c }))
    }

    /// Converte (em paralelo) para valores codificados do destino em [0, 1]; em saídas
    /// lineares os realces acima de 1 mantêm-se.
    pub fn convert(&self, px: &mut [[f32; 3]]) {
        match &self.kind {
            Kind::Matrix { m, curve } => {
                let (m, linear) = (*m, self.linear);
                let curve = curve.encode_lut();
                px.par_iter_mut().for_each(|q| {
                    let v = [
                        m[0][0] * q[0] + m[0][1] * q[1] + m[0][2] * q[2],
                        m[1][0] * q[0] + m[1][1] * q[1] + m[1][2] * q[2],
                        m[2][0] * q[0] + m[2][1] * q[1] + m[2][2] * q[2],
                    ];
                    *q = v.map(|c| {
                        let c = if linear { c.max(0.0) } else { c.clamp(0.0, 1.0) };
                        curve.encode(c)
                    });
                });
            }
            Kind::Icc(t) => {
                px.par_chunks_mut(4096).for_each(|chunk| {
                    let src = chunk.to_vec();
                    t.transform_pixels(&src, chunk);
                    for q in chunk.iter_mut() {
                        *q = q.map(|c| c.clamp(0.0, 1.0));
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um ficheiro em Adobe RGB / ProPhoto / P3 é lido com as cores certas: exportar em
    /// cada espaço e voltar a ler pelo perfil embutido devolve o Rec. 2020 linear original.
    #[test]
    fn embedded_input_profiles_round_trip() {
        let orig: Vec<[f32; 3]> = vec![
            [0.15, 0.35, 0.15],
            [0.5, 0.2, 0.15],
            [0.2, 0.22, 0.5],
            [0.18, 0.18, 0.18],
            [0.9, 0.85, 0.7],
        ];
        for space in [ColorSpace::AdobeRgb, ColorSpace::DisplayP3, ColorSpace::ProPhoto] {
            let out = OutputColor::builtin(space, false).unwrap();
            let mut px = orig.clone();
            out.convert(&mut px);
            let input = InputColor::new(out.icc()).unwrap().expect("perfil não sRGB");
            input.to_linear_rec2020(&mut px);
            for (a, b) in orig.iter().zip(&px) {
                for c in 0..3 {
                    assert!((a[c] - b[c]).abs() < 0.01, "{space:?}: {a:?} → {b:?}");
                }
            }
        }
        let srgb = OutputColor::builtin(ColorSpace::Srgb, false).unwrap();
        assert!(InputColor::new(srgb.icc()).unwrap().is_none(), "sRGB usa o caminho rápido");
        assert!(InputColor::new(b"lixo").is_err());
    }

    /// A conversão por matriz coincide com a LittleCMS a usar o perfil que embutimos —
    /// ou seja, o ficheiro é lido com as cores certas por qualquer programa.
    #[test]
    fn encode_lut_matches_the_formula() {
        for c in [Curve::Srgb, Curve::Bt709, Curve::Gamma(2.2), Curve::Gamma(1.8), Curve::Linear] {
            let lut = c.encode_lut();
            let mut worst = 0.0f32;
            for i in 0..=2_000_000u32 {
                let x = i as f32 / 2_000_000.0;
                worst = worst.max((lut.encode(x) - c.encode(x)).abs());
            }
            for x in [-0.5f32, 1.5, 7.0] {
                assert_eq!(lut.encode(x), c.encode(x), "fora de [0,1] é exato");
            }
            eprintln!("{c:?}: erro máximo {worst:e}");
            assert!(worst < 2.5e-7, "{c:?}: erro {worst}");
        }
    }

    #[test]
    fn matrix_conversion_matches_lcms_with_embedded_profile() {
        let src = builtin_profile(ColorSpace::Rec2020, true).unwrap();
        let samples: Vec<[f32; 3]> = vec![
            [0.18, 0.18, 0.18],
            [0.5, 0.2, 0.1],
            [0.05, 0.3, 0.6],
            [0.9, 0.85, 0.7],
            [0.01, 0.01, 0.02],
            [0.3, 0.6, 0.2],
        ];
        for space in ColorSpace::BUILTIN {
            for linear in [false, true] {
                let out = OutputColor::builtin(space, linear).unwrap();
                let dst = lcms2::Profile::new_icc(out.icc()).unwrap();
                let t: lcms2::Transform<[f32; 3], [f32; 3]> = lcms2::Transform::new(
                    &src,
                    lcms2::PixelFormat::RGB_FLT,
                    &dst,
                    lcms2::PixelFormat::RGB_FLT,
                    lcms2::Intent::RelativeColorimetric,
                )
                .unwrap();
                let mut mine = samples.clone();
                out.convert(&mut mine);
                let mut theirs = samples.clone();
                t.transform_in_place(&mut theirs);
                for (a, b) in mine.iter().zip(&theirs) {
                    for c in 0..3 {
                        if b[c] < 0.0 || b[c] > 1.0 {
                            continue; // fora da gama: nós cortamos
                        }
                        assert!(
                            (a[c] - b[c]).abs() < 2e-3,
                            "{space:?} linear={linear}: {a:?} vs {b:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn white_and_black_are_preserved_in_every_space() {
        for space in ColorSpace::BUILTIN {
            let out = OutputColor::builtin(space, false).unwrap();
            let mut px = vec![[1.0, 1.0, 1.0], [0.0, 0.0, 0.0]];
            out.convert(&mut px);
            for c in 0..3 {
                assert!((px[0][c] - 1.0).abs() < 2e-3, "{space:?} branco {:?}", px[0]);
                assert!(px[1][c].abs() < 1e-4, "{space:?} preto");
            }
            assert!(!out.description.is_empty());
            assert_eq!(inspect_icc(out.icc()).unwrap(), out.description);
        }
    }

    #[test]
    fn curves_roundtrip() {
        for c in [Curve::Srgb, Curve::Gamma(2.2), Curve::Gamma(1.8), Curve::Bt709, Curve::Linear] {
            for i in 0..=100 {
                let x = i as f32 / 100.0;
                assert!((c.decode(c.encode(x)) - x).abs() < 1e-4, "{c:?} {x}");
            }
        }
    }

    /// Perfil “do laboratório” (aqui um Adobe RGB gerado à parte): a LittleCMS converte e o
    /// resultado coincide com a nossa conversão; perfis não RGB são recusados.
    #[test]
    fn custom_icc_profile() {
        let lab = builtin_profile(ColorSpace::AdobeRgb, false).unwrap().icc().unwrap();
        let custom = OutputColor::custom(&lab, Intent::RelativeColorimetric).unwrap();
        let builtin = OutputColor::builtin(ColorSpace::AdobeRgb, false).unwrap();
        let mut a = vec![[0.2, 0.4, 0.1], [0.6, 0.5, 0.4]];
        let mut b = a.clone();
        custom.convert(&mut a);
        builtin.convert(&mut b);
        for (x, y) in a.iter().zip(&b) {
            for c in 0..3 {
                assert!((x[c] - y[c]).abs() < 3e-3, "{x:?} {y:?}");
            }
        }
        assert_eq!(custom.icc(), &lab[..]);
        let gray = lcms2::Profile::new_gray(
            &lcms2::CIExyY { x: D65.0, y: D65.1, Y: 1.0 },
            &lcms2::ToneCurve::new(2.2),
        )
        .unwrap()
        .icc()
        .unwrap();
        assert!(inspect_icc(&gray).is_err());
        assert!(inspect_icc(b"lixo").is_err());
    }
}
