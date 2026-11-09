//! # momento-presets
//!
//! Presets de edição:
//! - [`Component`]: partes independentes de uma receita que um preset pode conter (o
//!   utilizador escolhe quais ao criar o preset);
//! - [`apply`]: aplica um preset com **Amount** (0–150 %) por cima dos ajustes atuais;
//! - [`builtin`]: os presets internos (Natural Wedding, Film Warm, B&W Classic…);
//! - [`file`]: formato próprio `.momentopreset` (JSON, com as LUT incluídas);
//! - [`xmp`]: importação de presets XMP do Lightroom/ACR **aproximada**, com relatório do
//!   que foi interpretado e do que foi ignorado (os valores não reproduzem o mesmo aspeto,
//!   porque os motores são diferentes).

pub mod builtin;
pub mod file;
pub mod xmp;

use momento_image_processing::develop::curves::Monotone;
use momento_image_processing::develop::Settings;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Componentes que um preset pode incluir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Component {
    /// Exposição.
    Exposure,
    /// Contraste, realces, sombras, brancos, pretos.
    Tone,
    /// Temperatura e matiz.
    WhiteBalance,
    /// Vibração, saturação, clareza, textura, remover névoa.
    Presence,
    Hsl,
    /// Curva paramétrica e de pontos.
    Curve,
    /// Tonalização de sombras/realces.
    Grading,
    /// Preto e branco e mistura por cor.
    BlackWhite,
    /// Nitidez.
    Sharpening,
    /// Redução de ruído (sem o AI Denoise, que depende de cada fotografia).
    NoiseReduction,
    /// Vinheta e grão.
    Effects,
    /// Correções da objetiva.
    Lens,
    /// LUT .cube.
    Lut,
}

pub const ALL_COMPONENTS: [Component; 13] = [
    Component::Exposure,
    Component::Tone,
    Component::WhiteBalance,
    Component::Presence,
    Component::Hsl,
    Component::Curve,
    Component::Grading,
    Component::BlackWhite,
    Component::Sharpening,
    Component::NoiseReduction,
    Component::Effects,
    Component::Lens,
    Component::Lut,
];

/// Componentes de "aspeto" (o que muda o estilo sem mexer na exposição nem no balanço de
/// cada fotografia): o valor por omissão ao criar um preset.
pub const LOOK_COMPONENTS: [Component; 8] = [
    Component::Tone,
    Component::Presence,
    Component::Hsl,
    Component::Curve,
    Component::Grading,
    Component::BlackWhite,
    Component::Effects,
    Component::Lut,
];

impl Component {
    /// Campos da receita (bloco, campos em camelCase; vazio = bloco inteiro).
    fn paths(self) -> &'static [(&'static str, &'static [&'static str])] {
        match self {
            Component::Exposure => &[("light", &["exposure"])],
            Component::Tone => {
                &[("light", &["contrast", "highlights", "shadows", "whites", "blacks"])]
            }
            Component::WhiteBalance => &[("wb", &[])],
            Component::Presence => &[
                ("color", &["saturation", "vibrance"]),
                ("effects", &["clarity", "texture", "dehaze"]),
            ],
            Component::Hsl => &[("hsl", &[])],
            Component::Curve => &[("curve", &[])],
            Component::Grading => &[("grading", &[])],
            Component::BlackWhite => &[("bw", &[])],
            Component::Sharpening => &[(
                "detail",
                &["sharpenAmount", "sharpenRadius", "sharpenDetail", "sharpenMasking"],
            )],
            Component::NoiseReduction => {
                &[("detail", &["noiseLuminance", "noiseColor", "noiseDetail"])]
            }
            Component::Effects => &[(
                "effects",
                &[
                    "vignette",
                    "vignetteMidpoint",
                    "vignetteFeather",
                    "vignetteRoundness",
                    "grain",
                    "grainSize",
                    "grainRoughness",
                ],
            )],
            Component::Lens => &[("lens", &[])],
            Component::Lut => &[("lut", &[])],
        }
    }
}

/// Um preset: ajustes + que componentes aplica.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    /// Grupo (pasta) para organizar.
    #[serde(default)]
    pub group: String,
    pub components: Vec<Component>,
    pub settings: Settings,
    #[serde(default)]
    pub builtin: bool,
    /// Descrição curta (presets internos e importados).
    #[serde(default)]
    pub description: String,
}

fn to_value(s: &Settings) -> Value {
    serde_json::to_value(s).expect("Settings serializável")
}

/// Ajustes de um preset a partir dos ajustes atuais: só os componentes escolhidos, o resto
/// fica neutro (criar preset a partir da fotografia).
pub fn capture(current: &Settings, components: &[Component]) -> Settings {
    let neutral = to_value(&Settings::default());
    let mut out = neutral.clone();
    let cur = to_value(current);
    for c in components {
        for (block, fields) in c.paths() {
            if fields.is_empty() {
                out[*block] = cur[*block].clone();
            } else {
                for f in *fields {
                    out[*block][*f] = cur[*block][*f].clone();
                }
            }
        }
    }
    serde_json::from_value(out).unwrap_or_default()
}

