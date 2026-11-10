//! Especificação física de um álbum e presets de laboratório.
//!
//! Nada é universal: cada laboratório define sangria, zona segura, dobra, resolução e perfil.
//! Os valores iniciais são só um ponto de partida e o álbum mostra um alerta até a
//! especificação ser confirmada (preset do laboratório aplicado ou confirmação explícita).

use momento_color_management::{ColorSpace, Intent};
use serde::{Deserialize, Serialize};

/// Como o laboratório quer os ficheiros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportUnit {
    /// Uma imagem por página dupla (lay-flat).
    #[default]
    Spreads,
    /// Uma imagem por página (com sangria nos 4 lados).
    Pages,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumSpec {
    /// Formato de partida (ex.: "30x30"); "custom" se as medidas foram mudadas.
    pub format: String,
    /// Página (corte final), em mm.
    pub width_mm: f32,
    pub height_mm: f32,
    /// Páginas interiores (par).
    pub pages: u32,
    pub dpi: u32,
    /// Sangria para lá do corte, em cada lado.
    pub bleed_mm: f32,
    /// Margem de segurança para dentro do corte.
    pub safe_mm: f32,
    /// Largura total da zona da dobra (centrada no meio do spread).
    pub gutter_mm: f32,
    pub color: ColorSpace,
    pub icc_path: String,
    pub intent: Intent,
    /// Laboratório (nome do preset aplicado).
    pub lab: String,
    /// Medidas confirmadas com o laboratório.
    pub confirmed: bool,
    pub export_unit: ExportUnit,
    /// Limites do laboratório (0 = sem limite).
    pub min_pages: u32,
    pub max_pages: u32,
    /// Cor de fundo (sRGB #rrggbb).
    pub background: String,
}

impl Default for AlbumSpec {
    fn default() -> Self {
        AlbumSpec {
            format: "30x30".into(),
            width_mm: 300.0,
            height_mm: 300.0,
            pages: 40,
            dpi: 300,
            bleed_mm: 3.0,
            safe_mm: 10.0,
            gutter_mm: 16.0,
            color: ColorSpace::Srgb,
            icc_path: String::new(),
            intent: Intent::Perceptual,
            lab: String::new(),
            confirmed: false,
            export_unit: ExportUnit::Spreads,
            min_pages: 0,
            max_pages: 0,
            background: "#ffffff".into(),
        }
    }
}

/// Formatos iniciais (largura × altura da página, em cm). Os não quadrados podem ser
/// rodados (orientação).
pub const FORMATS: &[(&str, f32, f32)] = &[
    ("20x20", 20.0, 20.0),
    ("25x25", 25.0, 25.0),
    ("30x30", 30.0, 30.0),
    ("35x35", 35.0, 35.0),
    ("20x30", 30.0, 20.0),
    ("25x35", 35.0, 25.0),
    ("30x40", 40.0, 30.0),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    Square,
    Landscape,
    Portrait,
}

impl AlbumSpec {
    pub fn spreads(&self) -> usize {
        (self.pages.max(2) / 2) as usize
    }
    pub fn orientation(&self) -> Orientation {
        let (w, h) = (self.width_mm, self.height_mm);
        if (w - h).abs() < 0.5 {
            Orientation::Square
        } else if w > h {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        }
    }
    /// Aplica um formato (mantendo a orientação pedida).
    pub fn with_format(mut self, name: &str, orientation: Orientation) -> Self {
        if let Some((n, w, h)) = FORMATS.iter().find(|f| f.0 == name) {
            let (a, b) = (w * 10.0, h * 10.0);
            let (w, h) = match orientation {
                Orientation::Portrait => (a.min(b), a.max(b)),
                _ => (a.max(b), a.min(b)),
            };
            self.format = n.to_string();
            self.width_mm = w;
            self.height_mm = h;
        }
        self
    }
    /// Spread (página dupla) em mm, sem sangria.
    pub fn spread_mm(&self) -> (f32, f32) {
        (self.width_mm * 2.0, self.height_mm)
    }
    pub fn px_per_mm(&self) -> f32 {
        self.dpi as f32 / 25.4
    }
    /// Problemas da especificação (chaves i18n).
    pub fn problems(&self) -> Vec<&'static str> {
        let mut p = Vec::new();
        if !(50.0..=1000.0).contains(&self.width_mm) || !(50.0..=1000.0).contains(&self.height_mm) {
            p.push("album.problem.size");
        }
        if self.pages < 2 || self.pages % 2 == 1 {
            p.push("album.problem.pagesEven");
        }
        if self.min_pages > 0 && self.pages < self.min_pages {
            p.push("album.problem.pagesMin");
        }
        if self.max_pages > 0 && self.pages > self.max_pages {
            p.push("album.problem.pagesMax");
        }
        if !(72..=1200).contains(&self.dpi) {
            p.push("album.problem.dpi");
        }
        if self.bleed_mm < 0.0 || self.bleed_mm > 20.0 || self.safe_mm < 0.0 || self.gutter_mm < 0.0
        {
            p.push("album.problem.margins");
        }
        if self.color == ColorSpace::Icc && self.icc_path.trim().is_empty() {
            p.push("album.problem.icc");
        }
        p
    }
}

/// Preset de laboratório definido pelo utilizador (biblioteca global).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LabPreset {
    pub id: String,
    pub name: String,
    pub notes: String,
    pub dpi: u32,
    pub bleed_mm: f32,
    pub safe_mm: f32,
    pub gutter_mm: f32,
    pub color: Option<ColorSpace>,
    pub icc_path: String,
    pub intent: Intent,
    pub export_unit: ExportUnit,
    pub min_pages: u32,
    pub max_pages: u32,
}

impl LabPreset {
    /// Aplica o preset (as medidas da página mantêm-se: o formato é escolhido à parte).
    pub fn apply(&self, s: &mut AlbumSpec) {
        s.lab = self.name.clone();
        if self.dpi > 0 {
            s.dpi = self.dpi;
        }
        s.bleed_mm = self.bleed_mm;
        s.safe_mm = self.safe_mm;
        s.gutter_mm = self.gutter_mm;
        if let Some(c) = self.color {
            s.color = c;
            s.icc_path = self.icc_path.clone();
        }
        s.intent = self.intent;
        s.export_unit = self.export_unit;
        s.min_pages = self.min_pages;
        s.max_pages = self.max_pages;
        s.confirmed = true;
    }
    pub fn from_spec(name: &str, s: &AlbumSpec) -> Self {
        LabPreset {
            id: String::new(),
            name: name.into(),
            notes: String::new(),
            dpi: s.dpi,
            bleed_mm: s.bleed_mm,
            safe_mm: s.safe_mm,
            gutter_mm: s.gutter_mm,
            color: Some(s.color),
            icc_path: s.icc_path.clone(),
            intent: s.intent,
            export_unit: s.export_unit,
            min_pages: s.min_pages,
            max_pages: s.max_pages,
        }
    }
}
