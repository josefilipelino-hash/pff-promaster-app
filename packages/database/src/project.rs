//! Registo do projeto (uma única linha por catálogo).

use crate::{db_err, now_unix, Catalog};
use momento_core_types::{CoreError, CoreResult};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Tipos de evento suportados. O texto mostrado vem das traduções (`eventType.<id>`).
pub const EVENT_TYPES: &[&str] =
    &["wedding", "baptism", "communion", "corporate", "session", "concert", "social", "other"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub name: String,
    pub client_name: String,
    pub event_type: String,
    /// Data do evento `AAAA-MM-DD`.
    pub event_date: Option<String>,
    pub location: String,
    pub originals_dir: Option<String>,
    pub status: String,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Valida `AAAA-MM-DD` (sem dependências de calendário).
pub fn is_valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| s[r].parse::<u32>().ok();
    match (num(0..4), num(5..7), num(8..10)) {
        (Some(y), Some(m), Some(d)) => {
            let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
            let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            (1900..=2200).contains(&y)
                && (1..=12).contains(&m)
                && d >= 1
                && d <= days[m as usize - 1]
        }
        _ => false,
    }
}

impl Catalog {
    pub fn create_project(&self, p: &ProjectRecord) -> CoreResult<()> {
        if p.name.trim().is_empty() {
            return Err(CoreError::Other("o nome do projeto é obrigatório".into()));
        }
        if !EVENT_TYPES.contains(&p.event_type.as_str()) {
            return Err(CoreError::Other(format!("tipo de evento inválido: {}", p.event_type)));
        }
        if let Some(d) = &p.event_date {
            if !is_valid_date(d) {
                return Err(CoreError::Other(format!("data inválida: {d}")));
            }
        }
        let now = now_unix();
        self.connection()
            .execute(
                "INSERT INTO project(id, name, client_name, event_type, event_date, location,
                                     originals_dir, status, notes, created_at, updated_at)
                 VALUES(1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
                params![
                    p.name.trim(),
                    p.client_name.trim(),
                    p.event_type,
                    p.event_date,
                    p.location.trim(),
                    p.originals_dir,
                    p.status,
                    p.notes,
                    now
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn project(&self) -> CoreResult<Option<ProjectRecord>> {
        self.connection()
            .query_row(
                "SELECT name, client_name, event_type, event_date, location, originals_dir,
                        status, notes, created_at, updated_at FROM project WHERE id = 1",
                [],
                |r| {
                    Ok(ProjectRecord {
                        name: r.get(0)?,
                        client_name: r.get(1)?,
                        event_type: r.get(2)?,
                        event_date: r.get(3)?,
                        location: r.get(4)?,
                        originals_dir: r.get(5)?,
                        status: r.get(6)?,
                        notes: r.get(7)?,
                        created_at: r.get(8)?,
                        updated_at: r.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(db_err)
    }

    pub fn set_project_status(&self, status: &str) -> CoreResult<()> {
        self.connection()
            .execute(
                "UPDATE project SET status = ?1, updated_at = ?2 WHERE id = 1",
                params![status, now_unix()],
            )
            .map_err(db_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn sample() -> ProjectRecord {
        ProjectRecord {
            name: "Casamento Ana & Pedro".into(),
            client_name: "Ana & Pedro".into(),
            event_type: "wedding".into(),
            event_date: Some("2026-06-15".into()),
            location: "Sintra".into(),
            originals_dir: Some("/fotos/originais".into()),
            status: "created".into(),
            notes: String::new(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn create_and_read_project() {
        let c = Catalog::open_in_memory().unwrap();
        assert!(c.project().unwrap().is_none());
        c.create_project(&sample()).unwrap();
        let p = c.project().unwrap().unwrap();
        assert_eq!(p.client_name, "Ana & Pedro");
        assert_eq!(p.event_date.as_deref(), Some("2026-06-15"));
        assert!(p.created_at > 0);
        // Só pode existir um projeto por catálogo.
        assert!(c.create_project(&sample()).is_err());
    }

    #[test]
    fn rejects_invalid_input() {
        let c = Catalog::open_in_memory().unwrap();
        let mut p = sample();
        p.event_date = Some("2026-02-30".into());
        assert!(c.create_project(&p).is_err());
        let mut p = sample();
        p.event_type = "festa".into();
        assert!(c.create_project(&p).is_err());
        let mut p = sample();
        p.name = "  ".into();
        assert!(c.create_project(&p).is_err());
    }

    #[test]
    fn date_validation() {
        assert!(is_valid_date("2024-02-29"));
        assert!(!is_valid_date("2025-02-29"));
        assert!(!is_valid_date("2025-13-01"));
        assert!(!is_valid_date("15-06-2026"));
    }
}
