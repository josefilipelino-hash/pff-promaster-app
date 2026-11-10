//! Layouts de spread para 1, 2, 3, 4, 5 e 6+ fotografias.
//!
//! Cada página pode ficar vazia (`-`), com uma fotografia a sangrar (`B`), uma fotografia
//! com margens (`F`) ou várias em linhas justificadas (`G2`…`G9`): as fotografias mantêm
//! a proporção (sem cortes) e a combinação de linhas que melhor enche a página é escolhida
//! automaticamente. `P` é uma panorâmica a sangrar no spread inteiro.

use crate::model::{new_id, Crop, Slot, Spread};
use crate::spec::AlbumSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Empty,
    Bleed,
    /// `n` fotografias em linhas justificadas, com margens (1 = moldura).
    Grid(usize),
}

impl Fill {
    pub fn count(self) -> usize {
        match self {
            Fill::Empty => 0,
            Fill::Bleed => 1,
            Fill::Grid(n) => n,
        }
    }
    fn parse(s: &str) -> Option<Fill> {
        match s {
            "-" => Some(Fill::Empty),
            "B" => Some(Fill::Bleed),
            "F" => Some(Fill::Grid(1)),
            g if g.starts_with('G') => {
                g[1..].parse().ok().filter(|n| (1..=9).contains(n)).map(Fill::Grid)
            }
            _ => None,
        }
    }
}

/// Layout: panorâmica ou (página esquerda, página direita).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    Pano,
    Pages(Fill, Fill),
}

impl Template {
    pub fn parse(id: &str) -> Option<Template> {
        if id == "P" {
            return Some(Template::Pano);
        }
        let (l, r) = id.split_once('|')?;
        Some(Template::Pages(Fill::parse(l)?, Fill::parse(r)?))
    }
    pub fn count(self) -> usize {
        match self {
            Template::Pano => 1,
            Template::Pages(l, r) => l.count() + r.count(),
        }
    }
}

/// Layouts disponíveis para `n` fotografias (o primeiro é o predefinido).
pub fn templates_for(n: usize) -> Vec<String> {
    let v: Vec<&str> = match n {
        0 => vec![],
        1 => vec!["P", "-|B", "B|-", "-|F", "F|-"],
        2 => vec!["B|B", "F|F", "B|F", "F|B", "-|G2", "G2|-"],
        3 => vec!["B|G2", "G2|B", "F|G2", "G2|F", "-|G3"],
        4 => vec!["B|G3", "G3|B", "G2|G2", "F|G3", "G3|F"],
        5 => vec!["B|G4", "G4|B", "G2|G3", "G3|G2"],
        6 => vec!["G3|G3", "B|G5", "G5|B", "G2|G4", "G4|G2"],
        7 => vec!["G3|G4", "G4|G3", "B|G6"],
        8 => vec!["G4|G4", "G3|G5", "G5|G3"],
        9 => vec!["G4|G5", "G5|G4"],
        _ => {
            // 10+: metade em cada página (máximo 9 por página).
            let (a, b) = (n / 2, n - n / 2);
            return [format!("G{a}|G{b}"), format!("G{b}|G{a}")]
                .into_iter()
                .filter(|_| b <= 9)
                .collect();
        }
    };
    v.into_iter().map(String::from).collect()
}

