//! "Criar álbum automaticamente": seleção final → capítulos da narrativa → spreads.
//!
//! Abertura → Preparação → Cerimónia → Casal → Família e amigos → Receção → Festa →
//! Encerramento. Dentro de cada capítulo, ordem cronológica. Evita repetições (uma
//! fotografia por grupo de sequência e rajadas), páginas cheias de mais (no máximo 3 por
//! página por omissão), rostos na dobra (as panorâmicas só se não houver rosto na dobra,
//! senão vira página inteira) e fotografias sem resolução para o tamanho do slot.

use crate::layouts::{self, Template};
use crate::model::{effective_dpi, Album, Crop, Spread};
use crate::preflight::{faces_in_spread, PhotoInfo};
use crate::spec::AlbumSpec;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoOptions {
    /// Fotografias por página, no máximo.
    pub max_per_page: usize,
    pub margin_mm: f32,
    pub spacing_mm: f32,
    /// Usar o número de páginas da especificação (senão, as necessárias).
    pub keep_pages: bool,
}

impl Default for AutoOptions {
    fn default() -> Self {
        AutoOptions { max_per_page: 3, margin_mm: 12.0, spacing_mm: 4.0, keep_pages: true }
    }
}

const CHAPTERS: &[(&str, &[&str])] = &[
    (
        "preparacao",
        &["bride_prep", "groom_prep", "details", "dress", "shoes", "rings", "invitations"],
    ),
    (
        "cerimonia",
        &["decoration", "arrival", "ceremony", "entrance", "ring_exchange", "kiss", "exit"],
    ),
    ("casal", &["couple_session"]),
    ("familia", &["groups"]),
    ("rececao", &["cocktail", "reception", "speeches", "dinner", "cake"]),
    ("festa", &["first_dance", "party"]),
    ("encerramento", &["night", "closing"]),
];

fn chapter_of(kind: &str) -> Option<usize> {
    CHAPTERS.iter().position(|(_, ks)| ks.contains(&kind))
}

fn secs(t: &Option<String>) -> Option<i64> {
    let t = t.as_deref()?;
    let n = |r: std::ops::Range<usize>| t.get(r)?.parse::<i64>().ok();
    Some(
        (n(0..4)? * 372 + n(5..7)? * 31 + n(8..10)?) * 86_400
            + n(11..13)? * 3600
            + n(14..16)? * 60
            + n(17..19)?,
    )
}

/// Centro do recorte: os rostos (ou o centro).
fn crop_for(p: &PhotoInfo) -> Crop {
    if p.faces.is_empty() {
        return Crop::default();
    }
    let (mut x0, mut y0, mut x1, mut y1) = (1.0f32, 1.0f32, 0.0f32, 0.0f32);
    for f in &p.faces {
        x0 = x0.min(f[0]);
        y0 = y0.min(f[1]);
        x1 = x1.max(f[0] + f[2]);
        y1 = y1.max(f[1] + f[3]);
    }
    // Ligeiramente acima do centro dos rostos (cabeças não encostadas ao topo).
    Crop { zoom: 1.0, cx: (x0 + x1) / 2.0, cy: ((y0 + y1) / 2.0 + 0.08).min(0.95) }
}

/// Resultado com as decisões explicadas.
pub struct AutoResult {
    pub album: Album,
    pub used: usize,
    pub skipped_low_res: usize,
    pub skipped_duplicates: usize,
}

