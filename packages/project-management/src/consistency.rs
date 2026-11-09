//! Consistência visual (Fase 7): características de luz de cada fotografia e grupos por
//! iluminação, câmara, cenário, horário, balanço de brancos e flash/natural.
//!
//! As características medem-se nas miniaturas já em cache (rápido, ~10 ms cada) e no
//! EXIF: luminosidade da cena (EV a ISO 100 pela abertura/velocidade/ISO), dominante de
//! cor residual (iluminante estimado depois do balanço da câmara), chave e flash.

use crate::paths::{CacheLevel, ProjectPaths};
use crate::session::ProjectSession;
use momento_core_types::AssetId;
use momento_core_types::{CoreError, CoreResult};
use momento_database::consistency::{scene_ev, ConsistencyItem, LookFeature, PendingLook};
use momento_image_codecs::CodecRegistry;
use momento_image_processing::develop::auto::estimate_illuminant;
use momento_image_processing::develop::color::{luma, neutralize};
use momento_image_processing::develop::Settings;
use momento_image_processing::develop::Source;
use momento_presets::Component;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::Arc;

/// Critério de agrupamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Criterion {
    /// Luz semelhante e contínua no tempo (iluminação + flash + balanço + horário).
    Combined,
    Lighting,
    Camera,
    Scene,
    Time,
    WhiteBalance,
    Flash,
}

/// Um grupo de fotografias com luz/condições semelhantes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookGroup {
    pub id: String,
    /// Código para a interface (ex.: `lighting.indoor`, `wb.warm`, `flash.on`).
    pub code: String,
    /// Etiquetas adicionais (códigos) — no critério combinado.
    pub tags: Vec<String>,
    /// Texto livre (câmara) ou vazio.
    pub detail: String,
    pub moment: Option<i64>,
    /// Hora da primeira e da última fotografia.
    pub start: Option<String>,
    pub end: Option<String>,
    pub ids: Vec<AssetId>,
}

/// Iluminação a partir do EV da cena (ou da chave quando não há EXIF).
pub fn lighting_code(f: &LookFeature) -> &'static str {
    match f.ev {
        Some(ev) if ev >= 12.0 => "lighting.bright",
        Some(ev) if ev >= 9.0 => "lighting.shade",
        Some(ev) if ev >= 6.0 => "lighting.indoor",
        Some(_) => "lighting.dark",
        None => "lighting.unknown",
    }
}

/// Dominante de cor: `warmth` é a temperatura que neutralizaria a imagem (negativo = a
/// luz é quente e precisa de arrefecer).
pub fn wb_code(f: &LookFeature) -> &'static str {
    match f.warmth {
        w if w <= -15.0 => "wb.warm",
        w if w <= -5.0 => "wb.slightlyWarm",
        w if w < 5.0 => "wb.neutral",
        w if w < 15.0 => "wb.slightlyCool",
        _ => "wb.cool",
    }
}

pub fn flash_code(f: &LookFeature) -> &'static str {
    match f.flash {
        Some(true) => "flash.on",
        Some(false) => "flash.off",
        None => "flash.unknown",
    }
}

/// Minutos desde uma origem arbitrária (para medir intervalos), a partir de `AAAA-MM-DDTHH:MM:SS`.
fn minutes(t: &str) -> Option<f64> {
    let (d, h) = t.split_once('T')?;
    let mut dp = d.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, dd) = (dp.next()??, dp.next()??, dp.next()??);
    let mut hp = h.split(':');
    let hh: f64 = hp.next()?.parse().ok()?;
    let mm: f64 = hp.next()?.parse().ok()?;
    let ss: f64 = hp.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    // Dias aproximados (só para diferenças entre fotografias do mesmo evento).
    let days = y * 372 + m * 31 + dd;
    Some(days as f64 * 1440.0 + hh * 60.0 + mm + ss / 60.0)
}

const GAP_MINUTES: f64 = 20.0;

