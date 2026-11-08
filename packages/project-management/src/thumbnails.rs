//! Serviço de miniaturas em segundo plano.
//!
//! N threads retiram trabalho de uma [`WorkQueue`] (com prioridade para o que está
//! visível) e uma thread grava os resultados no catálogo em lotes. O estado de cada
//! miniatura fica em `asset.thumb_state`, por isso o trabalho por fazer é retomado na
//! próxima abertura do projeto.

use crate::paths::{CacheLevel, ProjectPaths};
use momento_core_types::{AssetId, CoreError, CoreResult};
use momento_database::assets::ThumbState;
use momento_database::Catalog;
use momento_image_codecs::{format_from_id, CodecRegistry};
use momento_image_processing::{render_preview, write_atomic, PreviewSpec};
use momento_task_queue::WorkQueue;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Lote de resultados enviado à interface.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbBatch {
    pub ready: Vec<AssetId>,
    pub failed: Vec<AssetId>,
    /// Miniaturas por gerar no momento do envio.
    pub remaining: u64,
    pub done: u64,
    pub total: u64,
}

pub type ThumbCallback = Arc<dyn Fn(ThumbBatch) + Send + Sync>;

pub struct ThumbnailService {
    queue: Arc<WorkQueue<AssetId>>,
    workers: Vec<JoinHandle<()>>,
    writer: Option<JoinHandle<()>>,
    total: Arc<AtomicU64>,
    done: Arc<AtomicU64>,
}

fn generate(
    paths: &ProjectPaths,
    catalog: &Catalog,
    registry: &CodecRegistry,
    id: AssetId,
) -> CoreResult<()> {
    let src =
        catalog.asset_source(id)?.ok_or_else(|| CoreError::Other("fotografia removida".into()))?;
    let format = format_from_id(&src.format)
        .ok_or_else(|| CoreError::UnsupportedFormat(src.format.clone()))?;
    let data = std::fs::read(&src.path)
        .map_err(|e| CoreError::Other(format!("original não encontrado ({e})")))?;
    let jpg = render_preview(registry, &data, format, src.orientation, PreviewSpec::THUMB)?;
    write_atomic(&paths.cached(CacheLevel::Thumb, id), &jpg)
}

impl ThumbnailService {
    pub fn start(
        paths: ProjectPaths,
        registry: Arc<CodecRegistry>,
        threads: usize,
        on_batch: ThumbCallback,
    ) -> CoreResult<Self> {
        let queue = Arc::new(WorkQueue::new());
        let total = Arc::new(AtomicU64::new(0));
        let done = Arc::new(AtomicU64::new(0));
        let (tx, rx) = mpsc::channel::<(AssetId, Result<(), String>)>();

        let mut workers = Vec::new();
        for i in 0..threads.max(1) {
            let (queue, paths, registry, tx) =
                (queue.clone(), paths.clone(), registry.clone(), tx.clone());
            let catalog = Catalog::open_existing(&paths.catalog())?;
            workers.push(
                std::thread::Builder::new()
                    .name(format!("thumb-{i}"))
                    .spawn(move || {
                        while let Some(id) = queue.pop() {
                            // Um pânico num descodificador não pode matar a thread nem a aplicação.
                            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                generate(&paths, &catalog, &registry, id)
                            }));
                            let r = match r {
                                Ok(Ok(())) => Ok(()),
                                Ok(Err(e)) => Err(e.to_string()),
                                Err(_) => Err("falha interna do descodificador".to_string()),
                            };
                            if tx.send((id, r)).is_err() {
                                break;
                            }
                        }
                    })
                    .map_err(CoreError::Io)?,
            );
        }
        drop(tx);

        let mut catalog = Catalog::open_existing(&paths.catalog())?;
        let (wq, wt, wd) = (queue.clone(), total.clone(), done.clone());
        let writer = std::thread::Builder::new()
            .name("thumb-writer".into())
            .spawn(move || {
                let mut pending: Vec<(AssetId, ThumbState, Option<String>)> = Vec::new();
                let mut last = Instant::now();
                loop {
                    let msg = rx.recv_timeout(Duration::from_millis(150));
                    let disconnected = matches!(msg, Err(mpsc::RecvTimeoutError::Disconnected));
                    if let Ok((id, r)) = msg {
                        match r {
                            Ok(()) => pending.push((id, ThumbState::Ready, None)),
                            Err(e) => {
                                tracing::warn!(asset = id.0, erro = %e, "miniatura falhou");
                                pending.push((id, ThumbState::Failed, Some(e)))
                            }
                        }
                    }
                    let due = !pending.is_empty()
                        && (pending.len() >= 48 || last.elapsed() >= Duration::from_millis(150));
                    if due || (disconnected && !pending.is_empty()) {
                        if let Err(e) = catalog.set_thumb_states(&pending) {
                            tracing::error!(erro = %e, "falha ao gravar estado das miniaturas");
                        }
                        let n = pending.len() as u64;
                        let d = wd.fetch_add(n, Ordering::SeqCst) + n;
                        let mut batch = ThumbBatch {
                            remaining: wq.len() as u64,
                            done: d,
                            total: wt.load(Ordering::SeqCst),
                            ..Default::default()
                        };
                        for (id, s, _) in pending.drain(..) {
                            if s == ThumbState::Ready {
                                batch.ready.push(id)
                            } else {
                                batch.failed.push(id)
                            }
                        }
                        on_batch(batch);
                        last = Instant::now();
                    }
                    if disconnected {
                        break;
                    }
                }
            })
            .map_err(CoreError::Io)?;

        Ok(ThumbnailService { queue, workers, writer: Some(writer), total, done })
    }

    pub fn enqueue(&self, ids: impl IntoIterator<Item = AssetId>) {
        let n = self.queue.push(ids);
        self.total.fetch_add(n as u64, Ordering::SeqCst);
    }

    /// Miniaturas visíveis no ecrã passam à frente.
    pub fn prioritize(&self, ids: impl IntoIterator<Item = AssetId>) {
        self.queue.prioritize(ids);
    }

    pub fn remaining(&self) -> usize {
        self.queue.len()
    }

    pub fn progress(&self) -> (u64, u64) {
        (self.done.load(Ordering::SeqCst), self.total.load(Ordering::SeqCst))
    }

    /// Termina as threads. O trabalho por fazer fica `pending` no catálogo.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.queue.close();
        for w in self.workers.drain(..) {
            let _ = w.join();
        }
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
    }
}

impl Drop for ThumbnailService {
    fn drop(&mut self) {
        self.stop();
    }
}
