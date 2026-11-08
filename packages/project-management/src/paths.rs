//! Estrutura da pasta de um projeto:
//!
//! ```text
//! <pasta de trabalho>/<AAAA-MM-DD_Cliente>.momento/
//!     catalog.db              catálogo SQLite (fonte de verdade)
//!     backups/                cópias de segurança do catálogo
//!     cache/thumbs/<xx>/<id>.jpg     miniaturas (regeneráveis)
//!     cache/previews/<xx>/<id>.jpg   previews 2560 px (regeneráveis)
//!     cache/full/<xx>/<id>.jpg       resolução total para TIFF (regenerável)
//! ```

use momento_core_types::AssetId;
use std::path::{Path, PathBuf};

pub const PROJECT_EXTENSION: &str = "momento";
pub const CATALOG_FILE: &str = "catalog.db";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPaths {
    pub root: PathBuf,
}

/// Nível de cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheLevel {
    Thumb,
    Preview,
    Full,
}

impl CacheLevel {
    fn dir(self) -> &'static str {
        match self {
            CacheLevel::Thumb => "thumbs",
            CacheLevel::Preview => "previews",
            CacheLevel::Full => "full",
        }
    }
}

impl ProjectPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        ProjectPaths { root: root.into() }
    }

    /// Aceita a pasta `.momento` ou o próprio `catalog.db`.
    pub fn from_user_path(p: &Path) -> Self {
        if p.file_name().is_some_and(|n| n == CATALOG_FILE) {
            ProjectPaths::new(p.parent().unwrap_or(p))
        } else {
            ProjectPaths::new(p)
        }
    }

    pub fn catalog(&self) -> PathBuf {
        self.root.join(CATALOG_FILE)
    }
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// Pasta de um nível de cache.
    pub fn level_dir(&self, level: CacheLevel) -> PathBuf {
        self.cache().join(level.dir())
    }

    /// Caminho determinístico (sem consulta ao catálogo), dividido em 256 subpastas.
    pub fn cached(&self, level: CacheLevel, id: AssetId) -> PathBuf {
        self.cache()
            .join(level.dir())
            .join(format!("{:02x}", (id.0 & 0xff) as u8))
            .join(format!("{}.jpg", id.0))
    }
}

/// Nome de pasta a partir da data e do cliente: `2026-06-15_Ana_e_Pedro`.
pub fn project_folder_name(event_date: Option<&str>, client: &str, name: &str) -> String {
    let base = if client.trim().is_empty() { name } else { client };
    let mut slug = String::new();
    for c in base.trim().chars() {
        if c.is_alphanumeric() || c == '-' {
            slug.push(c);
        } else if c == '&' || c == '+' {
            slug.push_str("_e_");
        } else if !slug.ends_with('_') {
            slug.push('_');
        }
    }
    let mut slug: String = slug.split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_");
    if slug.is_empty() {
        slug = "Projeto".into();
    }
    slug.truncate(slug.char_indices().nth(80).map(|(i, _)| i).unwrap_or(slug.len()));
    match event_date {
        Some(d) if !d.is_empty() => format!("{d}_{slug}.{PROJECT_EXTENSION}"),
        _ => format!("{slug}.{PROJECT_EXTENSION}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names() {
        assert_eq!(
            project_folder_name(Some("2026-06-15"), "Ana & Pedro", "x"),
            "2026-06-15_Ana_e_Pedro.momento"
        );
        assert_eq!(project_folder_name(None, "", "Batizado / Maria"), "Batizado_Maria.momento");
        assert_eq!(project_folder_name(None, "  ", "  "), "Projeto.momento");
        assert_eq!(
            project_folder_name(Some("2026-01-01"), "João Ção", ""),
            "2026-01-01_João_Ção.momento"
        );
    }

    #[test]
    fn cache_paths_are_sharded() {
        let p = ProjectPaths::new("/p.momento");
        assert_eq!(
            p.cached(CacheLevel::Thumb, AssetId(258)),
            PathBuf::from("/p.momento/cache/thumbs/02/258.jpg")
        );
        assert_eq!(
            ProjectPaths::from_user_path(Path::new("/p.momento/catalog.db")).root,
            PathBuf::from("/p.momento")
        );
    }
}
