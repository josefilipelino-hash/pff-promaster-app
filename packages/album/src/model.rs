//! Álbum → spreads (páginas duplas) → slots. Coordenadas em mm no spread: a origem é o
//! canto superior esquerdo do **corte** da página esquerda; a sangria fica fora
//! (x < 0, x > 2·largura, …). A dobra está em x = largura.

use crate::spec::AlbumSpec;
use serde::{Deserialize, Serialize};

/// Recorte de uma fotografia num slot: zoom ≥ 1 sobre o maior recorte com a proporção
/// do slot, centrado em (cx, cy) (coordenadas normalizadas da fotografia).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Crop {
    pub zoom: f32,
    pub cx: f32,
    pub cy: f32,
}

impl Default for Crop {
    fn default() -> Self {
        Crop { zoom: 1.0, cx: 0.5, cy: 0.5 }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Slot {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub photo: Option<i64>,
    pub crop: Crop,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Spread {
    pub id: String,
    /// Layout aplicado (ex.: "B|G3"); "custom" depois de mexer à mão.
    pub template: String,
    pub margin_mm: f32,
    pub spacing_mm: f32,
    pub slots: Vec<Slot>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub spec: AlbumSpec,
    pub spreads: Vec<Spread>,
    /// Resumo da montagem automática (capítulos e decisões), para mostrar ao utilizador.
    pub notes: Vec<String>,
}

/// Identificador curto e aleatório.
pub fn new_id() -> String {
    let mut b = [0u8; 6];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Janela do recorte na fotografia: (u0, v0, u1, v1) normalizados.
pub fn crop_window(photo_w: f32, photo_h: f32, slot_w: f32, slot_h: f32, c: &Crop) -> [f32; 4] {
    let ap = photo_w / photo_h.max(1e-6);
    let a_s = slot_w / slot_h.max(1e-6);
    let (mut ww, mut wh) = if a_s > ap { (1.0, ap / a_s) } else { (a_s / ap, 1.0) };
    let z = c.zoom.clamp(1.0, 10.0);
    ww /= z;
    wh /= z;
    let cx = c.cx.clamp(ww / 2.0, 1.0 - ww / 2.0);
    let cy = c.cy.clamp(wh / 2.0, 1.0 - wh / 2.0);
    [cx - ww / 2.0, cy - wh / 2.0, cx + ww / 2.0, cy + wh / 2.0]
}

/// Resolução efetiva (DPI) de uma fotografia `pw`×`ph` px num slot.
pub fn effective_dpi(pw: u32, ph: u32, slot: &Slot) -> f32 {
    let [u0, v0, u1, v1] = crop_window(pw as f32, ph as f32, slot.w, slot.h, &slot.crop);
    let dx = (u1 - u0) * pw as f32 / (slot.w / 25.4);
    let dy = (v1 - v0) * ph as f32 / (slot.h / 25.4);
    dx.min(dy)
}

impl Album {
    pub fn new(name: &str, spec: AlbumSpec) -> Self {
        let n = spec.spreads();
        Album {
            id: new_id(),
            name: name.into(),
            spreads: (0..n)
                .map(|_| Spread {
                    id: new_id(),
                    template: String::new(),
                    margin_mm: 12.0,
                    spacing_mm: 4.0,
                    slots: vec![],
                })
                .collect(),
            spec,
            notes: vec![],
        }
    }

    /// Fotografias usadas (sem repetições, por ordem).
    pub fn photos(&self) -> Vec<i64> {
        let mut v = Vec::new();
        for s in &self.spreads {
            for sl in &s.slots {
                if let Some(p) = sl.photo {
                    if !v.contains(&p) {
                        v.push(p);
                    }
                }
            }
        }
        v
    }

    /// Ajusta o número de spreads às páginas da especificação (spreads a mais só se
    /// estiverem vazios).
    pub fn sync_pages(&mut self) {
        let n = self.spec.spreads();
        while self.spreads.len() < n {
            self.spreads.push(Spread {
                id: new_id(),
                margin_mm: 12.0,
                spacing_mm: 4.0,
                ..Default::default()
            });
        }
        while self.spreads.len() > n
            && self.spreads.last().is_some_and(|s| s.slots.iter().all(|x| x.photo.is_none()))
        {
            self.spreads.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_and_dpi() {
        // Fotografia 3:2 num slot quadrado: recorte central com 2/3 da largura.
        let w = crop_window(6000.0, 4000.0, 100.0, 100.0, &Crop::default());
        assert!((w[2] - w[0] - 2.0 / 3.0).abs() < 1e-4 && (w[3] - w[1] - 1.0).abs() < 1e-4);
        // Zoom 2 e centro encostado: fica dentro da fotografia.
        let w = crop_window(6000.0, 4000.0, 100.0, 100.0, &Crop { zoom: 2.0, cx: 0.0, cy: 1.0 });
        assert!(w[0] >= 0.0 && w[3] <= 1.0 + 1e-6);
        // 4000 px de altura num slot de 100 mm: 4000 / 3,937 in ≈ 1016 DPI.
        let s = Slot { w: 100.0, h: 100.0, ..Default::default() };
        assert!((effective_dpi(6000, 4000, &s) - 1016.0).abs() < 1.0);
    }
}
