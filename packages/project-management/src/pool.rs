//! Pool de ligações ao catálogo para o trabalho fora da ligação principal (imagens
//! servidas à interface, edições, exportação).
//!
//! Medido na Fase 12 (`perf`): abrir uma ligação SQLite por pedido (ler o esquema,
//! PRAGMAs, fechar) era 72 % do tempo de servir uma miniatura em cache. As ligações
//! devolvidas ficam disponíveis para o pedido seguinte.

use momento_core_types::CoreResult;
use momento_database::Catalog;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Ligações inativas guardadas no máximo (o resto é fechado).
const MAX_IDLE: usize = 8;

pub struct CatalogPool {
    path: PathBuf,
    idle: Mutex<Vec<Catalog>>,
}

impl CatalogPool {
    pub fn new(path: PathBuf) -> Arc<Self> {
        Arc::new(CatalogPool { path, idle: Mutex::new(Vec::new()) })
    }

    pub fn get(self: &Arc<Self>) -> CoreResult<PooledCatalog> {
        let cached = self.idle.lock().unwrap_or_else(|e| e.into_inner()).pop();
        let cat = match cached {
            Some(c) => c,
            None => Catalog::open_existing(&self.path)?,
        };
        Ok(PooledCatalog { cat: Some(cat), pool: self.clone() })
    }

    /// Ligações inativas (testes).
    pub fn idle(&self) -> usize {
        self.idle.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

/// Ligação emprestada; volta ao pool quando sai de âmbito.
pub struct PooledCatalog {
    cat: Option<Catalog>,
    pool: Arc<CatalogPool>,
}

impl Deref for PooledCatalog {
    type Target = Catalog;
    fn deref(&self) -> &Catalog {
        self.cat.as_ref().expect("ligação presente até ao drop")
    }
}

impl DerefMut for PooledCatalog {
    fn deref_mut(&mut self) -> &mut Catalog {
        self.cat.as_mut().expect("ligação presente até ao drop")
    }
}

impl Drop for PooledCatalog {
    fn drop(&mut self) {
        if let Some(c) = self.cat.take() {
            // Uma ligação a meio de uma transação (pânico) não volta ao pool.
            if c.connection().is_autocommit() {
                let mut idle = self.pool.idle.lock().unwrap_or_else(|e| e.into_inner());
                if idle.len() < MAX_IDLE {
                    idle.push(c);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_connections_up_to_a_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        drop(Catalog::open(&path).unwrap());
        let pool = CatalogPool::new(path);
        {
            let a = pool.get().unwrap();
            let b = pool.get().unwrap();
            assert!(a.schema_version().unwrap() > 0 && b.schema_version().unwrap() > 0);
            assert_eq!(pool.idle(), 0);
        }
        assert_eq!(pool.idle(), 2);
        let many: Vec<_> = (0..12).map(|_| pool.get().unwrap()).collect();
        assert_eq!(pool.idle(), 0);
        drop(many);
        assert_eq!(pool.idle(), MAX_IDLE);
        // Uma ligação largada a meio de uma transação é fechada, não reutilizada.
        let taken: Vec<_> = (0..MAX_IDLE).map(|_| pool.get().unwrap()).collect();
        assert_eq!(pool.idle(), 0);
        let c = pool.get().unwrap();
        c.connection().execute_batch("BEGIN").unwrap();
        drop(c);
        assert_eq!(pool.idle(), 0, "a ligação com transação aberta não volta");
        drop(taken);
        assert_eq!(pool.idle(), MAX_IDLE);
    }
}
