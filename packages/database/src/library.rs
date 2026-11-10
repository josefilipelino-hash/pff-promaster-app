//! Biblioteca global: lista de projetos recentes (pasta de suporte da aplicação).

use crate::{db_err, migrations, now_millis, open_with_migrations};
use momento_core_types::CoreResult;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    /// Caminho da pasta `.momento` do projeto.
    pub path: String,
    pub name: String,
    pub client_name: String,
    pub event_type: String,
    pub event_date: Option<String>,
    pub location: String,
    pub photo_count: u64,
    /// Milissegundos desde 1970.
    pub last_opened: i64,
    /// Calculado na leitura: a pasta do projeto existe (disco ligado).
    #[serde(default)]
    pub available: bool,
}

pub struct Library {
    conn: Connection,
}

impl Library {
    pub fn open(path: &Path) -> CoreResult<Self> {
        Ok(Library { conn: open_with_migrations(path, migrations::LIBRARY)? })
    }

    pub fn open_in_memory() -> CoreResult<Self> {
        let conn = Connection::open_in_memory().map_err(db_err)?;
        migrations::apply(&conn, migrations::LIBRARY)?;
        Ok(Library { conn })
    }

    /// Insere ou atualiza um projeto e marca-o como aberto agora.
    pub fn touch(&self, p: &RecentProject) -> CoreResult<()> {
        self.conn
            .execute(
                "INSERT INTO recent_project(path, name, client_name, event_type, event_date,
                                            location, photo_count, last_opened)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7,
                        -- estritamente crescente, mesmo com duas aberturas no mesmo milissegundo
                        MAX(?8, COALESCE((SELECT MAX(last_opened) FROM recent_project), 0) + 1))
                 ON CONFLICT(path) DO UPDATE SET name = excluded.name,
                    client_name = excluded.client_name, event_type = excluded.event_type,
                    event_date = excluded.event_date, location = excluded.location,
                    photo_count = excluded.photo_count, last_opened = excluded.last_opened",
                params![
                    p.path,
                    p.name,
                    p.client_name,
                    p.event_type,
                    p.event_date,
                    p.location,
                    p.photo_count as i64,
                    now_millis()
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn update_photo_count(&self, path: &str, count: u64) -> CoreResult<()> {
        self.conn
            .execute(
                "UPDATE recent_project SET photo_count = ?1 WHERE path = ?2",
                params![count as i64, path],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn recent(&self, limit: u32) -> CoreResult<Vec<RecentProject>> {
        let mut st = self
            .conn
            .prepare(
                "SELECT path, name, client_name, event_type, event_date, location, photo_count,
                        last_opened
                 FROM recent_project ORDER BY last_opened DESC, rowid DESC LIMIT ?1",
            )
            .map_err(db_err)?;
        let rows = st
            .query_map([limit], |r| {
                let path: String = r.get(0)?;
                let available = Path::new(&path).join("catalog.db").is_file();
                Ok(RecentProject {
                    path,
                    name: r.get(1)?,
                    client_name: r.get(2)?,
                    event_type: r.get(3)?,
                    event_date: r.get(4)?,
                    location: r.get(5)?,
                    photo_count: r.get::<_, i64>(6)? as u64,
                    last_opened: r.get(7)?,
                    available,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Remove da lista (não apaga o projeto do disco).
    pub fn forget(&self, path: &str) -> CoreResult<()> {
        self.conn.execute("DELETE FROM recent_project WHERE path = ?1", [path]).map_err(db_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rp(path: &str) -> RecentProject {
        RecentProject {
            path: path.into(),
            name: "Casamento".into(),
            client_name: "Ana & Pedro".into(),
            event_type: "wedding".into(),
            event_date: Some("2026-06-15".into()),
            location: "Sintra".into(),
            photo_count: 0,
            last_opened: 0,
            available: false,
        }
    }

    #[test]
    fn recent_projects_order_update_and_forget() {
        let lib = Library::open_in_memory().unwrap();
        lib.touch(&rp("/a.momento")).unwrap();
        lib.touch(&rp("/b.momento")).unwrap();
        lib.update_photo_count("/a.momento", 42).unwrap();
        lib.touch(&RecentProject { photo_count: 42, ..rp("/a.momento") }).unwrap();
        let r = lib.recent(10).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].path, "/a.momento", "último aberto primeiro");
        assert_eq!(r[0].photo_count, 42);
        assert!(!r[0].available, "pasta inexistente");
        lib.forget("/a.momento").unwrap();
        assert_eq!(lib.recent(10).unwrap().len(), 1);
    }
}

/// Preset guardado (o conteúdo é o JSON do crate `momento-presets`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetRow {
    pub id: String,
    pub name: String,
    pub group: String,
    pub position: i64,
    pub body: String,
}

impl Library {
    /// Presets do utilizador, por grupo e posição.
    pub fn presets(&self) -> CoreResult<Vec<PresetRow>> {
        let mut st = self
            .conn
            .prepare("SELECT id, name, group_name, position, body FROM preset ORDER BY group_name, position, name")
            .map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                Ok(PresetRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    group: r.get(2)?,
                    position: r.get(3)?,
                    body: r.get(4)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    /// Cria ou substitui um preset.
    pub fn save_preset(&self, p: &PresetRow) -> CoreResult<()> {
        self.conn
            .execute(
                "INSERT INTO preset(id, name, group_name, position, body, updated_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET name = ?2, group_name = ?3, position = ?4, body = ?5, updated_at = ?6",
                params![p.id, p.name, p.group, p.position, p.body, now_millis()],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn delete_preset(&self, id: &str) -> CoreResult<bool> {
        Ok(self.conn.execute("DELETE FROM preset WHERE id = ?1", params![id]).map_err(db_err)? > 0)
    }

    /// Próxima posição livre num grupo.
    pub fn next_preset_position(&self, group: &str) -> CoreResult<i64> {
        self.conn
            .query_row(
                "SELECT coalesce(max(position) + 1, 0) FROM preset WHERE group_name = ?1",
                params![group],
                |r| r.get(0),
            )
            .map_err(db_err)
    }
}

/// Receita ou plano de exportação do utilizador (JSON do `momento-export-engine`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPresetRow {
    pub id: String,
    /// `recipe` | `plan`.
    pub kind: String,
    pub name: String,
    pub position: i64,
    pub body: String,
}

impl Library {
    pub fn export_presets(&self, kind: &str) -> CoreResult<Vec<ExportPresetRow>> {
        let mut st = self
            .conn
            .prepare("SELECT id, kind, name, position, body FROM export_preset WHERE kind = ?1 ORDER BY position, name")
            .map_err(db_err)?;
        let rows = st
            .query_map(params![kind], |r| {
                Ok(ExportPresetRow {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    name: r.get(2)?,
                    position: r.get(3)?,
                    body: r.get(4)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn save_export_preset(&self, p: &ExportPresetRow) -> CoreResult<()> {
        self.conn
            .execute(
                "INSERT INTO export_preset(id, kind, name, position, body, updated_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET name = ?3, position = ?4, body = ?5, updated_at = ?6",
                params![p.id, p.kind, p.name, p.position, p.body, now_millis()],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn delete_export_preset(&self, id: &str) -> CoreResult<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM export_preset WHERE id = ?1", params![id])
            .map_err(db_err)?
            > 0)
    }
}

impl Library {
    /// Presets de laboratório (JSON do `momento-album`), por nome.
    pub fn lab_presets(&self) -> CoreResult<Vec<(String, String, String)>> {
        let mut st = self
            .conn
            .prepare("SELECT id, name, body FROM album_lab ORDER BY name")
            .map_err(db_err)?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(db_err)?;
        rows.collect::<Result<_, _>>().map_err(db_err)
    }

    pub fn save_lab_preset(&self, id: &str, name: &str, body: &str) -> CoreResult<()> {
        self.conn
            .execute(
                "INSERT INTO album_lab(id, name, body, updated_at) VALUES(?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET name = ?2, body = ?3, updated_at = ?4",
                params![id, name, body, now_millis()],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn delete_lab_preset(&self, id: &str) -> CoreResult<bool> {
        Ok(self.conn.execute("DELETE FROM album_lab WHERE id = ?1", params![id]).map_err(db_err)?
            > 0)
    }
}

#[cfg(test)]
mod export_preset_tests {
    use super::*;

    #[test]
    fn export_presets_roundtrip() {
        let lib = Library::open_in_memory().unwrap();
        let row = ExportPresetRow {
            id: "u1".into(),
            kind: "recipe".into(),
            name: "Cliente".into(),
            position: 0,
            body: "{}".into(),
        };
        lib.save_export_preset(&row).unwrap();
        lib.save_export_preset(&ExportPresetRow { name: "Cliente 2".into(), ..row.clone() })
            .unwrap();
        let all = lib.export_presets("recipe").unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Cliente 2");
        assert!(lib.export_presets("plan").unwrap().is_empty());
        assert!(lib.delete_export_preset("u1").unwrap());
        assert!(lib.export_presets("recipe").unwrap().is_empty());
    }
}

#[cfg(test)]
mod preset_tests {
    use super::*;

    #[test]
    fn preset_crud_and_order() {
        let l = Library::open_in_memory().unwrap();
        let row = |id: &str, g: &str, pos: i64| PresetRow {
            id: id.into(),
            name: id.to_uppercase(),
            group: g.into(),
            position: pos,
            body: "{}".into(),
        };
        l.save_preset(&row("b", "Cor", 1)).unwrap();
        l.save_preset(&row("a", "Cor", 0)).unwrap();
        l.save_preset(&row("c", "", 0)).unwrap();
        assert_eq!(
            l.presets().unwrap().iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["c", "a", "b"]
        );
        assert_eq!(l.next_preset_position("Cor").unwrap(), 2);
        let mut r = row("a", "Película", 0);
        r.name = "Novo".into();
        l.save_preset(&r).unwrap();
        assert_eq!(l.presets().unwrap().iter().find(|p| p.id == "a").unwrap().group, "Película");
        assert!(l.delete_preset("a").unwrap());
        assert!(!l.delete_preset("a").unwrap());
        assert_eq!(l.presets().unwrap().len(), 2);
    }
}