fn finish(id: String, code: String, items: &[&ConsistencyItem]) -> LookGroup {
    LookGroup {
        id,
        code,
        tags: vec![],
        detail: String::new(),
        moment: None,
        start: items.iter().filter_map(|i| i.time.clone()).next(),
        end: items.iter().rev().find_map(|i| i.time.clone()),
        ids: items.iter().map(|i| i.id).collect(),
    }
}

/// Agrupa por um critério (as fotografias vêm por ordem cronológica corrigida).
pub fn group(items: &[ConsistencyItem], criterion: Criterion) -> Vec<LookGroup> {
    use std::collections::BTreeMap;
    let by_key = |key: &dyn Fn(&ConsistencyItem) -> String| -> Vec<LookGroup> {
        let mut m: BTreeMap<String, Vec<&ConsistencyItem>> = BTreeMap::new();
        for i in items {
            m.entry(key(i)).or_default().push(i);
        }
        m.into_iter().map(|(k, v)| finish(k.clone(), k, &v)).collect()
    };
    let feature_code = |f: fn(&LookFeature) -> &'static str, none: &'static str| {
        move |i: &ConsistencyItem| i.feature.as_ref().map(f).unwrap_or(none).to_string()
    };
    match criterion {
        Criterion::Lighting => by_key(&feature_code(lighting_code, "lighting.unknown")),
        Criterion::WhiteBalance => by_key(&feature_code(wb_code, "wb.unknown")),
        Criterion::Flash => by_key(&feature_code(flash_code, "flash.unknown")),
        Criterion::Camera => {
            let mut g = by_key(&|i| {
                if i.camera.is_empty() {
                    "camera.unknown".into()
                } else {
                    i.camera.clone()
                }
            });
            for x in &mut g {
                if x.code != "camera.unknown" {
                    x.detail = x.code.clone();
                    x.code = "camera".into();
                }
            }
            g
        }
        Criterion::Scene => {
            let mut g = by_key(&|i| {
                i.moment
                    .map(|m| format!("moment:{m}"))
                    .unwrap_or_else(|| "scene.unclassified".into())
            });
            for x in &mut g {
                if let Some(m) = x.code.strip_prefix("moment:") {
                    x.moment = m.parse().ok();
                    x.code = "scene.moment".into();
                }
            }
            g
        }
        Criterion::Time => {
            let mut out: Vec<LookGroup> = Vec::new();
            let mut cur: Vec<&ConsistencyItem> = Vec::new();
            let mut last: Option<f64> = None;
            for i in items {
                let t = i.time.as_deref().and_then(minutes);
                if let (Some(a), Some(b)) = (last, t) {
                    if b - a > GAP_MINUTES && !cur.is_empty() {
                        out.push(finish(format!("time:{}", out.len()), "time".into(), &cur));
                        cur.clear();
                    }
                }
                if t.is_some() {
                    last = t;
                }
                cur.push(i);
            }
            if !cur.is_empty() {
                out.push(finish(format!("time:{}", out.len()), "time".into(), &cur));
            }
            out
        }
        Criterion::Combined => {
            // Sequências contínuas com a mesma luz; as muito pequenas juntam-se à vizinha.
            let sig = |i: &ConsistencyItem| -> (String, String, String) {
                match &i.feature {
                    Some(f) => (lighting_code(f).into(), flash_code(f).into(), wb_code(f).into()),
                    None => {
                        ("lighting.unknown".into(), "flash.unknown".into(), "wb.unknown".into())
                    }
                }
            };
            type Sig = (String, String, String);
            // (fotografias, assinatura, começou depois de um intervalo de tempo)
            let mut runs: Vec<(Vec<&ConsistencyItem>, Sig, bool)> = Vec::new();
            let mut last: Option<f64> = None;
            for i in items {
                let t = i.time.as_deref().and_then(minutes);
                let s = sig(i);
                let gap = matches!((last, t), (Some(a), Some(b)) if b - a > GAP_MINUTES);
                match runs.last_mut() {
                    Some((v, rs, _)) if !gap && *rs == s => v.push(i),
                    _ => runs.push((vec![i], s, gap)),
                }
                if t.is_some() {
                    last = t;
                }
            }
            // 1) Exceções isoladas (1–2 fotografias, ex.: um flash) entre duas sequências
            //    iguais e contínuas: as três passam a uma só.
            let mut i = 1;
            while i + 1 < runs.len() {
                if runs[i].0.len() <= 2
                    && runs[i - 1].1 == runs[i + 1].1
                    && !runs[i].2
                    && !runs[i + 1].2
                {
                    let (mid, _, _) = runs.remove(i);
                    let (next, _, _) = runs.remove(i);
                    runs[i - 1].0.extend(mid);
                    runs[i - 1].0.extend(next);
                } else {
                    i += 1;
                }
            }
            // 2) Restos pequenos juntam-se à sequência anterior (sem atravessar intervalos).
            let mut merged: Vec<(Vec<&ConsistencyItem>, Sig, bool)> = Vec::new();
            for r in runs {
                match merged.last_mut() {
                    Some(prev) if r.0.len() <= 2 && !r.2 => prev.0.extend(r.0),
                    Some(prev) if prev.0.len() <= 2 && !r.2 => {
                        let mut v = std::mem::take(&mut prev.0);
                        v.extend(r.0);
                        *prev = (v, r.1, prev.2);
                    }
                    _ => merged.push(r),
                }
            }
            // O título é o da **maioria** das fotografias do grupo (depois de juntar
            // sequências pequenas, a primeira não representa necessariamente o grupo).
            let majority = |v: &[&ConsistencyItem], pick: fn(&Sig) -> &String| -> String {
                let mut count: BTreeMap<String, usize> = BTreeMap::new();
                for i in v {
                    *count.entry(pick(&sig(i)).clone()).or_default() += 1;
                }
                count.into_iter().max_by_key(|e| e.1).map(|e| e.0).unwrap_or_default()
            };
            merged
                .into_iter()
                .enumerate()
                .map(|(n, (v, _, _))| {
                    let l = majority(&v, |s| &s.0);
                    let mut g = finish(format!("combined:{n}"), l, &v);
                    g.tags = vec![majority(&v, |s| &s.1), majority(&v, |s| &s.2)];
                    g
                })
                .collect()
        }
    }
}

