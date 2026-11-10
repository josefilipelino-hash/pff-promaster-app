//! Momentos do casamento: lista predefinida (pela ordem habitual do dia) + momentos
//! personalizados; atribuição automática (com confiança) ou do utilizador (prevalece).

use crate::{db_err, Catalog};
use momento_core_types::{AssetId, CoreError, CoreResult};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Momentos predefinidos, pela ordem habitual de um casamento. A interface traduz a chave.
pub const MOMENT_KINDS: &[&str] = &[
    "bride_prep",
    "groom_prep",
    "details",
    "dress",
    "shoes",
    "rings",
    "invitations",
    "decoration",
    "arrival",
    "ceremony",
    "entrance",
    "ring_exchange",
    "kiss",
    "exit",
    "couple_session",
    "groups",
    "cocktail",
    "reception",
    "speeches",
    "dinner",
    "cake",
    "first_dance",
    "party",
    "night",
    "closing",
];

/// Classes de cena (ImageNet) mais prováveis de uma fotografia: (índice, probabilidade).
pub type SceneTop = (AssetId, Vec<(u16, f32)>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MomentRow {
    pub id: i64,
    /// Chave predefinida (`None` = momento personalizado).
    pub kind: Option<String>,
    pub name: Option<String>,
    pub sort: i64,
    pub custom: bool,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMoment {
    pub asset: AssetId,
    /// `None` = “Por classificar” escolhido pelo utilizador.
    pub moment_id: Option<i64>,
    pub confidence: Option<f32>,
    /// `auto` | `user`.
    pub source: String,
    /// JSON com as pistas usadas (`[{code, value?}]`).
    pub reasons: Option<String>,
}

impl Catalog {
    /// Garante os momentos predefinidos (idempotente).
    pub fn ensure_moments(&self) -> CoreResult<()> {
        let mut st = self
            .connection()
            .prepare("INSERT OR IGNORE INTO moment(kind, sort, custom) VALUES(?1, ?2, 0)")
            .map_err(db_err)?;
        for (i, k) in MOMENT_KINDS.iter().enumerate() {
            st.execute(params![k, (i as i64 + 1) * 10]).map_err(db_err)?;
        }
        Ok(())
    }

    pub fn list_moments(&self) -> CoreResult<Vec<MomentRow>> {
        self.ensure_moments()?;
        let mut st = self
            .connection()
            .prepare(
                "SELECT m.id, m.kind, m.name, m.sort, m.custom, count(am.asset_id)
                 FROM moment m LEFT JOIN asset_moment am ON am.moment_id = m.id
                 GROUP BY m.id ORDER BY m.sort, m.id",
            )
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                Ok(MomentRow {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    name: r.get(2)?,
                    sort: r.get(3)?,
                    custom: r.get::<_, i64>(4)? != 0,
                    count: r.get::<_, i64>(5)? as u64,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Cria um momento personalizado; `after` = momento depois do qual fica na lista.
    pub fn create_moment(&self, name: &str, after: Option<i64>) -> CoreResult<i64> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Other("o nome do momento é obrigatório".into()));
        }
        self.ensure_moments()?;
        let c = self.connection();
        let sort: i64 = match after {
            Some(id) => {
                c.query_row("SELECT sort FROM moment WHERE id = ?1", [id], |r| r.get(0))
                    .optional()
                    .map_err(db_err)?
                    .unwrap_or(0)
                    + 1
            }
            None => c
                .query_row("SELECT coalesce(max(sort), 0) + 10 FROM moment", [], |r| r.get(0))
                .map_err(db_err)?,
        };
        c.execute(
            "INSERT INTO moment(kind, name, sort, custom) VALUES(NULL, ?1, ?2, 1)",
            params![name, sort],
        )
        .map_err(db_err)?;
        Ok(c.last_insert_rowid())
    }

    pub fn rename_moment(&self, id: i64, name: &str) -> CoreResult<()> {
        let name = name.trim();
        self.connection()
            .execute(
                "UPDATE moment SET name = ?1 WHERE id = ?2",
                params![(!name.is_empty()).then_some(name), id],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// Só momentos personalizados. As fotografias voltam a ser classificadas automaticamente.
    pub fn delete_moment(&self, id: i64) -> CoreResult<()> {
        let n = self
            .connection()
            .execute("DELETE FROM moment WHERE id = ?1 AND custom = 1", [id])
            .map_err(db_err)?;
        if n == 0 {
            return Err(CoreError::Other("só é possível apagar momentos personalizados".into()));
        }
        Ok(())
    }

    /// Decisão do utilizador (arrastar para um momento). `None` = “Por classificar”.
    pub fn assign_moment(&self, ids: &[AssetId], moment: Option<i64>) -> CoreResult<usize> {
        let mut st = self
            .connection()
            .prepare(
                "INSERT INTO asset_moment(asset_id, moment_id, confidence, source, reasons) VALUES(?1, ?2, NULL, 'user', NULL)
                 ON CONFLICT(asset_id) DO UPDATE SET moment_id = excluded.moment_id, confidence = NULL,
                    source = 'user', reasons = NULL",
            )
            .map_err(db_err)?;
        let mut n = 0;
        for id in ids {
            n += st.execute(params![id.0, moment]).map_err(db_err)?;
        }
        Ok(n)
    }

    /// Devolve fotografias à classificação automática.
    pub fn clear_user_moments(&self, ids: &[AssetId]) -> CoreResult<usize> {
        let mut st = self
            .connection()
            .prepare("DELETE FROM asset_moment WHERE asset_id = ?1 AND source = 'user'")
            .map_err(db_err)?;
        let mut n = 0;
        for id in ids {
            n += st.execute([id.0]).map_err(db_err)?;
        }
        Ok(n)
    }

    /// Substitui as classificações automáticas (as do utilizador nunca são tocadas).
    pub fn save_auto_moments(&mut self, rows: &[(AssetId, i64, f32, String)]) -> CoreResult<()> {
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        tx.execute("DELETE FROM asset_moment WHERE source = 'auto'", []).map_err(db_err)?;
        {
            let mut ins = tx
                .prepare(
                    "INSERT OR IGNORE INTO asset_moment(asset_id, moment_id, confidence, source, reasons)
                     VALUES(?1, ?2, ?3, 'auto', ?4)",
                )
                .map_err(db_err)?;
            for (a, m, c, r) in rows {
                ins.execute(params![a.0, m, c, r]).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)
    }

    pub fn asset_moments(&self) -> CoreResult<Vec<AssetMoment>> {
        let mut st = self
            .connection()
            .prepare("SELECT asset_id, moment_id, confidence, source, reasons FROM asset_moment ORDER BY asset_id")
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                Ok(AssetMoment {
                    asset: AssetId(r.get(0)?),
                    moment_id: r.get(1)?,
                    confidence: r.get(2)?,
                    source: r.get(3)?,
                    reasons: r.get(4)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Classes de cena gravadas pela análise, por fotografia.
    pub fn scene_tops(&self) -> CoreResult<Vec<SceneTop>> {
        let mut st =
            self.connection().prepare("SELECT asset_id, top FROM scene").map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                Ok((AssetId(r.get(0)?), crate::analysis::blob_to_scene(&r.get::<_, Vec<u8>>(1)?)))
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{AnalysisResult, AnalysisRow, FaceRow};
    use crate::assets::tests::new_asset;
    use crate::assets::InsertResult;
    use crate::people::{ClusterRef, SOURCE_EXCLUDED};

    fn ids(c: &mut Catalog, n: u8) -> Vec<AssetId> {
        let items: Vec<_> = (1..=n).map(new_asset).collect();
        c.insert_assets(&items)
            .unwrap()
            .into_iter()
            .map(|r| match r {
                InsertResult::Inserted(id) => id,
                _ => panic!(),
            })
            .collect()
    }

    #[test]
    fn moments_user_wins_over_auto_and_custom_moments() {
        let mut c = Catalog::open_in_memory().unwrap();
        let a = ids(&mut c, 3);
        let ms = c.list_moments().unwrap();
        assert_eq!(ms.len(), MOMENT_KINDS.len());
        let kiss = ms.iter().find(|m| m.kind.as_deref() == Some("kiss")).unwrap().id;
        let cake = ms.iter().find(|m| m.kind.as_deref() == Some("cake")).unwrap().id;
        c.save_auto_moments(&[(a[0], kiss, 0.8, "[]".into()), (a[1], kiss, 0.7, "[]".into())])
            .unwrap();
        c.assign_moment(&[a[1]], Some(cake)).unwrap(); // arrastado pelo utilizador
        c.assign_moment(&[a[2]], None).unwrap(); // “Por classificar” explícito
                                                 // Nova classificação automática não toca nas decisões do utilizador.
        c.save_auto_moments(&[
            (a[0], kiss, 0.9, "[]".into()),
            (a[1], kiss, 0.9, "[]".into()),
            (a[2], kiss, 0.9, "[]".into()),
        ])
        .unwrap();
        let am = c.asset_moments().unwrap();
        let get = |id: AssetId| am.iter().find(|m| m.asset == id).unwrap().clone();
        assert_eq!(get(a[0]).moment_id, Some(kiss));
        assert_eq!((get(a[1]).moment_id, get(a[1]).source.as_str()), (Some(cake), "user"));
        assert_eq!(get(a[2]).moment_id, None);
        let custom = c.create_moment("Fogo de artifício", Some(cake)).unwrap();
        let ms = c.list_moments().unwrap();
        let pos = |id: i64| ms.iter().position(|m| m.id == id).unwrap();
        assert_eq!(pos(custom), pos(cake) + 1, "fica logo a seguir ao bolo");
        c.assign_moment(&[a[0]], Some(custom)).unwrap();
        assert!(c.delete_moment(kiss).is_err(), "predefinidos não se apagam");
        c.delete_moment(custom).unwrap();
        assert!(
            c.asset_moments().unwrap().iter().all(|m| m.asset != a[0]),
            "volta à classificação automática"
        );
        assert!(c.create_moment("  ", None).is_err());
    }

    #[test]
    fn people_clusters_names_merge_exclude_and_survive_reanalysis() {
        let mut c = Catalog::open_in_memory().unwrap();
        let a = ids(&mut c, 3);
        let face = |x: f32| FaceRow {
            x,
            y: 0.2,
            w: 0.1,
            h: 0.1,
            px: 120.0,
            presence: 0.99,
            identity: vec![x; 128],
            ..Default::default()
        };
        let res = |id: AssetId, xs: &[f32]| AnalysisResult {
            asset: id,
            row: AnalysisRow { width: 2048, height: 1365, ..Default::default() },
            faces: xs.iter().map(|&x| face(x)).collect(),
            scene: vec![(834, 0.4), (982, 0.2)],
            ..Default::default()
        };
        c.save_analyses(&[res(a[0], &[0.1, 0.5]), res(a[1], &[0.1]), res(a[2], &[0.5])], &[])
            .unwrap();
        assert_eq!(c.scene_tops().unwrap().len(), 3);
        let f = c.face_identities().unwrap();
        assert_eq!(f.len(), 4);
        // Dois grupos: rostos em x=0,1 (noiva) e x=0,5 (noivo).
        let asg: Vec<(i64, ClusterRef)> = f
            .iter()
            .map(|x| (x.face_id, ClusterRef::New(if x.identity[0] < 0.3 { 0 } else { 1 })))
            .collect();
        c.apply_clusters(&asg, 2).unwrap();
        let p = c.list_persons().unwrap();
        assert_eq!(p.len(), 2);
        assert!(p.iter().all(|x| x.photo_count == 2 && x.cover.is_some()));
        let bride = p.iter().find(|x| c.person_faces(x.id, 10).unwrap()[0].x < 0.3).unwrap().id;
        let groom = p.iter().find(|x| x.id != bride).unwrap().id;
        c.update_person(bride, Some("Ana"), Some("bride"), false).unwrap();
        assert!(c.update_person(groom, None, Some("rei"), false).is_err(), "papel inválido");
        c.update_person(groom, Some("Pedro"), Some("groom"), false).unwrap();
        assert_eq!(c.list_persons().unwrap()[0].role.as_deref(), Some("bride"), "noiva primeiro");
        // Noivos juntos = fotografia a[0].
        let pp = c.photo_persons().unwrap();
        let together: Vec<AssetId> = a
            .iter()
            .copied()
            .filter(|id| pp.contains(&(*id, bride)) && pp.contains(&(*id, groom)))
            .collect();
        assert_eq!(together, vec![a[0]]);
        assert_eq!(c.photo_roles().unwrap().len(), 4);
        // Novo agrupamento automático não desfaz pessoas identificadas.
        c.apply_clusters(&[], 0).unwrap();
        assert_eq!(c.list_persons().unwrap().iter().filter(|x| x.name.is_some()).count(), 2);
        assert_eq!(c.photo_persons().unwrap().len(), 4, "rostos confirmados mantêm-se");
        // “Não é esta pessoa” e reanálise: a decisão passa para o novo rosto na mesma posição.
        let groom_face_a2 =
            c.face_identities().unwrap().into_iter().find(|x| x.asset == a[2]).unwrap().face_id;
        c.exclude_faces(&[groom_face_a2]).unwrap();
        c.save_analyses(&[res(a[2], &[0.5])], &[]).unwrap();
        let after = c.face_identities().unwrap().into_iter().find(|x| x.asset == a[2]).unwrap();
        assert_eq!((after.source, after.person_id), (SOURCE_EXCLUDED, None));
        c.save_analyses(&[res(a[1], &[0.1])], &[]).unwrap();
        let again = c.face_identities().unwrap().into_iter().find(|x| x.asset == a[1]).unwrap();
        assert_eq!(again.person_id, Some(bride), "confirmação preservada");
        // Fase 15: reanálise sem modelos (nenhum rosto no resultado) não apaga a decisão; e
        // um rosto detetado noutra posição (IoU < 0,5) também não a apaga.
        c.save_analyses(&[AnalysisResult { faces: vec![], ..res(a[1], &[]) }], &[]).unwrap();
        let kept = c.face_identities().unwrap().into_iter().find(|x| x.asset == a[1]).unwrap();
        assert_eq!(kept.person_id, Some(bride), "sem modelos: decisão mantida");
        c.save_analyses(&[res(a[1], &[0.8])], &[]).unwrap();
        let faces_a1: Vec<_> =
            c.face_identities().unwrap().into_iter().filter(|x| x.asset == a[1]).collect();
        assert!(
            faces_a1.iter().any(|x| x.person_id == Some(bride)),
            "caixa deslocada: decisão mantida {faces_a1:?}"
        );
        assert_eq!(faces_a1.len(), 2, "rosto novo + decisão antiga");
        // Juntar e limpar.
        let extra = c.create_person(None, None).unwrap();
        let again = faces_a1.iter().find(|x| x.person_id == Some(bride)).unwrap();
        c.assign_faces(&[again.face_id], extra).unwrap();
        c.merge_persons(extra, bride).unwrap();
        assert_eq!(c.photo_persons().unwrap().iter().filter(|x| x.1 == bride).count(), 2);
        c.clear_people().unwrap();
        assert!(c.list_persons().unwrap().is_empty() && c.face_identities().unwrap().is_empty());
    }
}
