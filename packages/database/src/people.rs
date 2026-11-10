//! Pessoas: agrupamento de rostos (automático) e identificação pelo utilizador
//! (nome + papel: noiva, noivo, pais, irmãos, padrinhos, madrinhas…).
//!
//! Dados biométricos (identidade facial) ficam só neste catálogo local.

use crate::analysis::blob_to_f32s_pub;
use crate::{db_err, now_unix, Catalog};
use momento_core_types::{AssetId, CoreError, CoreResult};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Papéis disponíveis (a interface traduz).
pub const ROLES: &[&str] = &[
    "bride",
    "groom",
    "bride_mother",
    "bride_father",
    "groom_mother",
    "groom_father",
    "sibling",
    "best_man",
    "maid_of_honor",
    "important",
    "other",
];

pub const SOURCE_AUTO: i64 = 0;
pub const SOURCE_USER: i64 = 1;
pub const SOURCE_EXCLUDED: i64 = 2;

/// Rosto com identidade, para o agrupamento.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceIdentity {
    pub face_id: i64,
    pub asset: AssetId,
    pub identity: Vec<f32>,
    pub px: f32,
    pub sharpness: f32,
    pub presence: f32,
    pub person_id: Option<i64>,
    pub source: i64,
}

/// Destino de um rosto no resultado do agrupamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterRef {
    Existing(i64),
    New(usize),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceRef {
    pub face_id: i64,
    pub asset: AssetId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 0 automático, 1 confirmado.
    pub source: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonRow {
    pub id: i64,
    pub name: Option<String>,
    pub role: Option<String>,
    pub hidden: bool,
    pub face_count: u64,
    pub photo_count: u64,
    /// Rosto representativo (o maior confirmado, ou o maior).
    pub cover: Option<FaceRef>,
}

fn validate_role(role: Option<&str>) -> CoreResult<()> {
    match role {
        Some(r) if !ROLES.contains(&r) => Err(CoreError::Other(format!("papel inválido: {r}"))),
        _ => Ok(()),
    }
}

impl Catalog {
    pub fn face_identities(&self) -> CoreResult<Vec<FaceIdentity>> {
        let mut st = self
            .connection()
            .prepare(
                "SELECT id, asset_id, identity, px, sharpness, presence, person_id, person_source
                 FROM face WHERE identity IS NOT NULL ORDER BY id",
            )
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                Ok(FaceIdentity {
                    face_id: r.get(0)?,
                    asset: AssetId(r.get(1)?),
                    identity: blob_to_f32s_pub(&r.get::<_, Vec<u8>>(2)?),
                    px: r.get(3)?,
                    sharpness: r.get(4)?,
                    presence: r.get(5)?,
                    person_id: r.get(6)?,
                    source: r.get(7)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Aplica um agrupamento: apaga os grupos automáticos sem nome (sem rostos confirmados),
    /// limpa as atribuições automáticas e grava as novas. Pessoas com nome/papel e rostos
    /// confirmados ou excluídos pelo utilizador nunca são alterados.
    pub fn apply_clusters(
        &mut self,
        assignments: &[(i64, ClusterRef)],
        new_clusters: usize,
    ) -> CoreResult<()> {
        let now = now_unix();
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        tx.execute_batch(
            "UPDATE face SET person_id = NULL WHERE person_source = 0;
             DELETE FROM person WHERE name IS NULL AND role IS NULL AND hidden = 0
               AND id NOT IN (SELECT person_id FROM face WHERE person_id IS NOT NULL);",
        )
        .map_err(db_err)?;
        let mut ids = Vec::with_capacity(new_clusters);
        {
            let mut ins =
                tx.prepare("INSERT INTO person(created_at) VALUES(?1)").map_err(db_err)?;
            for _ in 0..new_clusters {
                ins.execute([now]).map_err(db_err)?;
                ids.push(tx.last_insert_rowid());
            }
            let mut upd = tx
                .prepare("UPDATE face SET person_id = ?1 WHERE id = ?2 AND person_source = 0")
                .map_err(db_err)?;
            for (face, c) in assignments {
                let pid = match c {
                    ClusterRef::Existing(p) => *p,
                    ClusterRef::New(i) => {
                        *ids.get(*i).ok_or_else(|| CoreError::Other("grupo inexistente".into()))?
                    }
                };
                upd.execute(params![pid, face]).map_err(db_err)?;
            }
        }
        // Grupos novos que ficaram sem rostos (não deve acontecer, mas não deixa lixo).
        tx.execute_batch(
            "DELETE FROM person WHERE name IS NULL AND role IS NULL AND hidden = 0
               AND id NOT IN (SELECT person_id FROM face WHERE person_id IS NOT NULL);",
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    pub fn list_persons(&self) -> CoreResult<Vec<PersonRow>> {
        let c = self.connection();
        let mut st = c
            .prepare(
                "SELECT p.id, p.name, p.role, p.hidden, count(f.id), count(DISTINCT f.asset_id)
                 FROM person p LEFT JOIN face f ON f.person_id = p.id
                 GROUP BY p.id HAVING count(f.id) > 0 OR p.name IS NOT NULL OR p.role IS NOT NULL",
            )
            .map_err(db_err)?;
        let mut rows: Vec<PersonRow> = st
            .query_map([], |r| {
                Ok(PersonRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    role: r.get(2)?,
                    hidden: r.get::<_, i64>(3)? != 0,
                    face_count: r.get::<_, i64>(4)? as u64,
                    photo_count: r.get::<_, i64>(5)? as u64,
                    cover: None,
                })
            })
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;
        let mut cov = c
            .prepare(
                "SELECT id, asset_id, x, y, w, h, person_source FROM face WHERE person_id = ?1
                 ORDER BY person_source = 1 DESC, px * presence * (sharpness + 0.5) DESC LIMIT 1",
            )
            .map_err(db_err)?;
        for p in &mut rows {
            p.cover = cov.query_row([p.id], face_ref).optional().map_err(db_err)?;
        }
        // Com papel primeiro (pela ordem dos papéis), depois com nome, depois os maiores grupos.
        let role_rank = |r: &Option<String>| {
            r.as_deref().and_then(|r| ROLES.iter().position(|x| *x == r)).unwrap_or(99)
        };
        rows.sort_by(|a, b| {
            role_rank(&a.role)
                .cmp(&role_rank(&b.role))
                .then(b.name.is_some().cmp(&a.name.is_some()))
                .then(b.photo_count.cmp(&a.photo_count))
                .then(a.id.cmp(&b.id))
        });
        Ok(rows)
    }

    pub fn person_faces(&self, person: i64, limit: u32) -> CoreResult<Vec<FaceRef>> {
        let mut st = self
            .connection()
            .prepare(
                "SELECT id, asset_id, x, y, w, h, person_source FROM face WHERE person_id = ?1
                 ORDER BY person_source = 1 DESC, px * presence DESC LIMIT ?2",
            )
            .map_err(db_err)?;
        let rows = st.query_map(params![person, limit], face_ref).map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn create_person(&self, name: Option<&str>, role: Option<&str>) -> CoreResult<i64> {
        validate_role(role)?;
        self.connection()
            .execute(
                "INSERT INTO person(name, role, created_at) VALUES(?1, ?2, ?3)",
                params![name, role, now_unix()],
            )
            .map_err(db_err)?;
        Ok(self.connection().last_insert_rowid())
    }

    /// Nome/papel/oculta. Ao identificar uma pessoa, os seus rostos ficam confirmados.
    pub fn update_person(
        &mut self,
        id: i64,
        name: Option<&str>,
        role: Option<&str>,
        hidden: bool,
    ) -> CoreResult<()> {
        validate_role(role)?;
        let name = name.map(str::trim).filter(|n| !n.is_empty());
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        tx.execute(
            "UPDATE person SET name = ?1, role = ?2, hidden = ?3 WHERE id = ?4",
            params![name, role, hidden as i64, id],
        )
        .map_err(db_err)?;
        // Ocultar também é uma decisão: sem confirmar os rostos, o próximo reagrupamento
        // refazia a pessoa visível (Fase 15).
        if name.is_some() || role.is_some() || hidden {
            tx.execute(
                "UPDATE face SET person_source = 1 WHERE person_id = ?1 AND person_source = 0",
                [id],
            )
            .map_err(db_err)?;
        }
        tx.commit().map_err(db_err)
    }

    /// Junta `from` em `into` (os rostos ficam confirmados como `into`).
    pub fn merge_persons(&mut self, from: i64, into: i64) -> CoreResult<()> {
        if from == into {
            return Ok(());
        }
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        tx.execute(
            "UPDATE face SET person_id = ?1, person_source = 1 WHERE person_id = ?2",
            params![into, from],
        )
        .map_err(db_err)?;
        tx.execute(
            "UPDATE face SET person_source = 1 WHERE person_id = ?1 AND person_source = 0",
            [into],
        )
        .map_err(db_err)?;
        tx.execute(
            "UPDATE person SET name = coalesce(name, (SELECT name FROM person WHERE id = ?2)),
                               role = coalesce(role, (SELECT role FROM person WHERE id = ?2)) WHERE id = ?1",
            params![into, from],
        )
        .map_err(db_err)?;
        tx.execute("DELETE FROM person WHERE id = ?1", [from]).map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    /// Atribui rostos a uma pessoa (confirmado pelo utilizador).
    pub fn assign_faces(&self, faces: &[i64], person: i64) -> CoreResult<usize> {
        let mut st = self
            .connection()
            .prepare("UPDATE face SET person_id = ?1, person_source = 1 WHERE id = ?2")
            .map_err(db_err)?;
        let mut n = 0;
        for f in faces {
            n += st.execute(params![person, f]).map_err(db_err)?;
        }
        Ok(n)
    }

    /// “Não é esta pessoa”: o rosto sai do grupo e não volta a ser atribuído automaticamente.
    pub fn exclude_faces(&self, faces: &[i64]) -> CoreResult<usize> {
        let mut st = self
            .connection()
            .prepare("UPDATE face SET person_id = NULL, person_source = 2 WHERE id = ?1")
            .map_err(db_err)?;
        let mut n = 0;
        for f in faces {
            n += st.execute([f]).map_err(db_err)?;
        }
        Ok(n)
    }

    /// Pares (fotografia, pessoa) para as pesquisas (“noiva + mãe”…). Exclui pessoas ocultas.
    pub fn photo_persons(&self) -> CoreResult<Vec<(AssetId, i64)>> {
        let mut st = self
            .connection()
            .prepare(
                "SELECT DISTINCT f.asset_id, f.person_id FROM face f JOIN person p ON p.id = f.person_id
                 WHERE p.hidden = 0 ORDER BY f.asset_id",
            )
            .map_err(db_err)?;
        let rows = st.query_map([], |r| Ok((AssetId(r.get(0)?), r.get(1)?))).map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Papéis presentes em cada fotografia (para a classificação de momentos).
    pub fn photo_roles(&self) -> CoreResult<Vec<(AssetId, String)>> {
        let mut st = self
            .connection()
            .prepare(
                "SELECT DISTINCT f.asset_id, p.role FROM face f JOIN person p ON p.id = f.person_id
                 WHERE p.role IS NOT NULL ORDER BY f.asset_id",
            )
            .map_err(db_err)?;
        let rows = st.query_map([], |r| Ok((AssetId(r.get(0)?), r.get(1)?))).map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Apaga todos os dados de pessoas (identidades, grupos, nomes) deste projeto.
    pub fn clear_people(&mut self) -> CoreResult<()> {
        let tx = self.connection_mut().transaction().map_err(db_err)?;
        tx.execute_batch("UPDATE face SET identity = NULL, person_id = NULL, person_source = 0; DELETE FROM person;")
            .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }
}

fn face_ref(r: &rusqlite::Row) -> rusqlite::Result<FaceRef> {
    Ok(FaceRef {
        face_id: r.get(0)?,
        asset: AssetId(r.get(1)?),
        x: r.get(2)?,
        y: r.get(3)?,
        w: r.get(4)?,
        h: r.get(5)?,
        source: r.get(6)?,
    })
}