pub fn build(name: &str, spec: &AlbumSpec, photos: &[PhotoInfo], opt: &AutoOptions) -> AutoResult {
    // 1. Seleção final: favoritas; sem favoritas, as não rejeitadas.
    let picks: Vec<&PhotoInfo> = photos.iter().filter(|p| p.flag > 0).collect();
    let mut pool: Vec<&PhotoInfo> =
        if picks.is_empty() { photos.iter().filter(|p| p.flag >= 0).collect() } else { picks };
    // 2. Resolução: precisa de dar 80 % do DPI do laboratório numa fotografia de 1/3 de página.
    let min_side_px =
        (spec.width_mm.min(spec.height_mm) / 3.0 / 25.4 * spec.dpi as f32 * 0.8) as u32;
    let before = pool.len();
    pool.retain(|p| p.width.min(p.height) >= min_side_px);
    let skipped_low_res = before - pool.len();
    // 3. Repetições: a melhor de cada grupo de sequência e de cada rajada (< 3 s, mesma orientação).
    pool.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut seen_groups = HashSet::new();
    let mut kept: Vec<&PhotoInfo> = Vec::new();
    for p in pool {
        if let Some(g) = p.group {
            if !seen_groups.insert(g) {
                continue;
            }
        }
        let burst = kept.iter().any(|k| {
            (k.aspect() > 1.0) == (p.aspect() > 1.0)
                && matches!((secs(&k.time), secs(&p.time)), (Some(a), Some(b)) if (a - b).abs() < 3)
        });
        if !burst {
            kept.push(p);
        }
    }
    let skipped_duplicates = before - skipped_low_res - kept.len();
    // 4. Capítulos (sem momento: o da classificada mais próxima no tempo).
    let classified: Vec<(i64, usize)> = kept
        .iter()
        .filter_map(|p| Some((secs(&p.time)?, chapter_of(p.moment.as_deref()?)?)))
        .collect();
    let mut chapters: Vec<Vec<&PhotoInfo>> = vec![Vec::new(); CHAPTERS.len()];
    for p in &kept {
        let c = p.moment.as_deref().and_then(chapter_of).or_else(|| {
            let t = secs(&p.time)?;
            classified.iter().min_by_key(|(ct, _)| (ct - t).abs()).map(|x| x.1)
        });
        chapters[c.unwrap_or(4)].push(p);
    }
    for c in chapters.iter_mut() {
        c.sort_by(|a, b| a.time.cmp(&b.time).then(a.key.cmp(&b.key)));
    }
    // 5. Abertura: a melhor fotografia horizontal do casal (ou a melhor de todas).
    let opening = chapters[2]
        .iter()
        .chain(chapters[1].iter())
        .filter(|p| p.aspect() > 1.2)
        .max_by(|a, b| a.score.total_cmp(&b.score))
        .or_else(|| kept.first())
        .map(|p| p.key);
    if let Some(o) = opening {
        for c in chapters.iter_mut() {
            c.retain(|p| p.key != o);
        }
    }
    // 6. Orçamento de spreads: abertura + capítulos proporcionais (mínimo 1 se não vazio).
    let total_spreads = if opt.keep_pages { spec.spreads() } else { usize::MAX };
    let per_spread = opt.max_per_page.clamp(1, 6) * 2;
    let available = total_spreads.saturating_sub(opening.is_some() as usize).max(1);
    let counts: Vec<usize> = chapters.iter().map(|c| c.len()).collect();
    let need: usize = counts.iter().map(|n| n.div_ceil(per_spread)).sum();
    let mut budget: Vec<usize> = if opt.keep_pages && need > available {
        // Menos espaço do que fotografias: proporcional, ficam as mais relevantes.
        let total: usize = counts.iter().sum::<usize>().max(1);
        let mut b: Vec<usize> = counts
            .iter()
            .map(|&n| {
                if n == 0 {
                    0
                } else {
                    ((n * available) as f32 / total as f32).floor().max(1.0) as usize
                }
            })
            .collect();
        while b.iter().sum::<usize>() > available {
            let i = (0..b.len()).max_by_key(|&i| b[i]).unwrap();
            b[i] -= 1;
        }
        b
    } else {
        // Espaço de sobra: fotografias com mais respiração (até 2 por spread no mínimo).
        let mut b: Vec<usize> = counts.iter().map(|n| n.div_ceil(per_spread)).collect();
        let mut spare = if opt.keep_pages { available.saturating_sub(need) } else { 0 };
        while spare > 0 {
            let Some(i) = (0..b.len())
                .filter(|&i| b[i] > 0 && counts[i] > b[i] * 2)
                .max_by_key(|&i| counts[i] * 100 / b[i].max(1))
            else {
                break;
            };
            b[i] += 1;
            spare -= 1;
        }
        b
    };
    for (c, b) in chapters.iter_mut().zip(budget.iter_mut()) {
        let cap = *b * per_spread;
        if c.len() > cap {
            let mut best: Vec<&PhotoInfo> = c.clone();
            best.sort_by(|a, b| b.score.total_cmp(&a.score));
            let keep: HashSet<i64> = best.iter().take(cap).map(|p| p.key).collect();
            c.retain(|p| keep.contains(&p.key));
        }
    }
    // 7. Spreads.
    let by_key: HashMap<i64, &PhotoInfo> = photos.iter().map(|p| (p.key, p)).collect();
    let mut album = Album::new(name, spec.clone());
    album.spreads.clear();
    let mut notes = Vec::new();
    if let Some(o) = opening {
        album.spreads.push(make(spec, &[by_key[&o]], opt, true));
        notes.push("abertura:1".to_string());
    }
    for (ci, c) in chapters.iter().enumerate() {
        if c.is_empty() || budget[ci] == 0 {
            continue;
        }
        let n_spreads = budget[ci].min(c.len());
        // Distribuição equilibrada: a primeira do capítulo com menos fotografias (destaque).
        let base = c.len() / n_spreads;
        let extra = c.len() % n_spreads;
        let mut i = 0;
        for s in 0..n_spreads {
            let k = (base + usize::from(s >= n_spreads - extra)).max(1);
            let group: Vec<&PhotoInfo> = c[i..(i + k).min(c.len())].to_vec();
            i += k;
            album.spreads.push(make(spec, &group, opt, s == 0 && group.len() <= 2));
        }
        notes.push(format!("{}:{}", CHAPTERS[ci].0, n_spreads));
    }
    // Páginas: as da especificação (spreads vazios no fim) ou as necessárias.
    if opt.keep_pages {
        while album.spreads.len() < spec.spreads() {
            album.spreads.push(Spread {
                id: crate::model::new_id(),
                margin_mm: opt.margin_mm,
                spacing_mm: opt.spacing_mm,
                ..Default::default()
            });
        }
    } else {
        album.spec.pages = (album.spreads.len() * 2) as u32;
    }
    album.notes = notes;
    let used = album.photos().len();
    AutoResult { album, used, skipped_low_res, skipped_duplicates }
}