/// Linhas justificadas: escolhe a divisão em linhas (pela ordem) que mais enche a caixa.
/// Devolve retângulos (x, y, w, h) com a proporção exata de cada fotografia.
pub fn justify(aspects: &[f32], bx: f32, by: f32, bw: f32, bh: f32, spacing: f32) -> Vec<[f32; 4]> {
    let n = aspects.len();
    if n == 0 {
        return vec![];
    }
    let mut best: Option<(f32, Vec<[f32; 4]>)> = None;
    for mask in 0u32..(1 << (n - 1)) {
        // Cortes entre i e i+1 onde o bit i está ligado.
        let mut rows: Vec<Vec<usize>> = vec![vec![0]];
        for i in 1..n {
            if mask & (1 << (i - 1)) != 0 {
                rows.push(vec![]);
            }
            rows.last_mut().unwrap().push(i);
        }
        if rows.iter().any(|r| r.len() > 4) {
            continue;
        }
        let heights: Vec<f32> = rows
            .iter()
            .map(|r| {
                let sum: f32 = r.iter().map(|&i| aspects[i].max(0.1)).sum();
                (bw - spacing * (r.len() - 1) as f32) / sum
            })
            .collect();
        let gaps = spacing * (rows.len() - 1) as f32;
        let total = heights.iter().sum::<f32>() + gaps;
        // Reduz as fotografias (não o espaçamento) para caber na altura.
        let k = ((bh - gaps) / (total - gaps)).min(1.0);
        if k <= 0.0 {
            continue;
        }
        let used_h = (total - gaps) * k + gaps;
        let mut rects = Vec::new();
        let mut y = by + (bh - used_h) / 2.0;
        let mut area = 0.0;
        for (r, h) in rows.iter().zip(&heights) {
            let h = h * k;
            let row_w: f32 = r.iter().map(|&i| aspects[i].max(0.1) * h).sum::<f32>()
                + spacing * (r.len() - 1) as f32;
            let mut x = bx + (bw - row_w) / 2.0;
            for &i in r {
                let w = aspects[i].max(0.1) * h;
                rects.push([x, y, w, h]);
                area += w * h;
                x += w + spacing;
            }
            y += h + spacing;
        }
        // Preferir encher a página; penalizar fotografias minúsculas.
        let min_side = rects.iter().map(|r| r[2].min(r[3])).fold(f32::MAX, f32::min);
        let score = area / (bw * bh) + if min_side < bw.min(bh) * 0.2 { -0.3 } else { 0.0 };
        if best.as_ref().is_none_or(|b| score > b.0) {
            best = Some((score, rects));
        }
    }
    best.map(|b| b.1).unwrap_or_default()
}

fn slot(x: f32, y: f32, w: f32, h: f32, photo: Option<i64>) -> Slot {
    Slot { id: new_id(), x, y, w, h, photo, crop: Crop::default() }
}

