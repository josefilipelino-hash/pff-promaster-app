//! Disponibilidade dos originais (Fase 13): disco desligado → **offline** (a referência
//! fica intacta e volta sozinha), ficheiro desaparecido com o disco presente → **em falta**
//! (relink). Nunca se apaga uma referência por um original não estar acessível.

use momento_core_types::{AssetId, CoreError, CoreResult};
use momento_database::safety::{Availability, Original};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Erro para um original inacessível — só com o nome do ficheiro (sem o caminho, que
/// pode revelar nomes de clientes ou pastas pessoais nos logs).
pub fn unavailable(path: &str) -> CoreError {
    let name =
        Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    CoreError::Unavailable(name)
}

/// Lê um original. "Não existe" e "sem acesso" → [`CoreError::Unavailable`].
pub fn read_original(path: &str) -> CoreResult<Vec<u8>> {
    std::fs::read(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied => unavailable(path),
        _ => CoreError::Io(e),
    })
}

/// Estado de cada original agora. A existência das pastas é guardada em cache durante a
/// verificação (15 000 fotografias em poucas pastas → poucas chamadas ao sistema).
pub fn scan(originals: &[Original]) -> Vec<(AssetId, Availability)> {
    let mut dirs: HashMap<PathBuf, bool> = HashMap::new();
    originals
        .iter()
        .map(|o| {
            let p = Path::new(&o.path);
            let a = if p.is_file() {
                Availability::Ok
            } else {
                let parent = p.parent().map(Path::to_path_buf).unwrap_or_default();
                let parent_ok = *dirs.entry(parent.clone()).or_insert_with(|| parent.is_dir());
                if parent_ok {
                    Availability::Missing
                } else {
                    Availability::Offline
                }
            };
            (o.id, a)
        })
        .collect()
}

/// Procura numa pasta (e subpastas) ficheiros com o mesmo nome e tamanho que os originais
/// pedidos e confirma o conteúdo pelo hash BLAKE3. Devolve (fotografia, novo caminho).
pub fn find_in_folder(
    folder: &Path,
    wanted: &[Original],
    max_depth: usize,
) -> Vec<(AssetId, PathBuf)> {
    let mut by_name: HashMap<String, Vec<&Original>> = HashMap::new();
    for o in wanted {
        by_name.entry(o.file_name.to_lowercase()).or_default().push(o);
    }
    let mut found = Vec::new();
    let mut taken = std::collections::HashSet::new();
    walk(folder, max_depth, &mut |path| {
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_lowercase()) else {
            return;
        };
        let Some(cands) = by_name.get(&name) else { return };
        let Ok(meta) = std::fs::metadata(path) else { return };
        for o in cands {
            if taken.contains(&o.id) || meta.len() != o.file_size {
                continue;
            }
            if let Ok(data) = std::fs::read(path) {
                if <[u8; 32]>::from(blake3::hash(&data)) == o.content_hash {
                    taken.insert(o.id);
                    found.push((o.id, path.to_path_buf()));
                    break;
                }
            }
        }
    });
    found
}

fn walk(dir: &Path, depth: usize, f: &mut dyn FnMut(&Path)) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let p = e.path();
        if ft.is_dir() {
            let hidden = e.file_name().to_string_lossy().starts_with('.');
            if depth > 0 && !hidden {
                walk(&p, depth - 1, f);
            }
        } else if ft.is_file() {
            f(&p);
        }
    }
}

/// Confirma que `path` tem exatamente o conteúdo do original (relink de um ficheiro).
pub fn same_content(path: &Path, o: &Original) -> CoreResult<bool> {
    let meta = std::fs::metadata(path)?;
    if meta.len() != o.file_size {
        return Ok(false);
    }
    Ok(<[u8; 32]>::from(blake3::hash(&std::fs::read(path)?)) == o.content_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn orig(id: i64, path: &Path, data: &[u8]) -> Original {
        Original {
            id: AssetId(id),
            path: path.to_string_lossy().into_owned(),
            file_name: path.file_name().unwrap().to_string_lossy().into_owned(),
            file_size: data.len() as u64,
            content_hash: blake3::hash(data).into(),
            status: Availability::Ok,
            backup_path: None,
        }
    }

    #[test]
    fn offline_missing_and_relink_search() {
        let d = tempfile::tempdir().unwrap();
        let disk = d.path().join("Disco");
        std::fs::create_dir_all(disk.join("casamento")).unwrap();
        let a = disk.join("casamento/A.jpg");
        let b = disk.join("casamento/B.jpg");
        std::fs::write(&a, b"aaaa").unwrap();
        std::fs::write(&b, b"bbbb").unwrap();
        let os = vec![orig(1, &a, b"aaaa"), orig(2, &b, b"bbbb")];
        assert!(scan(&os).iter().all(|(_, s)| *s == Availability::Ok));
        // Ficheiro apagado com o disco presente → em falta.
        std::fs::remove_file(&b).unwrap();
        assert_eq!(scan(&os)[1].1, Availability::Missing);
        // Disco "desligado" → offline (não em falta).
        let elsewhere = d.path().join("Disco-desligado");
        std::fs::rename(&disk, &elsewhere).unwrap();
        assert!(scan(&os).iter().all(|(_, s)| *s == Availability::Offline));
        assert!(
            matches!(read_original(&os[0].path), Err(CoreError::Unavailable(n)) if n == "A.jpg")
        );
        // Relink por pasta: encontra pelo nome, confirma pelo conteúdo.
        std::fs::write(elsewhere.join("casamento/B.jpg"), b"outro").unwrap();
        let found = find_in_folder(&elsewhere, &os, 4);
        assert_eq!(found.len(), 1, "B tem outro conteúdo: não é aceite");
        assert_eq!(found[0].0, AssetId(1));
        assert!(same_content(&found[0].1, &os[0]).unwrap());
    }
}