/// Mede as características de uma fotografia (miniatura em cache + EXIF do original).
fn measure_one(
    paths: &ProjectPaths,
    registry: &CodecRegistry,
    p: &PendingLook,
) -> Option<LookFeature> {
    // Miniatura em cache; se ainda não existir, a preview (gerada a pedido).
    let thumb = crate::previews::ensure(paths, registry, p.id, CacheLevel::Thumb)
        .or_else(|_| crate::previews::ensure(paths, registry, p.id, CacheLevel::Preview))
        .ok()?;
    let img = image::open(&thumb.path).ok()?.thumbnail(640, 640).to_rgb8();
    let src = Source::from_srgb(&img, img.width(), img.height());
    let (warmth, tint) =
        estimate_illuminant(src.level_for(320.0)).map(neutralize).unwrap_or((0.0, 0.0));
    let mut lum: Vec<f32> = img
        .pixels()
        .step_by(3)
        .map(|q| luma([q[0] as f32 / 255.0, q[1] as f32 / 255.0, q[2] as f32 / 255.0]))
        .collect();
    lum.sort_by(f32::total_cmp);
    let key = lum.get(lum.len() / 2).copied().unwrap_or(0.0);
    // EXIF no início do ficheiro (JPEG e RAW baseados em TIFF); sem leitura completa.
    let flash = std::fs::File::open(&p.path).ok().and_then(|f| {
        let mut buf = Vec::new();
        f.take(1 << 20).read_to_end(&mut buf).ok()?;
        momento_metadata::read_exif(&buf).flash
    });
    Some(LookFeature { ev: scene_ev(p.iso, p.exposure_time, p.f_number), warmth, tint, key, flash })
}