/// Aplica um layout a um spread com as fotografias dadas (chave, proporção largura/altura).
/// Fotografias a mais ficam de fora; a menos deixam slots vazios.
pub fn apply(
    spec: &AlbumSpec,
    id: &str,
    photos: &[(i64, f32)],
    margin: f32,
    spacing: f32,
) -> Option<Spread> {
    let t = Template::parse(id)?;
    let (pw, ph, b) = (spec.width_mm, spec.height_mm, spec.bleed_mm);
    let mut slots = Vec::new();
    let mut it = photos.iter();
    match t {
        Template::Pano => {
            slots.push(slot(-b, -b, 2.0 * pw + 2.0 * b, ph + 2.0 * b, it.next().map(|p| p.0)));
        }
        Template::Pages(l, r) => {
            for (page, fill) in [(0usize, l), (1, r)] {
                let x0 = page as f32 * pw;
                match fill {
                    Fill::Empty => {}
                    Fill::Bleed => {
                        // Sangra nos três lados exteriores; o lado da dobra fica no meio.
                        let (x, w) = if page == 0 { (-b, pw + b) } else { (pw, pw + b) };
                        slots.push(slot(x, -b, w, ph + 2.0 * b, it.next().map(|p| p.0)));
                    }
                    Fill::Grid(n) => {
                        let chosen: Vec<(i64, f32)> = it.by_ref().take(n).copied().collect();
                        let mut aspects: Vec<f32> = chosen.iter().map(|p| p.1).collect();
                        // Slots vazios: proporção 3:2.
                        aspects.resize(n, 1.5);
                        // Margem do lado da dobra: metade da zona da dobra, no mínimo.
                        let inner = margin.max(spec.gutter_mm / 2.0);
                        let (mx0, mx1) = if page == 0 { (margin, inner) } else { (inner, margin) };
                        let rects = justify(
                            &aspects,
                            x0 + mx0,
                            margin,
                            pw - mx0 - mx1,
                            ph - 2.0 * margin,
                            spacing,
                        );
                        for (i, r) in rects.iter().enumerate() {
                            slots.push(slot(r[0], r[1], r[2], r[3], chosen.get(i).map(|p| p.0)));
                        }
                    }
                }
            }
        }
    }
    Some(Spread {
        id: new_id(),
        template: id.to_string(),
        margin_mm: margin,
        spacing_mm: spacing,
        slots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> AlbumSpec {
        AlbumSpec {
            width_mm: 300.0,
            height_mm: 300.0,
            bleed_mm: 3.0,
            gutter_mm: 16.0,
            ..Default::default()
        }
    }

    #[test]
    fn every_count_has_layouts_that_fit_the_page() {
        let s = spec();
        for n in 1..=12usize {
            let photos: Vec<(i64, f32)> =
                (0..n as i64).map(|k| (k, if k % 3 == 0 { 0.67 } else { 1.5 })).collect();
            for id in templates_for(n) {
                let id = id.as_str();
                let t = Template::parse(id).unwrap();
                assert_eq!(t.count(), n, "{id} para {n}");
                let sp = apply(&s, id, &photos, 12.0, 4.0).unwrap();
                assert_eq!(sp.slots.len(), n, "{id}");
                for sl in &sp.slots {
                    assert!(
                        sl.x >= -3.01
                            && sl.x + sl.w <= 603.01
                            && sl.y >= -3.01
                            && sl.y + sl.h <= 303.01,
                        "{id}: {sl:?}"
                    );
                    // Fora das panorâmicas, nenhum slot atravessa a dobra.
                    if id != "P" {
                        assert!(
                            sl.x + sl.w <= 300.01 || sl.x >= 299.99,
                            "{id} atravessa a dobra: {sl:?}"
                        );
                    }
                }
                // Sem sobreposições.
                for (i, a) in sp.slots.iter().enumerate() {
                    for b in &sp.slots[i + 1..] {
                        let ov = a.x < b.x + b.w - 0.01
                            && b.x < a.x + a.w - 0.01
                            && a.y < b.y + b.h - 0.01
                            && b.y < a.y + a.h - 0.01;
                        assert!(!ov, "{id}: sobreposição");
                    }
                }
            }
        }
    }

    #[test]
    fn grid_keeps_aspect_ratios_and_respects_margins() {
        let s = spec();
        let sp = apply(&s, "-|G3", &[(1, 1.5), (2, 0.6667), (3, 1.5)], 15.0, 5.0).unwrap();
        let photos = [1.5f32, 0.6667, 1.5];
        for (sl, a) in sp.slots.iter().zip(photos) {
            assert!((sl.w / sl.h - a).abs() < 0.01, "proporção mantida (sem cortes)");
            assert!(sl.x >= 300.0 + 8.0 - 0.01 && sl.x + sl.w <= 600.0 - 15.0 + 0.01);
            assert!(sl.y >= 15.0 - 0.01 && sl.y + sl.h <= 285.0 + 0.01);
        }
        let pano = apply(&s, "P", &[(9, 3.0)], 12.0, 4.0).unwrap();
        assert_eq!((pano.slots[0].x, pano.slots[0].w), (-3.0, 606.0), "panorâmica a sangrar");
        let bleed = apply(&s, "B|-", &[(9, 1.0)], 12.0, 4.0).unwrap();
        assert_eq!(
            (bleed.slots[0].x, bleed.slots[0].w),
            (-3.0, 303.0),
            "sangra para fora, não na dobra"
        );
    }
}
