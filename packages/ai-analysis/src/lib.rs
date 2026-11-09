//! # momento-ai-analysis
//!
//! Tudo o que não precisa de redes neuronais e é determinístico e explicável:
//!
//! - [`metrics`]: nitidez (global, pico de foco, rosto, olhos), desfocagem de movimento,
//!   exposição, realces queimados, sombras bloqueadas, ruído, contraste, horizonte;
//! - [`hash`]: hashes percetuais (pHash DCT e dHash) para duplicados e semelhança;
//! - [`selection`]: agrupamento em sequências/duplicados/pares RAW+JPEG, pontuação por
//!   critério, etiquetas (Melhor, Alternativa, Rever, Rejeitar sugerido) e explicações.
//!
//! Os modelos de visão (rostos, olhos, expressões, embeddings) estão em
//! `momento-face-analysis`; este crate recebe os resultados já calculados.

pub mod hash;
pub mod metrics;
pub mod selection;