/// Curva de pontos avaliada em `n` posições (vazia = identidade).
fn sample_curve(points: &Value, n: usize) -> Vec<f32> {
    let pts: Vec<[f32; 2]> = serde_json::from_value(points.clone()).unwrap_or_default();
    let m = Monotone::new(&pts);
    (0..n)
        .map(|i| {
            let x = i as f32 / (n - 1) as f32;
            m.as_ref().map(|m| m.eval(x)).unwrap_or(x)
        })
        .collect()
}

/// Mistura de um valor numérico: até 100 % interpola entre o atual e o do preset; acima,
/// exagera o preset a partir do neutro.
fn blend_num(base: f64, preset: f64, neutral: f64, a: f64) -> f64 {
    if a <= 1.0 {
        base + (preset - base) * a
    } else {
        preset + (preset - neutral) * (a - 1.0)
    }
}

fn blend(base: &Value, preset: &Value, neutral: &Value, a: f64, key: &str) -> Value {
    match (base, preset) {
        (Value::Number(b), Value::Number(p)) => {
            let n = neutral.as_f64().unwrap_or(0.0);
            let v = blend_num(b.as_f64().unwrap_or(0.0), p.as_f64().unwrap_or(0.0), n, a);
            serde_json::Number::from_f64(v).map(Value::Number).unwrap_or_else(|| preset.clone())
        }
        // Curvas de pontos: mistura das curvas avaliadas em 17 posições.
        (Value::Array(_), Value::Array(_)) if ["rgb", "red", "green", "blue"].contains(&key) => {
            if (a - 1.0).abs() < 1e-6 {
                return preset.clone();
            }
            let (yb, yp) = (sample_curve(base, 17), sample_curve(preset, 17));
            let pts: Vec<[f32; 2]> = (0..17)
                .map(|i| {
                    let x = i as f32 / 16.0;
                    let y = blend_num(yb[i] as f64, yp[i] as f64, x as f64, a) as f32;
                    [x, y.clamp(0.0, 1.0)]
                })
                .collect();
            if pts.iter().all(|p| (p[0] - p[1]).abs() < 1e-4) {
                Value::Array(vec![])
            } else {
                serde_json::to_value(pts).unwrap_or(Value::Array(vec![]))
            }
        }
        (Value::Array(bv), Value::Array(pv)) if bv.len() == pv.len() => {
            let nv = neutral.as_array().cloned().unwrap_or_default();
            Value::Array(
                bv.iter()
                    .zip(pv)
                    .enumerate()
                    .map(|(i, (b, p))| blend(b, p, nv.get(i).unwrap_or(&Value::Null), a, ""))
                    .collect(),
            )
        }
        (Value::Object(bo), Value::Object(po)) => {
            let mut m = Map::new();
            for (k, pv) in po {
                let bv = bo.get(k).unwrap_or(pv);
                m.insert(k.clone(), blend(bv, pv, &neutral[k.as_str()], a, k));
            }
            Value::Object(m)
        }
        // Booleanos e textos (P&B ligado, LUT): do preset a partir de 50 %.
        _ => {
            if a >= 0.5 {
                preset.clone()
            } else {
                base.clone()
            }
        }
    }
}

/// Aplica os componentes escolhidos de `preset` sobre `base`, com `amount` em % (0–150).
/// O resto da receita (recorte, retoque, AI Denoise, componentes não incluídos) fica igual.
pub fn apply(
    base: &Settings,
    preset: &Settings,
    components: &[Component],
    amount: f32,
) -> Settings {
    let a = (amount.clamp(0.0, 150.0) / 100.0) as f64;
    let neutral = to_value(&Settings::default());
    let mut out = to_value(base);
    let p = to_value(preset);
    for c in components {
        for (block, fields) in c.paths() {
            let keys: Vec<String> = if fields.is_empty() {
                p[*block].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default()
            } else {
                fields.iter().map(|f| f.to_string()).collect()
            };
            for k in keys {
                let v = blend(&out[*block][&k], &p[*block][&k], &neutral[*block][&k], a, &k);
                out[*block][&k] = v;
            }
        }
    }
    // A LUT: a quantidade escala com o Amount; a referência vem do preset.
    let mut s: Settings = serde_json::from_value(out).unwrap_or_else(|_| base.clone());
    if components.contains(&Component::Lut) && !preset.lut.id.is_empty() {
        s.lut.id = preset.lut.id.clone();
        s.lut.name = preset.lut.name.clone();
        s.lut.amount = (preset.lut.amount * a as f32).clamp(0.0, 100.0);
    }
    s
}

#[cfg(test)]
mod tests;
