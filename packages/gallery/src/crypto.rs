//! PIN/palavra-passe: a lista de fotografias (e os textos) é cifrada com AES-256-GCM,
//! chave derivada por PBKDF2-HMAC-SHA256; o navegador decifra com a WebCrypto.
//!
//! O que isto protege e o que não protege (explicado na interface e no PRIVACY.md):
//! sem o código ninguém vê a galeria nem descobre os nomes dos ficheiros (aleatórios,
//! 128 bits); quem tiver o endereço direto de uma imagem consegue abri-la. Um PIN de 4
//! dígitos é uma barreira ligeira (pode ser adivinhado por tentativa offline).

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use momento_core_types::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};

pub const ITERATIONS: u32 = 250_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sealed {
    pub v: u32,
    pub kdf: String,
    pub iter: u32,
    pub salt: String,
    pub iv: String,
    pub data: String,
}

pub fn random<const N: usize>() -> CoreResult<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|e| CoreError::Other(format!("aleatoriedade: {e}")))?;
    Ok(b)
}

fn key(secret: &str, salt: &[u8], iter: u32) -> [u8; 32] {
    let mut k = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(secret.as_bytes(), salt, iter, &mut k);
    k
}

pub fn seal(plain: &[u8], secret: &str) -> CoreResult<Sealed> {
    let salt = random::<16>()?;
    let iv = random::<12>()?;
    let k = key(secret, &salt, ITERATIONS);
    let cipher = Aes256Gcm::new_from_slice(&k).map_err(|e| CoreError::Other(e.to_string()))?;
    let data = cipher
        .encrypt(Nonce::from_slice(&iv), plain)
        .map_err(|_| CoreError::Other("cifra da galeria".into()))?;
    Ok(Sealed {
        v: 1,
        kdf: "PBKDF2-SHA256".into(),
        iter: ITERATIONS,
        salt: B64.encode(salt),
        iv: B64.encode(iv),
        data: B64.encode(data),
    })
}

/// Decifra (testes e verificação após gerar).
pub fn open(s: &Sealed, secret: &str) -> CoreResult<Vec<u8>> {
    let d = |v: &str| B64.decode(v).map_err(|_| CoreError::Corrupt("base64".into()));
    let k = key(secret, &d(&s.salt)?, s.iter);
    let cipher = Aes256Gcm::new_from_slice(&k).map_err(|e| CoreError::Other(e.to_string()))?;
    let iv = d(&s.iv)?;
    if iv.len() != 12 {
        return Err(CoreError::Corrupt("iv".into()));
    }
    cipher
        .decrypt(Nonce::from_slice(&iv), d(&s.data)?.as_ref())
        .map_err(|_| CoreError::Other("código errado".into()))
}

/// Identificador aleatório (hex, 128 bits) para nomes de ficheiros.
pub fn token() -> CoreResult<String> {
    Ok(random::<16>()?.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_and_open() {
        let s = seal(b"{\"photos\":[]}", "1234").unwrap();
        assert_eq!(open(&s, "1234").unwrap(), b"{\"photos\":[]}");
        assert!(open(&s, "1235").is_err());
        let again = seal(b"{\"photos\":[]}", "1234").unwrap();
        assert_ne!(s.salt, again.salt, "sal novo de cada vez");
        assert_ne!(s.data, again.data);
        assert_eq!(token().unwrap().len(), 32);
    }
}
