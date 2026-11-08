//! Gestão do tamanho da cache de previews (LRU pela data de modificação, que é
//! atualizada sempre que um ficheiro é servido).
//!
//! - `thumbs/`: nunca é limpa automaticamente (≈15 KB por fotografia);
//! - `previews/`: limite por omissão de 10 GB;
//! - `full/`: limite por omissão de 5 GB (revelações RAW em resolução total, ≈10–20 MB cada).

use crate::paths::{CacheLevel, ProjectPaths};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const DEFAULT_PREVIEW_LIMIT: u64 = 10 * 1024 * 1024 * 1024;
pub const DEFAULT_FULL_LIMIT: u64 = 5 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelStats {
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub thumbs: LevelStats,
    pub previews: LevelStats,
    pub full: LevelStats,
}

fn files(dir: &Path) -> Vec<(PathBuf, u64, SystemTime)> {
    let mut out = Vec::new();
    let Ok(shards) = std::fs::read_dir(dir) else { return out };
    for shard in shards.flatten() {
        let Ok(entries) = std::fs::read_dir(shard.path()) else { continue };
        for e in entries.flatten() {
            if let Ok(m) = e.metadata() {
                if m.is_file() && e.file_name().to_string_lossy().ends_with(".jpg") {
                    out.push((e.path(), m.len(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH)));
                }
            }
        }
    }
    out
}

fn stats_of(dir: &Path) -> LevelStats {
    let f = files(dir);
    LevelStats { files: f.len() as u64, bytes: f.iter().map(|x| x.1).sum() }
}

pub fn stats(paths: &ProjectPaths) -> CacheStats {
    CacheStats {
        thumbs: stats_of(&paths.level_dir(CacheLevel::Thumb)),
        previews: stats_of(&paths.level_dir(CacheLevel::Preview)),
        full: stats_of(&paths.level_dir(CacheLevel::Full)),
    }
}

/// Remove os ficheiros menos usados até o nível ficar abaixo de `limit` bytes.
/// Devolve (ficheiros removidos, bytes libertados).
pub fn prune(dir: &Path, limit: u64) -> (u64, u64) {
    let mut f = files(dir);
    let mut total: u64 = f.iter().map(|x| x.1).sum();
    if total <= limit {
        return (0, 0);
    }
    f.sort_by_key(|x| x.2); // mais antigos primeiro
    let (mut n, mut freed) = (0, 0);
    for (p, size, _) in f {
        if total <= limit {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            total -= size;
            freed += size;
            n += 1;
        }
    }
    (n, freed)
}

pub fn prune_project(paths: &ProjectPaths) {
    let (n1, b1) = prune(&paths.level_dir(CacheLevel::Preview), DEFAULT_PREVIEW_LIMIT);
    let (n2, b2) = prune(&paths.level_dir(CacheLevel::Full), DEFAULT_FULL_LIMIT);
    if n1 + n2 > 0 {
        tracing::info!(
            removidos = n1 + n2,
            libertados_mb = (b1 + b2) / 1_048_576,
            "cache de previews limpa (LRU)"
        );
    }
}

/// Marca um ficheiro como usado agora (para o LRU).
pub fn touch(path: &Path) {
    if let Ok(f) = std::fs::File::options().append(true).open(path) {
        let _ = f.set_modified(SystemTime::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_removes_least_recently_used_first() {
        let dir = tempfile::tempdir().unwrap();
        let shard = dir.path().join("00");
        std::fs::create_dir_all(&shard).unwrap();
        for i in 0..5u64 {
            let p = shard.join(format!("{i}.jpg"));
            std::fs::write(&p, vec![0u8; 1000]).unwrap();
            let f = std::fs::File::options().append(true).open(&p).unwrap();
            f.set_modified(
                SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000 + i * 100),
            )
            .unwrap();
        }
        // O ficheiro 0 (o mais antigo) é usado agora: passa a ser o mais recente.
        touch(&shard.join("0.jpg"));
        let (n, freed) = prune(dir.path(), 2500);
        assert_eq!((n, freed), (3, 3000));
        let mut left: Vec<_> = std::fs::read_dir(&shard)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, ["0.jpg", "4.jpg"]);
        assert_eq!(stats_of(dir.path()), LevelStats { files: 2, bytes: 2000 });
    }
}
