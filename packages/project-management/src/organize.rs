//! Organização do casamento: pessoas (agrupamento de rostos), momentos e sincronização
//! de hora entre câmaras. Tudo recalculado a partir do catálogo, em segundos, sem
//! reanalisar fotografias; as decisões do utilizador nunca são alteradas.

use crate::analysis::{recompute_selection, SelectionSummary};
use momento_ai_analysis::clock::{self, ClockPhoto, OffsetSuggestion};
use momento_ai_analysis::moments::{self, Cues, FaceBrief, MomentLabel, MomentPhoto};
use momento_ai_analysis::people::{self, FaceInput, Target};
use momento_core_types::{AssetId, CoreResult};
use momento_database::people::ClusterRef;
use momento_database::Catalog;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeSummary {
    pub selection: SelectionSummary,
    pub persons: u64,
    pub classified: u64,
    pub unclassified: u64,
    pub duration_ms: u64,
}

/// Reagrupa os rostos (as pessoas identificadas e os rostos confirmados mantêm-se).
pub fn recluster_people(catalog: &mut Catalog) -> CoreResult<u64> {
    let faces: Vec<FaceInput> = catalog
        .face_identities()?
        .into_iter()
        .map(|f| FaceInput {
            id: f.face_id,
            asset: f.asset.0,
            identity: f.identity,
            px: f.px,
            presence: f.presence,
            sharpness: f.sharpness,
            person: f.person_id,
            source: f.source,
        })
        .collect();
    if faces.is_empty() {
        return Ok(0);
    }
    let c = people::cluster(&faces);
    let asg: Vec<(i64, ClusterRef)> = c
        .assignments
        .iter()
        .map(|(f, t)| {
            (
                *f,
                match t {
                    Target::Existing(p) => ClusterRef::Existing(*p),
                    Target::New(i) => ClusterRef::New(*i),
                },
            )
        })
        .collect();
    catalog.apply_clusters(&asg, c.new_clusters)?;
    Ok(catalog.list_persons()?.len() as u64)
}

fn label_of(kind: Option<&str>, id: i64) -> MomentLabel {
    match kind.and_then(|k| moments::KINDS.iter().find(|x| **x == k)) {
        Some(k) => MomentLabel::Kind(k),
        None => MomentLabel::Custom(id),
    }
}

/// Classifica momentos das fotografias analisadas sem decisão do utilizador.
pub fn classify_moments(catalog: &mut Catalog) -> CoreResult<(u64, u64)> {
    let list = catalog.list_moments()?;
    let kind_by_id: HashMap<i64, Option<String>> =
        list.iter().map(|m| (m.id, m.kind.clone())).collect();
    let id_by_kind: HashMap<&str, i64> =
        list.iter().filter_map(|m| m.kind.as_deref().map(|k| (k, m.id))).collect();
    let user: HashMap<i64, Option<MomentLabel>> = catalog
        .asset_moments()?
        .into_iter()
        .filter(|m| m.source == "user")
        .map(|m| {
            (
                m.asset.0,
                m.moment_id.map(|id| label_of(kind_by_id.get(&id).and_then(|k| k.as_deref()), id)),
            )
        })
        .collect();
    let scenes: HashMap<i64, Vec<(u16, f32)>> =
        catalog.scene_tops()?.into_iter().map(|(a, s)| (a.0, s)).collect();
    let mut roles: HashMap<i64, Vec<String>> = HashMap::new();
    for (a, r) in catalog.photo_roles()? {
        roles.entry(a.0).or_default().push(r);
    }
    let photos: Vec<MomentPhoto> = catalog
        .selection_rows()?
        .into_iter()
        .map(|r| {
            let id = r.asset.0;
            MomentPhoto {
                id,
                time_ms: r.capture_time.as_deref().and_then(crate::analysis::capture_ms),
                gps: r.gps,
                scene: scenes.get(&id).cloned().unwrap_or_default(),
                embedding: r.embedding,
                faces: r
                    .faces
                    .iter()
                    .map(|f| FaceBrief { x: f.x, y: f.y, w: f.w, h: f.h, px: f.px })
                    .collect(),
                roles: roles.remove(&id).unwrap_or_default(),
                brightness: r.analysis.brightness,
                iso: r.iso,
            }
        })
        .collect();
    let res = moments::classify(&photos, &user, &Cues::new());
    let mut rows = Vec::new();
    let mut unclassified = 0;
    for r in &res {
        let moment = match r.label {
            Some(MomentLabel::Kind(k)) => id_by_kind.get(k).copied(),
            Some(MomentLabel::Custom(id)) => Some(id),
            None => None,
        };
        match moment {
            Some(m) => rows.push((
                AssetId(r.id),
                m,
                r.confidence,
                serde_json::to_string(&r.reasons).unwrap_or_else(|_| "[]".into()),
            )),
            None => unclassified += 1,
        }
    }
    catalog.save_auto_moments(&rows)?;
    Ok((rows.len() as u64, unclassified))
}

/// Sugestões de correção de hora para cada câmara em relação à `reference`.
pub fn suggest_offsets(catalog: &Catalog, reference: &str) -> CoreResult<Vec<OffsetSuggestion>> {
    let emb: HashMap<i64, Vec<f32>> = catalog
        .selection_rows()?
        .into_iter()
        .filter_map(|r| r.embedding.map(|e| (r.asset.0, e)))
        .collect();
    let photos: Vec<ClockPhoto> = catalog
        .clock_samples()?
        .into_iter()
        .filter_map(|s| {
            Some(ClockPhoto {
                camera: s.camera,
                time_ms: crate::analysis::capture_ms(&s.capture_time)?,
                embedding: emb.get(&s.asset.0)?.clone(),
            })
        })
        .collect();
    Ok(clock::suggest(&photos, reference))
}

/// Recalcula tudo: seleção, pessoas e momentos. Erros são registados, não propagados
/// (uma parte falhada não impede as outras).
pub fn rebuild(catalog: &mut Catalog) -> OrganizeSummary {
    let t0 = Instant::now();
    let mut s = OrganizeSummary::default();
    match recompute_selection(catalog) {
        Ok(x) => s.selection = x,
        Err(e) => tracing::error!(erro = %e, "falha ao calcular a seleção"),
    }
    match recluster_people(catalog) {
        Ok(n) => s.persons = n,
        Err(e) => tracing::error!(erro = %e, "falha ao agrupar pessoas"),
    }
    match classify_moments(catalog) {
        Ok((c, u)) => (s.classified, s.unclassified) = (c, u),
        Err(e) => tracing::error!(erro = %e, "falha ao classificar momentos"),
    }
    s.duration_ms = t0.elapsed().as_millis() as u64;
    s
}

/// Só momentos (rápido): depois de o utilizador arrastar fotografias ou identificar pessoas.
pub fn refresh_moments(catalog: &mut Catalog) -> CoreResult<(u64, u64)> {
    classify_moments(catalog)
}
