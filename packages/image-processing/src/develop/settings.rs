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
    /// 0…100: preservação do detalhe na redução de ruído de luminância (alto = mais textura).
    pub noise_detail: f32,
    /// AI Denoise (FFDNet), 0 = desligado, 1…100 = intensidade. Calculado sobre a fonte
    /// (antes de todos os outros ajustes) a partir do nível de ruído medido.
    pub ai_denoise: f32,
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
            noise_detail: 50.0,
            ai_denoise: 0.0,
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
    /// Perspetiva, −100…+100 (±40°): rotação vertical (inclinação) e horizontal da câmara
    /// num modelo projetivo real — as linhas retas continuam retas.
    pub vertical: f32,
    pub horizontal: f32,
    /// Compensa o alongamento depois da perspetiva (−100…+100: mais larga / mais alta).
    pub aspect_adjust: f32,
    /// Escala depois da correção, % (100…200; o enquadramento nunca mostra zonas vazias).
    pub scale: f32,
    /// Deslocamento do enquadramento, −100…+100 (fração da área disponível).
    pub offset_x: f32,
    pub offset_y: f32,
    /// Distância focal equivalente a 35 mm usada no modelo projetivo (0 = 35 mm).
    pub focal35: f32,
    /// Modo Upright aplicado por último ("", "level", "vertical", "full", "auto", "guided").
    pub upright: String,
    /// Guias do Upright guiado: segmentos [x1, y1, x2, y2] na fotografia orientada sem
    /// perspetiva (0–1, depois das rotações de 90°/espelho).
    pub guides: Vec<[f32; 4]>,
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
            aspect_adjust: 0.0,
            scale: 100.0,
            offset_x: 0.0,
            offset_y: 0.0,
            focal35: 0.0,
            upright: String::new(),
            guides: Vec::new(),
        }
    }
}

/// Perfil de objetiva (base de dados Lensfun) já resolvido para esta fotografia: os
/// coeficientes interpolados para a focal e a abertura ficam na receita (render determinístico,
/// sem consultar a base de dados).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LensProfile {
    pub enabled: bool,
    /// Objetiva (e câmara) do perfil, para mostrar.
    pub name: String,
    /// Intensidade da correção da distorção e da vinheta, % (100 = perfil; 0–200).
    pub distortion: f32,
    pub vignetting: f32,
    /// Corrigir a aberração cromática lateral (TCA) do perfil.
    pub chromatic: bool,
    /// Modelo de distorção: 0 nenhum, 1 poly3 (k1), 2 poly5 (k1, k2), 3 ptlens (a, b, c).
    pub dist_model: u8,
    pub dist: [f32; 3],
    /// Raio na unidade da calibração (Hugin) por unidade de meia-diagonal desta imagem.
    pub dist_scale: f32,
    /// TCA: 0 nenhum, 1 poly3 [vr, vb, cr, cb, br, bb], 2 linear [kr, kb].
    pub tca_model: u8,
    pub tca: [f32; 6],
    /// Vinheta "pa": [k1, k2, k3] e escala do raio (meia-diagonal da calibração).
    pub vig: [f32; 3],
    pub vig_scale: f32,
}

impl Default for LensProfile {
    fn default() -> Self {
        LensProfile {
            enabled: false,
            name: String::new(),
            distortion: 100.0,
            vignetting: 100.0,
            chromatic: true,
            dist_model: 0,
            dist: [0.0; 3],
            dist_scale: 1.0,
            tca_model: 0,
            tca: [1.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            vig: [0.0; 3],
            vig_scale: 1.0,
        }
    }
}

impl LensProfile {
    /// Fator radial da distorção (raio da fonte / raio corrigido) no raio `r` (meia-diagonal = 1).
    #[inline]
    pub fn distortion_factor(&self, r: f32) -> f32 {
        if !self.enabled || self.dist_model == 0 || self.distortion == 0.0 {
            return 1.0;
        }
        let h = r * self.dist_scale;
        let [a, b, c] = self.dist;
        let p = match self.dist_model {
            1 => 1.0 - a + a * h * h,
            2 => 1.0 + a * h * h + b * h.powi(4),
            3 => a * h.powi(3) + b * h * h + c * h + (1.0 - a - b - c),
            _ => 1.0,
        };
        1.0 + (p - 1.0) * self.distortion.clamp(0.0, 200.0) / 100.0
    }

    /// Fatores radiais do vermelho e do azul em relação ao verde (TCA), no raio `r`.
    #[inline]
    pub fn tca_factors(&self, r: f32) -> (f32, f32) {
        if !self.enabled || !self.chromatic || self.tca_model == 0 {
            return (1.0, 1.0);
        }
        let [vr, vb, cr, cb, br, bb] = self.tca;
        if self.tca_model == 2 {
            return (vr, vb);
        }
        let h = r * self.dist_scale;
        (br * h * h + cr * h + vr, bb * h * h + cb * h + vb)
    }

