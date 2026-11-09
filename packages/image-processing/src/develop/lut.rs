//! LUTs de cor `.cube` (formato Adobe/Resolve): 1D e 3D, interpolação linear/trilinear.
//!
//! **O que é reproduzido corretamente:** LUTs criativas definidas para entrada e saída
//! **sRGB** (domínio 0–1). Aplicam-se no fim do pipeline, depois de converter para sRGB e
//! antes de quantizar, com a mesma interpolação que os editores de vídeo — o resultado é
//! o da LUT (as cores fora de sRGB são cortadas antes, como num ecrã sRGB).
//!
//! **O que é recusado:** LUTs para entradas *log* ou HDR (domínio diferente de 0–1, ex.:
//! S-Log3, V-Log, LogC): o motor não produz essas curvas, por isso o aspeto não seria o
//! pretendido. LUTs 3D maiores do que 65³ também são recusadas.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

#[derive(Debug, Clone, PartialEq)]
pub struct Lut {
    pub title: String,
    /// Número de amostras por eixo.
    pub size: usize,
    /// 3D (vermelho varia mais depressa, como no `.cube`) ou 1D (por canal).
    pub three_d: bool,
    pub data: Vec<[f32; 3]>,
}

/// Erros com texto para o utilizador.
pub fn parse_cube(text: &str) -> Result<Lut, String> {
    let mut title = String::new();
    let mut size3 = None;
    let mut size1 = None;
    let mut dmin = [0.0f32; 3];
    let mut dmax = [1.0f32; 3];
    let mut data: Vec<[f32; 3]> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        let first = it.next().unwrap_or("");
        let rest: Vec<&str> = it.collect();
        let nums = |v: &[&str]| -> Result<Vec<f32>, String> {
            v.iter().map(|x| x.parse::<f32>().map_err(|_| format!("valor inválido: {x}"))).collect()
        };
        match first {
            "TITLE" => title = line[5..].trim().trim_matches('"').to_string(),
            "LUT_3D_SIZE" => size3 = rest.first().and_then(|v| v.parse::<usize>().ok()),
            "LUT_1D_SIZE" => size1 = rest.first().and_then(|v| v.parse::<usize>().ok()),
            "DOMAIN_MIN" => {
                let v = nums(&rest)?;
                if v.len() == 3 {
                    dmin = [v[0], v[1], v[2]];
                }
            }
            "DOMAIN_MAX" => {
                let v = nums(&rest)?;
                if v.len() == 3 {
                    dmax = [v[0], v[1], v[2]];
                }
            }
            "LUT_3D_INPUT_RANGE" | "LUT_1D_INPUT_RANGE" => {
                let v = nums(&rest)?;
                if v.len() == 2 {
                    dmin = [v[0]; 3];
                    dmax = [v[1]; 3];
                }
            }
            _ if first.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.') => {
                let mut v = vec![first];
                v.extend(rest);
                let n = nums(&v)?;
                if n.len() != 3 {
                    return Err(format!("linha com {} valores (esperados 3)", n.len()));
                }
                data.push([n[0], n[1], n[2]]);
            }
            // Palavras-chave desconhecidas (ex.: LUT_IN_VIDEO_RANGE) — ignoradas pela norma.
            _ => {}
        }
    }
    if dmin.iter().any(|v| v.abs() > 1e-4) || dmax.iter().any(|v| (v - 1.0).abs() > 1e-4) {
        return Err(format!(
            "LUT com domínio {:?}–{:?}: feita para imagens log/HDR, que este motor não produz; não é suportada",
            dmin, dmax
        ));
    }
    let (size, three_d) = match (size3, size1) {
        (Some(n), _) => (n, true),
        (None, Some(n)) => (n, false),
        _ => return Err("falta LUT_3D_SIZE ou LUT_1D_SIZE".into()),
    };
    if three_d && !(2..=65).contains(&size) {
        return Err(format!("tamanho 3D {size} não suportado (2–65)"));
    }
    if !three_d && !(2..=65536).contains(&size) {
        return Err(format!("tamanho 1D {size} não suportado"));
    }
    let expected = if three_d { size * size * size } else { size };
    if data.len() != expected {
        return Err(format!("{} entradas, esperadas {expected}", data.len()));
    }
    if data.iter().flatten().any(|v| !v.is_finite()) {
        return Err("valores não finitos".into());
    }
    Ok(Lut { title, size, three_d, data })
}

