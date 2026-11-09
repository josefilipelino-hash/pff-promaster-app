//! Parâmetros de edição (receita tipada). Todos neutros por omissão: uma receita vazia
//! reproduz o original. Guardados como blocos do `EditRecipe` (só os blocos alterados).

use momento_core_types::recipe::EditRecipe;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Light {
    /// EV, −5…+5.
    pub exposure: f32,
    /// −100…+100 (todos os seguintes).
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WhiteBalance {
    /// Relativo ao balanço da câmara, −100 (mais frio) … +100 (mais quente).
    pub temperature: f32,
    /// −100 (verde) … +100 (magenta).
    pub tint: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Color {
    pub saturation: f32,
    pub vibrance: f32,
}

/// HSL por cor: vermelho, laranja, amarelo, verde, água, azul, roxo, magenta.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Hsl {
    pub hue: [f32; 8],
    pub saturation: [f32; 8],
    pub luminance: [f32; 8],
}

pub const HSL_BANDS: [&str; 8] =
    ["red", "orange", "yellow", "green", "aqua", "blue", "purple", "magenta"];
/// Centro de cada banda (graus).
pub const HSL_CENTERS: [f32; 8] = [0.0, 30.0, 60.0, 120.0, 180.0, 225.0, 270.0, 315.0];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Curve {
    /// Curva paramétrica, −100…+100 por zona.
    pub highlights: f32,
    pub lights: f32,
    pub darks: f32,
    pub shadows: f32,
    /// Divisões entre zonas (0–100).
    pub split_shadows: f32,
    pub split_midtones: f32,
    pub split_highlights: f32,
    /// Curvas de pontos (x, y em 0–1). Vazia = identidade.
    pub rgb: Vec<[f32; 2]>,
    pub red: Vec<[f32; 2]>,
    pub green: Vec<[f32; 2]>,
    pub blue: Vec<[f32; 2]>,
}

impl Default for Curve {
    fn default() -> Self {
        Curve {
            highlights: 0.0,
            lights: 0.0,
            darks: 0.0,
            shadows: 0.0,
            split_shadows: 25.0,
            split_midtones: 50.0,
            split_highlights: 75.0,
            rgb: Vec::new(),
            red: Vec::new(),
            green: Vec::new(),
            blue: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Detail {
    /// 0…150.
    pub sharpen_amount: f32,
    /// Píxeis da resolução total, 0,5…3.
    pub sharpen_radius: f32,
    /// 0…100: quanto do detalhe fino é realçado (baixo = só arestas).
    pub sharpen_detail: f32,
    /// 0…100: protege as zonas lisas (máscara de arestas).
    pub sharpen_masking: f32,
    /// 0…100.
    pub noise_luminance: f32,
    pub noise_color: f32,
}

impl Default for Detail {
    fn default() -> Self {
        Detail {
            sharpen_amount: 0.0,
            sharpen_radius: 1.0,
            sharpen_detail: 25.0,
            sharpen_masking: 0.0,
            noise_luminance: 0.0,
            noise_color: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Effects {
    pub texture: f32,
    pub clarity: f32,
    pub dehaze: f32,
    /// Vinheta depois do recorte, −100…+100.
    pub vignette: f32,
    pub vignette_midpoint: f32,
    pub vignette_feather: f32,
    pub vignette_roundness: f32,
    /// Grão, 0…100.
    pub grain: f32,
    pub grain_size: f32,
    pub grain_roughness: f32,
}

impl Default for Effects {
    fn default() -> Self {
        Effects {
            texture: 0.0,
            clarity: 0.0,
            dehaze: 0.0,
            vignette: 0.0,
            vignette_midpoint: 50.0,
            vignette_feather: 50.0,
            vignette_roundness: 0.0,
            grain: 0.0,
            grain_size: 25.0,
            grain_roughness: 50.0,
        }
    }
}

/// Recorte normalizado (0–1) no enquadramento já rodado/endireitado.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Crop {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Default for Crop {
    fn default() -> Self {
        Crop { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Geometry {
    pub crop: Crop,
    /// Rotações de 90° no sentido horário (0–3).
    pub quarter_turns: u8,
    pub flip_horizontal: bool,
    /// Endireitar, graus (−45…+45, positivo = sentido horário).
    pub straighten: f32,
    /// Proporção do recorte: "free", "original", "1:1", "4:5", "3:2", "16:9"… (só interface).
    pub aspect: String,
    /// Perspetiva, −100…+100.
    pub vertical: f32,
    pub horizontal: f32,
}

impl Default for Geometry {
    fn default() -> Self {
        Geometry {
            crop: Crop::default(),
            quarter_turns: 0,
            flip_horizontal: false,
            straighten: 0.0,
            aspect: "original".into(),
            vertical: 0.0,
            horizontal: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Lens {
    /// Distorção, −100…+100 (positivo corrige barril).
    pub distortion: f32,
    /// Aberração cromática: vermelho/ciano e azul/amarelo, −100…+100.
    pub ca_red: f32,
    pub ca_blue: f32,
    /// Correção da vinheta da objetiva, −100…+100 (positivo clareia os cantos).
    pub vignetting: f32,
    pub vignetting_midpoint: f32,
}

impl Default for Lens {
    fn default() -> Self {
        Lens {
            distortion: 0.0,
            ca_red: 0.0,
            ca_blue: 0.0,
            vignetting: 0.0,
            vignetting_midpoint: 50.0,
        }
    }
}

/// Todos os ajustes de uma variante.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub light: Light,
    pub wb: WhiteBalance,
    pub color: Color,
    pub hsl: Hsl,
    pub curve: Curve,
    pub detail: Detail,
    pub effects: Effects,
    pub geometry: Geometry,
    pub lens: Lens,
}

/// Nomes dos blocos (copiar/colar escolhe blocos).
pub const BLOCKS: [&str; 9] =
    ["light", "wb", "color", "hsl", "curve", "detail", "effects", "geometry", "lens"];

fn put<T: Serialize + Default + PartialEq>(r: &mut EditRecipe, name: &str, v: &T) {
    if *v != T::default() {
        r.blocks.insert(name.into(), serde_json::to_value(v).expect("serializável"));
    } else {
        r.blocks.remove(name);
    }
}

fn get<T: DeserializeOwned + Default>(r: &EditRecipe, name: &str) -> T {
    r.blocks.get(name).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

impl Settings {
    pub fn from_recipe(r: &EditRecipe) -> Self {
        Settings {
            light: get(r, "light"),
            wb: get(r, "wb"),
            color: get(r, "color"),
            hsl: get(r, "hsl"),
            curve: get(r, "curve"),
            detail: get(r, "detail"),
            effects: get(r, "effects"),
            geometry: get(r, "geometry"),
            lens: get(r, "lens"),
        }
    }

    /// Receita com só os blocos diferentes do neutro (preserva campos desconhecidos de `base`).
    pub fn to_recipe(&self, base: &EditRecipe) -> EditRecipe {
        let mut r = base.clone();
        put(&mut r, "light", &self.light);
        put(&mut r, "wb", &self.wb);
        put(&mut r, "color", &self.color);
        put(&mut r, "hsl", &self.hsl);
        put(&mut r, "curve", &self.curve);
        put(&mut r, "detail", &self.detail);
        put(&mut r, "effects", &self.effects);
        put(&mut r, "geometry", &self.geometry);
        put(&mut r, "lens", &self.lens);
        r
    }

    pub fn is_identity(&self) -> bool {
        *self == Settings::default()
    }
}

/// Copia os blocos indicados de `from` para `into` (colar/sincronizar definições).
pub fn merge_blocks(into: &EditRecipe, from: &EditRecipe, blocks: &[String]) -> EditRecipe {
    let mut r = into.clone();
    for b in blocks {
        match from.blocks.get(b) {
            Some(v) => {
                r.blocks.insert(b.clone(), v.clone());
            }
            None => {
                r.blocks.remove(b);
            }
        }
    }
    r
}