    /// Ganho que corrige a vinheta no raio `r` (meia-diagonal desta imagem = 1).
    #[inline]
    pub fn vignetting_gain(&self, r: f32) -> f32 {
        if !self.enabled || self.vignetting == 0.0 || self.vig == [0.0; 3] {
            return 1.0;
        }
        let r2 = (r * self.vig_scale).powi(2);
        let [k1, k2, k3] = self.vig;
        let c = (1.0 + k1 * r2 + k2 * r2 * r2 + k3 * r2 * r2 * r2).max(0.05);
        c.powf(-self.vignetting.clamp(0.0, 200.0) / 100.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Correções do perfil da objetiva (aplicadas antes das manuais).
    pub profile: LensProfile,
}

impl Default for Lens {
    fn default() -> Self {
        Lens {
            distortion: 0.0,
            ca_red: 0.0,
            ca_blue: 0.0,
            vignetting: 0.0,
            vignetting_midpoint: 50.0,
            profile: LensProfile::default(),
        }
    }
}

/// Tonalização (color grading simples): cor nas sombras e nos realces.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Grading {
    /// Matiz (0–360°) e intensidade (0–100) nas sombras.
    pub shadows_hue: f32,
    pub shadows_sat: f32,
    /// Matiz e intensidade nos realces.
    pub highlights_hue: f32,
    pub highlights_sat: f32,
    /// −100 (mais sombras) … +100 (mais realces).
    pub balance: f32,
}

impl Default for Grading {
    fn default() -> Self {
        Grading {
            shadows_hue: 220.0,
            shadows_sat: 0.0,
            highlights_hue: 45.0,
            highlights_sat: 0.0,
            balance: 0.0,
        }
    }
}

/// Preto e branco com mistura por cor (luminância de cada cor na conversão, −100…+100).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BlackWhite {
    pub enabled: bool,
    /// Vermelho, laranja, amarelo, verde, água, azul, roxo, magenta.
    pub mix: [f32; 8],
}

/// LUT de cor (.cube) aplicada no fim, em sRGB. `id` = SHA-256 do ficheiro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LutRef {
    pub id: String,
    pub name: String,
    /// 0…100 %.
    pub amount: f32,
}

impl Default for LutRef {
    fn default() -> Self {
        LutRef { id: String::new(), name: String::new(), amount: 100.0 }
    }
}

/// Correção de olho vermelho: círculo em coordenadas normalizadas da fotografia orientada
/// (x pela largura, y pela altura; raio pelo lado maior).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RedEye {
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

/// Remoção de mancha (pó do sensor): o círculo (x, y, r) é substituído pela zona (sx, sy),
/// com transição suave. Mesmas coordenadas que [`RedEye`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Spot {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub sx: f32,
    pub sy: f32,
    /// Corrigir (heal): a zona copiada recebe a luz e a cor à volta do destino.
    /// `false` = clonar tal e qual.
    pub heal: bool,
    /// Transição da borda, 0–100 % do raio.
    pub feather: f32,
    /// Opacidade, 0–100.
    pub opacity: f32,
}

impl Default for Spot {
    fn default() -> Self {
        Spot {
            x: 0.0,
            y: 0.0,
            r: 0.0,
            sx: 0.0,
            sy: 0.0,
            heal: false,
            feather: 45.0,
            opacity: 100.0,
        }
    }
}

/// Remoção de objeto / clonagem com pincel: a zona pintada (traço) é substituída pela mesma
/// forma deslocada (`dx`, `dy`, coordenadas normalizadas).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Patch {
    pub stroke: BrushStroke,
    pub dx: f32,
    pub dy: f32,
    pub heal: bool,
    pub opacity: f32,
}

impl Default for Patch {
    fn default() -> Self {
        Patch { stroke: BrushStroke::default(), dx: 0.05, dy: 0.0, heal: true, opacity: 100.0 }
    }
}

/// Traço de pincel (coordenadas normalizadas da fotografia orientada; `size` = raio em
/// unidades do lado maior).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BrushStroke {
    pub points: Vec<[f32; 2]>,
    pub size: f32,
    /// 0–100 % do raio.
    pub feather: f32,
    /// 0–100: quanto cada passagem acrescenta.
    pub flow: f32,
    /// Apagar (pincel "menos").
    pub erase: bool,
}

impl Default for BrushStroke {
    fn default() -> Self {
        BrushStroke { points: vec![], size: 0.02, feather: 50.0, flow: 100.0, erase: false }
    }
}