impl Lut {
    /// Aplica a LUT a um RGB codificado (0–1).
    pub fn apply(&self, c: [f32; 3]) -> [f32; 3] {
        let n = self.size;
        let pos = |v: f32| -> (usize, usize, f32) {
            let x = v.clamp(0.0, 1.0) * (n - 1) as f32;
            let i = (x as usize).min(n - 2);
            (i, i + 1, x - i as f32)
        };
        if !self.three_d {
            return std::array::from_fn(|ch| {
                let (a, b, t) = pos(c[ch]);
                self.data[a][ch] * (1.0 - t) + self.data[b][ch] * t
            });
        }
        let (r0, r1, fr) = pos(c[0]);
        let (g0, g1, fg) = pos(c[1]);
        let (b0, b1, fb) = pos(c[2]);
        let at = |r: usize, g: usize, b: usize| self.data[r + g * n + b * n * n];
        let lerp = |a: [f32; 3], b: [f32; 3], t: f32| -> [f32; 3] {
            std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
        };
        let c00 = lerp(at(r0, g0, b0), at(r1, g0, b0), fr);
        let c10 = lerp(at(r0, g1, b0), at(r1, g1, b0), fr);
        let c01 = lerp(at(r0, g0, b1), at(r1, g0, b1), fr);
        let c11 = lerp(at(r0, g1, b1), at(r1, g1, b1), fr);
        lerp(lerp(c00, c10, fg), lerp(c01, c11, fg), fb)
    }
}

/// Identificador de uma LUT: BLAKE3 do ficheiro (16 bytes em hexadecimal).
pub fn lut_id(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex()[..32].to_string()
}

fn registry() -> &'static RwLock<HashMap<String, Arc<Lut>>> {
    static R: OnceLock<RwLock<HashMap<String, Arc<Lut>>>> = OnceLock::new();
    R.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Pastas onde procurar `<id>.cube` quando uma receita usa uma LUT ainda não carregada.
fn search_paths() -> &'static RwLock<Vec<std::path::PathBuf>> {
    static P: OnceLock<RwLock<Vec<std::path::PathBuf>>> = OnceLock::new();
    P.get_or_init(|| RwLock::new(Vec::new()))
}

pub fn add_search_path(p: std::path::PathBuf) {
    let mut g = search_paths().write().unwrap_or_else(|e| e.into_inner());
    if !g.contains(&p) {
        g.push(p);
    }
}

pub fn register(id: &str, lut: Lut) -> Arc<Lut> {
    let l = Arc::new(lut);
    registry().write().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), l.clone());
    l
}

/// LUT pelo identificador (em memória ou numa das pastas registadas).
pub fn get(id: &str) -> Option<Arc<Lut>> {
    if id.is_empty() {
        return None;
    }
    if let Some(l) = registry().read().unwrap_or_else(|e| e.into_inner()).get(id) {
        return Some(l.clone());
    }
    let paths = search_paths().read().unwrap_or_else(|e| e.into_inner()).clone();
    for dir in paths {
        let f = dir.join(format!("{id}.cube"));
        if let Ok(text) = std::fs::read_to_string(&f) {
            if let Ok(l) = parse_cube(&text) {
                return Some(register(id, l));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_cube(n: usize) -> String {
        let mut s = format!("TITLE \"id\"\nLUT_3D_SIZE {n}\n");
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    let f = |v: usize| v as f32 / (n - 1) as f32;
                    s += &format!("{} {} {}\n", f(r), f(g), f(b));
                }
            }
        }
        s
    }

    #[test]
    fn identity_and_channel_order() {
        let l = parse_cube(&identity_cube(17)).unwrap();
        assert_eq!((l.size, l.three_d, l.title.as_str()), (17, true, "id"));
        for c in [[0.1, 0.5, 0.9], [0.33, 0.2, 0.75], [1.0, 0.0, 0.5]] {
            let o = l.apply(c);
            for i in 0..3 {
                assert!((o[i] - c[i]).abs() < 1e-5, "{c:?} → {o:?}");
            }
        }
        // LUT que troca vermelho e azul: confirma a ordem (vermelho varia mais depressa).
        let mut s = String::from("LUT_3D_SIZE 2\n");
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    s += &format!("{b} {g} {r}\n");
                }
            }
        }
        let sw = parse_cube(&s).unwrap();
        let o = sw.apply([0.8, 0.3, 0.1]);
        assert!((o[0] - 0.1).abs() < 1e-5 && (o[2] - 0.8).abs() < 1e-5);
    }

    #[test]
    fn one_d_and_rejections() {
        let l = parse_cube("LUT_1D_SIZE 2\n0 0 0\n0.5 1 1\n").unwrap();
        let o = l.apply([1.0, 0.5, 0.0]);
        assert!((o[0] - 0.5).abs() < 1e-6 && (o[1] - 0.5).abs() < 1e-6 && o[2].abs() < 1e-6);
        assert!(parse_cube("LUT_3D_SIZE 2\nDOMAIN_MIN -0.1 -0.1 -0.1\nDOMAIN_MAX 1.2 1.2 1.2\n")
            .unwrap_err()
            .contains("log"));
        assert!(parse_cube("LUT_3D_SIZE 2\n0 0 0\n").is_err(), "entradas em falta");
        assert!(parse_cube("0 0 0\n").is_err(), "sem tamanho");
        assert!(parse_cube("LUT_3D_SIZE 2\nabc 0 0\n").is_err());
    }
}
