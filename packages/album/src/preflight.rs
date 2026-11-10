//! Verificação antes de enviar ao laboratório: resolução efetiva, zona segura, rostos na
//! dobra, rostos cortados pelo enquadramento, bordas perto do corte, slots vazios, repetições e especificação por confirmar.

use crate::model::{crop_window, effective_dpi, Album, Slot};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// O que se sabe de cada fotografia.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PhotoInfo {
    pub key: i64,
    /// Píxeis disponíveis (orientados, com o recorte da edição).
    pub width: u32,
    pub height: u32,
    /// "AAAA-MM-DDTHH:MM:SS".
    pub time: Option<String>,
    /// Tipo do momento (ex.: "ceremony") ou `None`.
    pub moment: Option<String>,
    pub flag: i8,
    pub stars: u8,
    /// Relevância 0–1 (análise; sem análise, pelas estrelas).
    pub score: f32,
    /// Rostos (x, y, w, h) normalizados na fotografia.
    pub faces: Vec<[f32; 4]>,
    /// Pessoas identificadas.
    pub persons: Vec<i64>,
    /// Grupo de sequência/duplicados da análise.
    pub group: Option<i64>,
}

impl PhotoInfo {
    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height.max(1) as f32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// Spread (0…); `None` = o álbum inteiro.
    pub spread: Option<usize>,
    pub slot: Option<String>,
    pub severity: Severity,
    /// Chave i18n.
    pub code: String,
    pub value: Option<f32>,
}

fn issue(
    spread: Option<usize>,
    slot: Option<&Slot>,
    severity: Severity,
    code: &str,
    value: Option<f32>,
) -> Issue {
    Issue { spread, slot: slot.map(|s| s.id.clone()), severity, code: code.into(), value }
}

/// Rostos de uma fotografia num slot, em mm do spread (só a parte visível).
pub fn faces_in_spread(info: &PhotoInfo, slot: &Slot) -> Vec<[f32; 4]> {
    let [u0, v0, u1, v1] =
        crop_window(info.width as f32, info.height as f32, slot.w, slot.h, &slot.crop);
    info.faces
        .iter()
        .filter_map(|f| {
            let (fx0, fy0, fx1, fy1) =
                (f[0].max(u0), f[1].max(v0), (f[0] + f[2]).min(u1), (f[1] + f[3]).min(v1));
            if fx1 <= fx0 || fy1 <= fy0 {
                return None;
            }
            let mx = |u: f32| slot.x + (u - u0) / (u1 - u0) * slot.w;
            let my = |v: f32| slot.y + (v - v0) / (v1 - v0) * slot.h;
            Some([mx(fx0), my(fy0), mx(fx1) - mx(fx0), my(fy1) - my(fy0)])
        })
        .collect()
}

/// Verifica o álbum. Resolução: erro abaixo de 50 % do DPI do laboratório, aviso abaixo de 80 %.
pub fn check(album: &Album, photos: &HashMap<i64, PhotoInfo>) -> Vec<Issue> {
    let s = &album.spec;
    let mut out = Vec::new();
    for p in s.problems() {
        out.push(issue(None, None, Severity::Error, p, None));
    }
    if !s.confirmed {
        out.push(issue(None, None, Severity::Warning, "album.issue.unconfirmed", None));
    }
    if album.spreads.len() * 2 != s.pages as usize {
        out.push(issue(
            None,
            None,
            Severity::Warning,
            "album.issue.pageCount",
            Some((album.spreads.len() * 2) as f32),
        ));
    }
    let (pw, ph, b, safe) = (s.width_mm, s.height_mm, s.bleed_mm, s.safe_mm);
    let (g0, g1) = (pw - s.gutter_mm / 2.0, pw + s.gutter_mm / 2.0);
    let mut used: HashMap<i64, usize> = HashMap::new();
    for (si, sp) in album.spreads.iter().enumerate() {
        let n = sp.slots.iter().filter(|x| x.photo.is_some()).count();
        if n > 8 {
            out.push(issue(
                Some(si),
                None,
                Severity::Warning,
                "album.issue.crowded",
                Some(n as f32),
            ));
        }
        for sl in &sp.slots {
            let Some(key) = sl.photo else {
                out.push(issue(Some(si), Some(sl), Severity::Warning, "album.issue.empty", None));
                continue;
            };
            *used.entry(key).or_default() += 1;
            let Some(info) = photos.get(&key) else {
                out.push(issue(Some(si), Some(sl), Severity::Error, "album.issue.missing", None));
                continue;
            };
            let dpi = effective_dpi(info.width, info.height, sl);
            if dpi < s.dpi as f32 * 0.5 {
                out.push(issue(
                    Some(si),
                    Some(sl),
                    Severity::Error,
                    "album.issue.lowDpi",
                    Some(dpi.round()),
                ));
            } else if dpi < s.dpi as f32 * 0.8 {
                out.push(issue(
                    Some(si),
                    Some(sl),
                    Severity::Warning,
                    "album.issue.lowDpi",
                    Some(dpi.round()),
                ));
            }
            // Rostos cortados pelo enquadramento do slot (mais de 15 % do rosto fora).
            let [u0, v0, u1, v1] =
                crop_window(info.width as f32, info.height as f32, sl.w, sl.h, &sl.crop);
            if info.faces.iter().any(|f| {
                let area = (f[2] * f[3]).max(1e-9);
                let vis = ((f[0] + f[2]).min(u1) - f[0].max(u0)).max(0.0)
                    * ((f[1] + f[3]).min(v1) - f[1].max(v0)).max(0.0);
                vis > 0.0 && vis / area < 0.85
            }) {
                out.push(issue(
                    Some(si),
                    Some(sl),
                    Severity::Warning,
                    "album.issue.faceCropped",
                    None,
                ));
            }
            // Rostos na dobra e fora da zona segura.
            let faces = faces_in_spread(info, sl);
            if faces.iter().any(|f| f[0] < g1 && f[0] + f[2] > g0) {
                out.push(issue(
                    Some(si),
                    Some(sl),
                    Severity::Error,
                    "album.issue.faceInGutter",
                    None,
                ));
            }
            if faces.iter().any(|f| {
                f[0] < safe
                    || f[1] < safe
                    || f[0] + f[2] > 2.0 * pw - safe
                    || f[1] + f[3] > ph - safe
            }) {
                out.push(issue(
                    Some(si),
                    Some(sl),
                    Severity::Warning,
                    "album.issue.faceUnsafe",
                    None,
                ));
            }
            // Bordas entre a sangria e a zona segura (nem a sangrar nem a salvo).
            let tol = 0.5;
            let edges = [
                sl.x > -b + tol && sl.x < safe,
                sl.y > -b + tol && sl.y < safe,
                sl.x + sl.w < 2.0 * pw + b - tol && sl.x + sl.w > 2.0 * pw - safe,
                sl.y + sl.h < ph + b - tol && sl.y + sl.h > ph - safe,
            ];
            if edges.iter().any(|e| *e) {
                out.push(issue(
                    Some(si),
                    Some(sl),
                    Severity::Warning,
                    "album.issue.edgeNearTrim",
                    None,
                ));
            }
        }
    }
    for (key, n) in used {
        if n > 1 {
            out.push(Issue {
                spread: None,
                slot: None,
                severity: Severity::Warning,
                code: "album.issue.repeated".into(),
                value: Some(key as f32),
            });
        }
    }
    out
}