/// Como uma parte da máscara se combina com as anteriores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MaskOp {
    #[default]
    Add,
    Subtract,
    Intersect,
}

/// Forma de uma parte da máscara.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MaskShape {
    Brush {
        strokes: Vec<BrushStroke>,
    },
    /// 100 % do lado de (x0, y0), 0 % a partir de (x1, y1).
    Linear {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    /// Elipse (raios normalizados pelo lado maior), 100 % no interior.
    Radial {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        /// Graus.
        angle: f32,
        /// 0–100 %.
        feather: f32,
    },
    /// Luminância (0–1, visualização) entre `min` e `max`.
    Luminance {
        min: f32,
        max: f32,
        /// Transição (0–1).
        smooth: f32,
    },
    /// Cor: matiz (graus) ± intervalo, com saturação mínima.
    Color {
        hue: f32,
        range: f32,
        min_sat: f32,
        smooth: f32,
    },
    /// Máscara calculada (IA local ou céu): imagem guardada no projeto (`masks/<raster>.png`).
    Raster {
        kind: String,
        raster: String,
        /// Desfocagem da borda, 0–100.
        feather: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MaskComponent {
    pub op: MaskOp,
    pub invert: bool,
    pub shape: MaskShape,
}

impl Default for MaskComponent {
    fn default() -> Self {
        MaskComponent {
            op: MaskOp::Add,
            invert: false,
            shape: MaskShape::Brush { strokes: vec![] },
        }
    }
}

/// Ajustes dentro de uma máscara (−100…+100; exposição em EV).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LocalAdjust {
    pub exposure: f32,
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub temperature: f32,
    pub tint: f32,
    pub saturation: f32,
    pub texture: f32,
    pub clarity: f32,
    pub sharpness: f32,
    /// Redução de ruído local (0…100; negativo não tem efeito).
    pub noise: f32,
}

/// Ajuste local: máscara (componentes) + ajustes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LocalMask {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub invert: bool,
    /// Densidade (opacidade da máscara), 0–100.
    pub density: f32,
    pub components: Vec<MaskComponent>,
    pub adjust: LocalAdjust,
}

impl Default for LocalMask {
    fn default() -> Self {
        LocalMask {
            id: String::new(),
            name: String::new(),
            enabled: true,
            invert: false,
            density: 100.0,
            components: vec![],
            adjust: LocalAdjust::default(),
        }
    }
}

/// Regiões de um rosto (pontos da malha facial, coordenadas normalizadas).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FaceRegions {
    pub oval: Vec<[f32; 2]>,
    pub left_eye: Vec<[f32; 2]>,
    pub right_eye: Vec<[f32; 2]>,
    pub left_brow: Vec<[f32; 2]>,
    pub right_brow: Vec<[f32; 2]>,
    pub lips_outer: Vec<[f32; 2]>,
    pub lips_inner: Vec<[f32; 2]>,
    pub under_left: Vec<[f32; 2]>,
    pub under_right: Vec<[f32; 2]>,
    /// Íris: x, y, raio (lado maior).
    pub left_iris: [f32; 3],
    pub right_iris: [f32; 3],
}

/// Retrato (subtil): quantidades 0–100, aplicadas só nas regiões de cada rosto.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Portrait {
    pub skin: f32,
    pub dark_circles: f32,
    pub shine: f32,
    pub teeth: f32,
    pub eyes: f32,
    pub faces: Vec<FaceRegions>,
}

/// Retoque básico (não destrutivo): olhos vermelhos e manchas do sensor.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Retouch {
    pub red_eyes: Vec<RedEye>,
    pub spots: Vec<Spot>,
    /// Remoção de objetos / clonagem com pincel.
    pub patches: Vec<Patch>,
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
    pub retouch: Retouch,
    pub grading: Grading,
    pub bw: BlackWhite,
    pub lut: LutRef,
    /// Ajustes locais (máscaras).
    pub masks: Vec<LocalMask>,
    pub portrait: Portrait,
}

/// Nomes dos blocos (copiar/colar escolhe blocos).
pub const BLOCKS: [&str; 15] = [
    "light", "wb", "color", "hsl", "curve", "detail", "effects", "geometry", "lens", "retouch",
    "grading", "bw", "lut", "masks", "portrait",
];

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
            retouch: get(r, "retouch"),
            grading: get(r, "grading"),
            bw: get(r, "bw"),
            lut: get(r, "lut"),
            masks: get(r, "masks"),
            portrait: get(r, "portrait"),
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
        put(&mut r, "retouch", &self.retouch);
        put(&mut r, "grading", &self.grading);
        put(&mut r, "bw", &self.bw);
        put(&mut r, "lut", &self.lut);
        put(&mut r, "masks", &self.masks);
        put(&mut r, "portrait", &self.portrait);
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
