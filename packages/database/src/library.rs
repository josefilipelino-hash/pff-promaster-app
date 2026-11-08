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
