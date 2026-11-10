//! # momento-album
//!
//! Álbuns fotográficos (Fase 11): especificação física e presets de laboratório, spreads
//! com slots em mm, layouts de 1 a 6+ fotografias, montagem automática com narrativa,
//! verificação (DPI, zona segura, dobra) e exportação (JPEG/TIFF por spread, PDF de
//! impressão e de prova). A revelação das fotografias vem de fora (fornecedor), sempre a
//! partir dos originais.

pub mod auto;
pub mod compose;
pub mod export;
pub mod layouts;
pub mod model;
pub mod pdf;
pub mod preflight;
pub mod spec;

pub use model::*;
pub use spec::*;

/// "Álbum Ana & Pedro" → "album-ana-e-pedro".
pub fn slug(s: &str) -> String {
    const FROM: &str = "áàâãäåçéèêëíìîïñóòôõöúùûüýÁÀÂÃÄÅÇÉÈÊËÍÌÎÏÑÓÒÔÕÖÚÙÛÜÝ";
    const TO: &str = "aaaaaaceeeeiiiinooooouuuuyaaaaaaceeeeiiiinooooouuuuy";
    let mut out = String::new();
    for c in s.replace('&', " e ").chars() {
        let c = FROM.chars().position(|f| f == c).map(|i| TO.as_bytes()[i] as char).unwrap_or(c);
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let t = out.trim_matches('-').to_string();
    if t.is_empty() {
        "album".into()
    } else {
        t
    }
}

#[cfg(test)]
mod tests;