/// Mede todas as fotografias pendentes (em paralelo). Devolve quantas foram medidas.
pub fn measure_pending(
    paths: &ProjectPaths,
    registry: &CodecRegistry,
    pending: &[PendingLook],
) -> Vec<(AssetId, LookFeature)> {
    pending.par_iter().filter_map(|p| measure_one(paths, registry, p).map(|f| (p.id, f))).collect()
}

// ---------------------------------------------------------------- sessão

/// Etiqueta do histórico para um preset: `preset:<nome>`.
pub const PRESET_LABEL_PREFIX: &str = "preset:";

impl ProjectSession {
    /// Mede as fotografias ainda sem características de luz. Devolve quantas mediu.
    pub fn measure_consistency(&self) -> CoreResult<usize> {
        let cat = self.open_catalog()?;
        let pending = cat.pending_look_features()?;
        let rows = measure_pending(&self.paths, self.registry(), &pending);
        cat.save_look_features(&rows)?;
        Ok(rows.len())
    }

    /// Grupos de consistência (as fotografias sem características entram como "sem dados").
    pub fn consistency_groups(&self, criterion: Criterion) -> CoreResult<(Vec<LookGroup>, usize)> {
        let items = self.open_catalog()?.consistency_items()?;
        let missing = items.iter().filter(|i| i.feature.is_none()).count();
        Ok((group(&items, criterion), missing))
    }

    /// Aplica um preset (componentes escolhidos, Amount em %) a várias fotografias. Cada uma
    /// fica com uma entrada `preset:<nome>` no histórico. Devolve quantas mudaram.
    pub fn apply_preset(
        self: &Arc<Self>,
        keys: &[i64],
        name: &str,
        preset: &Settings,
        components: &[Component],
        amount: f32,
    ) -> CoreResult<usize> {
        if keys.is_empty() {
            return Err(CoreError::Other("nenhuma fotografia".into()));
        }
        if components.contains(&Component::Lut) && !preset.lut.id.is_empty() {
            self.copy_lut_to_project(&preset.lut.id)?;
        }
        let label = format!("{PRESET_LABEL_PREFIX}{name}");
        let mut changed = Vec::new();
        {
            let mut cat = self.open_catalog()?;
            for &k in keys {
                let (variant, _) = cat.resolve_key(k)?;
                let base_recipe = cat.recipe_of(variant)?;
                let base = Settings::from_recipe(&base_recipe);
                let s = momento_presets::apply(&base, preset, components, amount);
                let recipe = s.to_recipe(&base_recipe);
                if recipe != base_recipe {
                    cat.commit_edit(variant, &label, &recipe)?;
                    changed.push(k);
                }
            }
        }
        let n = changed.len();
        if n > 0 {
            self.after_edit(changed);
        }
        Ok(n)
    }

