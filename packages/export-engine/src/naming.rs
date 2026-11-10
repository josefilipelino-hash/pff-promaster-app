//! Nomes e pastas dos ficheiros exportados.

use crate::recipe::{Conflict, Naming};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Valores dos campos do modelo de nome.
#[derive(Debug, Clone, Default)]
pub struct Vars {
    pub stem: String,
    pub seq: u32,
    /// "AAAA-MM-DD"
    pub date: String,
    /// "HHMMSS"
    pub time: String,
    pub project: String,
    pub client: String,
    pub recipe: String,
    pub moment: String,
    pub width: u32,
    pub height: u32,
    /// Identificador aleatório (galerias): campo {token}.
    pub token: String,
}

/// Carateres proibidos em Windows/macOS e nomes reservados do Windows.
pub fn sanitize(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    out = out.trim().trim_end_matches('.').trim().to_string();
    let upper = out.to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"];
    let stem = upper.split('.').next().unwrap_or("");
    if reserved.contains(&stem)
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
    {
        out = format!("_{out}");
    }
    out
}

fn expand(t: &str, n: &Naming, v: &Vars) -> String {
    let seq = format!("{:0width$}", v.seq, width = n.seq_digits.clamp(1, 9) as usize);
    t.replace("{nome}", &v.stem)
        .replace("{seq}", &seq)
        .replace("{data}", &v.date)
        .replace("{hora}", &v.time)
        .replace("{projeto}", &v.project)
        .replace("{cliente}", &v.client)
        .replace("{receita}", &v.recipe)
        .replace("{momento}", &v.moment)
        .replace("{largura}", &v.width.to_string())
        .replace("{altura}", &v.height.to_string())
        .replace("{token}", &v.token)
}

/// Nome do ficheiro (com extensão).
pub fn file_name(n: &Naming, v: &Vars, ext: &str) -> String {
    let body = sanitize(&format!("{}{}{}", n.prefix, expand(&n.template, n, v), n.suffix));
    let body = if body.is_empty() { "foto".to_string() } else { body };
    format!("{body}.{ext}")
}

/// Subpasta relativa (níveis separados por "/"; sem ".." nem caminhos absolutos).
pub fn subfolder(n: &Naming, v: &Vars) -> PathBuf {
    let mut p = PathBuf::new();
    for part in expand(&n.subfolder, n, v).split(['/', '\\']) {
        let s = sanitize(part);
        if s.is_empty() || s == "." || s == ".." {
            continue;
        }
        p.push(s);
    }
    p
}

fn key(p: &Path) -> String {
    // Sistemas de ficheiros sem distinção de maiúsculas (Windows, macOS).
    p.to_string_lossy().to_lowercase()
}

/// Decide o caminho final segundo a política de conflitos. `reserved` guarda os caminhos
/// já atribuídos nesta exportação; `protected` os originais (nunca sobrescritos).
pub fn resolve(
    dir: &Path,
    name: &str,
    conflict: Conflict,
    reserved: &mut HashSet<String>,
    protected: &HashSet<String>,
) -> Option<PathBuf> {
    let first = dir.join(name);
    let taken = |p: &Path, reserved: &HashSet<String>| {
        reserved.contains(&key(p)) || protected.contains(&key(p)) || p.exists()
    };
    let chosen = match conflict {
        Conflict::Skip if taken(&first, reserved) => return None,
        Conflict::Overwrite
            if !reserved.contains(&key(&first)) && !protected.contains(&key(&first)) =>
        {
            first
        }
        _ => {
            if !taken(&first, reserved) {
                first
            } else {
                let (stem, ext) = match name.rfind('.') {
                    Some(i) => (&name[..i], &name[i..]),
                    None => (name, ""),
                };
                (2..)
                    .map(|i| dir.join(format!("{stem}-{i}{ext}")))
                    .find(|p| !taken(p, reserved))
                    .expect("nome livre")
            }
        }
    };
    reserved.insert(key(&chosen));
    Some(chosen)
}

/// Chave de comparação de caminhos (para a lista de originais protegidos).
pub fn path_key(p: &Path) -> String {
    key(&std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vars {
        Vars {
            stem: "IMG_0001".into(),
            seq: 7,
            date: "2026-06-15".into(),
            project: "Casamento Ana & Pedro".into(),
            client: "Ana: Pedro".into(),
            recipe: "Galeria".into(),
            moment: "Cerimónia".into(),
            ..Default::default()
        }
    }

    #[test]
    fn names_tokens_and_sanitizing() {
        let n = Naming {
            template: "{data}_{seq}_{nome}".into(),
            prefix: "AP_".into(),
            suffix: "_web".into(),
            seq_digits: 4,
            ..Default::default()
        };
        assert_eq!(file_name(&n, &vars(), "jpg"), "AP_2026-06-15_0007_IMG_0001_web.jpg");
        let n = Naming { template: "{cliente}/{nome}?".into(), ..Default::default() };
        assert_eq!(file_name(&n, &vars(), "tif"), "Ana_ Pedro_IMG_0001_.tif");
        assert_eq!(sanitize("CON"), "_CON");
        assert_eq!(sanitize("  ponto. "), "ponto");
        let n = Naming { subfolder: "Entrega/../{momento}//Alta".into(), ..Default::default() };
        assert_eq!(subfolder(&n, &vars()), PathBuf::from("Entrega").join("Cerimónia").join("Alta"));
    }

    #[test]
    fn conflicts() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.jpg"), b"x").unwrap();
        let mut reserved = HashSet::new();
        let protected = HashSet::new();
        let r = resolve(d.path(), "a.jpg", Conflict::Rename, &mut reserved, &protected).unwrap();
        assert_eq!(r.file_name().unwrap(), "a-2.jpg");
        let r = resolve(d.path(), "a.jpg", Conflict::Rename, &mut reserved, &protected).unwrap();
        assert_eq!(r.file_name().unwrap(), "a-3.jpg", "reservado nesta exportação");
        assert!(resolve(d.path(), "a.jpg", Conflict::Skip, &mut reserved, &protected).is_none());
        let mut fresh = HashSet::new();
        let r = resolve(d.path(), "a.jpg", Conflict::Overwrite, &mut fresh, &protected).unwrap();
        assert_eq!(r.file_name().unwrap(), "a.jpg");
        // Um original nunca é sobrescrito, mesmo com "substituir".
        let mut prot = HashSet::new();
        prot.insert(key(&d.path().join("a.jpg")));
        let r =
            resolve(d.path(), "a.jpg", Conflict::Overwrite, &mut HashSet::new(), &prot).unwrap();
        assert_ne!(r.file_name().unwrap(), "a.jpg");
    }
}