/// Escolhe o layout de um spread e acerta os recortes (rostos fora da dobra).
fn make(spec: &AlbumSpec, group: &[&PhotoInfo], opt: &AutoOptions, hero: bool) -> Spread {
    let photos: Vec<(i64, f32)> = group.iter().map(|p| (p.key, p.aspect())).collect();
    let n = photos.len();
    let options_owned = layouts::templates_for(n);
    let options: Vec<&str> = options_owned.iter().map(String::as_str).collect();
    let landscape = |p: &&PhotoInfo| p.aspect() > 1.0;
    let max = opt.max_per_page.max(1);
    // Ordem de preferência conforme o número e a orientação das fotografias.
    let mut prefs: Vec<&str> = match n {
        1 if hero && group[0].aspect() > 1.2 => vec!["P", "-|B"],
        1 => vec!["-|B", "-|F"],
        2 if group.iter().all(|p| !landscape(p)) => vec!["B|B", "F|F"],
        2 => vec!["F|F", "B|F"],
        3 => vec!["B|G2", "F|G2"],
        4 => vec!["G2|G2", "B|G3"],
        5 => vec!["G2|G3", "B|G4"],
        _ => options.clone(),
    };
    // Nunca mais de `max` por página.
    prefs.retain(|id| match Template::parse(id) {
        Some(Template::Pages(l, r)) => l.count() <= max && r.count() <= max,
        _ => true,
    });
    prefs.extend(options.iter().filter(|id| {
        matches!(Template::parse(id), Some(Template::Pages(l, r)) if l.count() <= max && r.count() <= max)
    }));
    for id in prefs {
        let Some(mut sp) = layouts::apply(spec, id, &photos, opt.margin_mm, opt.spacing_mm) else {
            continue;
        };
        let mut ok = true;
        for sl in sp.slots.iter_mut() {
            let Some(p) = group.iter().find(|p| Some(p.key) == sl.photo) else { continue };
            sl.crop = crop_for(p);
            // Resolução suficiente para este slot.
            if effective_dpi(p.width, p.height, sl) < spec.dpi as f32 * 0.8 {
                ok = false;
            }
            // Rostos na dobra: tentar afastar o recorte; senão, outro layout.
            if sl.x < spec.width_mm && sl.x + sl.w > spec.width_mm {
                let g =
                    (spec.width_mm - spec.gutter_mm / 2.0, spec.width_mm + spec.gutter_mm / 2.0);
                let on_fold = |sl: &crate::model::Slot| {
                    faces_in_spread(p, sl).iter().any(|f| f[0] < g.1 && f[0] + f[2] > g.0)
                };
                if on_fold(sl) {
                    let mut fixed = false;
                    for z in [1.0f32, 1.15, 1.3] {
                        for cx in [0.3f32, 0.7, 0.2, 0.8] {
                            sl.crop = Crop { zoom: z, cx, ..crop_for(p) };
                            if !on_fold(sl)
                                && effective_dpi(p.width, p.height, sl) >= spec.dpi as f32 * 0.8
                            {
                                fixed = true;
                                break;
                            }
                        }
                        if fixed {
                            break;
                        }
                    }
                    ok &= fixed;
                }
            }
        }
        if ok {
            return sp;
        }
    }
    // Último recurso: uma moldura por página (sem dobra, sem ampliar).
    let fallback = if n == 1 {
        "-|F".to_string()
    } else {
        options_owned.last().cloned().unwrap_or_else(|| "F|F".into())
    };
    let mut sp =
        layouts::apply(spec, &fallback, &photos, opt.margin_mm, opt.spacing_mm).unwrap_or_default();
    let target = spec.dpi as f32 * 0.8;
    for sl in sp.slots.iter_mut() {
        if let Some(p) = group.iter().find(|p| Some(p.key) == sl.photo) {
            sl.crop = crop_for(p);
            // Sem resolução para a moldura: reduzi-la (centrada) até ao tamanho que a
            // fotografia aguenta, em vez de imprimir ampliada.
            let dpi = effective_dpi(p.width, p.height, sl);
            if dpi < target {
                let f = dpi / target;
                let (cx, cy) = (sl.x + sl.w / 2.0, sl.y + sl.h / 2.0);
                sl.w *= f;
                sl.h *= f;
                sl.x = cx - sl.w / 2.0;
                sl.y = cy - sl.h / 2.0;
            }
        }
    }
    sp
}