    /// Copia a LUT (de qualquer pasta registada) para `<projeto>/luts/`, para o projeto
    /// continuar a abrir noutro computador.
    pub fn copy_lut_to_project(&self, id: &str) -> CoreResult<()> {
        let dir = self.paths.root.join("luts");
        let dst = dir.join(format!("{id}.cube"));
        if dst.is_file() {
            return Ok(());
        }
        for p in crate::session::lut_dirs() {
            let src = p.join(format!("{id}.cube"));
            if src.is_file() {
                std::fs::create_dir_all(&dir).map_err(|e| CoreError::Other(e.to_string()))?;
                std::fs::copy(&src, &dst).map_err(|e| CoreError::Other(e.to_string()))?;
                return Ok(());
            }
        }
        Err(CoreError::Other(format!("ficheiro da LUT {id} não encontrado")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(
        n: i64,
        time: &str,
        ev: f32,
        warmth: f32,
        flash: Option<bool>,
        cam: &str,
        moment: Option<i64>,
    ) -> ConsistencyItem {
        ConsistencyItem {
            id: AssetId(n),
            file_name: format!("IMG_{n}.jpg"),
            camera: cam.into(),
            time: Some(time.into()),
            moment,
            feature: Some(LookFeature { ev: Some(ev), warmth, tint: 0.0, key: 0.4, flash }),
        }
    }

    #[test]
    fn groups_by_every_criterion() {
        // Cerimónia ao ar livre (sol), igreja (interior quente), festa (escuro, flash).
        let mut v = Vec::new();
        for i in 0..5 {
            v.push(item(
                i,
                &format!("2026-06-15T15:{:02}:00", i * 3),
                14.0,
                0.0,
                Some(false),
                "Canon EOS R5",
                Some(1),
            ));
        }
        for i in 5..10 {
            v.push(item(
                i,
                &format!("2026-06-15T17:{:02}:00", i * 2),
                7.0,
                -25.0,
                Some(false),
                "SONY ILCE-7M4",
                Some(2),
            ));
        }
        // Uma fotografia com flash a meio da igreja: não deve criar um grupo sozinha.
        v[7].feature.as_mut().unwrap().flash = Some(true);
        for i in 10..16 {
            v.push(item(
                i,
                &format!("2026-06-15T22:{:02}:00", i),
                3.0,
                -10.0,
                Some(true),
                "Canon EOS R5",
                Some(3),
            ));
        }
        let ids = |g: &LookGroup| g.ids.iter().map(|a| a.0).collect::<Vec<_>>();
        let lighting = group(&v, Criterion::Lighting);
        assert_eq!(lighting.len(), 3);
        assert_eq!(
            ids(lighting.iter().find(|g| g.code == "lighting.bright").unwrap()),
            [0, 1, 2, 3, 4]
        );
        let cams = group(&v, Criterion::Camera);
        assert_eq!(cams.len(), 2);
        assert!(
            cams.iter().all(|g| g.code == "camera")
                && cams.iter().any(|g| g.detail == "SONY ILCE-7M4")
        );
        let wb = group(&v, Criterion::WhiteBalance);
        assert!(wb.iter().any(|g| g.code == "wb.warm" && g.ids.len() == 5));
        let flash = group(&v, Criterion::Flash);
        assert_eq!(flash.iter().find(|g| g.code == "flash.on").unwrap().ids.len(), 7);
        let time = group(&v, Criterion::Time);
        assert_eq!(time.len(), 3, "intervalos > 20 min separam");
        assert_eq!(
            (time[0].start.as_deref(), time[0].end.as_deref()),
            (Some("2026-06-15T15:00:00"), Some("2026-06-15T15:12:00"))
        );
        let scene = group(&v, Criterion::Scene);
        assert_eq!(scene.len(), 3);
        assert!(scene.iter().all(|g| g.code == "scene.moment" && g.moment.is_some()));
        let comb = group(&v, Criterion::Combined);
        assert_eq!(comb.len(), 3, "{:?}", comb.iter().map(ids).collect::<Vec<_>>());
        assert_eq!(ids(&comb[1]), [5, 6, 7, 8, 9], "a fotografia com flash fica com as vizinhas");
        assert_eq!(comb[2].tags, ["flash.on", "wb.slightlyWarm"]);
        assert_eq!(comb[1].tags, ["flash.off", "wb.warm"], "maioria (4 sem flash, 1 com)");
        // Uma fotografia com EV conhecido seguida de várias sem EXIF: o título é o da maioria.
        let mut w = vec![item(0, "2026-06-15T15:00:00", 14.0, 0.0, Some(false), "Nikon", None)];
        for i in 1..5 {
            let mut x = item(i, "2026-06-15T15:01:00", 0.0, 0.0, None, "", None);
            x.feature.as_mut().unwrap().ev = None;
            w.push(x);
        }
        let g = group(&w, Criterion::Combined);
        assert_eq!((g.len(), g[0].code.as_str()), (1, "lighting.unknown"));
    }
}
