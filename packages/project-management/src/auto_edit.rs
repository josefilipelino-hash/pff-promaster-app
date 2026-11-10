//! Edição automática (Fase 6): uma fotografia a pedido, ou centenas em segundo plano numa
//! fila com progresso, pausa, retoma e cancelamento.
//!
//! Cada fotografia é analisada individualmente ([`develop::auto::auto_edit`]); o resultado
//! é gravado como uma entrada normal do histórico ("auto") — não destrutivo e reversível
//! com desfazer. Os rostos e as íris (olhos vermelhos, exposição dos rostos, recorte) vêm
//! dos modelos locais; sem eles, a edição continua sem essa informação.
//!
//! O **AI Denoise** é pesado (≈ 3 s por megapíxel em 4 núcleos): no editor corre em
//! segundo plano (a pré-visualização usa a fonte sem ele até estar pronto) e na fila é
//! calculado com a fotografia, para a miniatura editada já o incluir.

use crate::edit::{self, AiKey};
use crate::session::{EditView, ProjectSession};
use image::RgbImage;
use momento_core_types::{AssetId, CoreError, CoreResult};
use momento_database::Catalog;
use momento_face_analysis::{Denoiser, VisionModels};
use momento_image_codecs::{format_from_id, FormatFamily};
use momento_image_processing::develop::auto::{
    auto_edit, AutoContext, AutoOptions, AutoReport, FaceBox,
};
use momento_image_processing::develop::consistency::{match_reference, MatchOptions, MatchReport};
use momento_image_processing::develop::{self, Quality, RedEye, Settings, Source};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

/// Etiqueta do histórico para a edição automática.
pub const AUTO_LABEL: &str = "auto";
/// Etiqueta do histórico para a sincronização com uma referência.
pub const SYNC_LABEL: &str = "sync";
/// Lado maior da imagem usada para encontrar rostos e íris.
const FACE_EDGE: u32 = 1600;

/// Progresso da fila (evento `auto-edit-progress`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoEditProgress {
    /// Tipo de trabalho: `auto` (edição automática) ou `sync` (sincronizar com referência).
    pub kind: String,
    pub total: u64,
    pub done: u64,
    pub failed: u64,
    pub paused: bool,
    pub cancelled: bool,
    /// A fila terminou (ou foi cancelada) e não há mais trabalho.
    pub finished: bool,
    /// Ficheiro em processamento.
    pub current: Option<String>,
}

/// Progresso do AI Denoise de uma fotografia no editor (evento `ai-denoise`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDenoiseProgress {
    pub key: i64,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
}

/// Resultado da edição automática de uma fotografia.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoEditResult {
    pub view: EditView,
    pub report: AutoReport,
}

#[derive(Default)]
struct Ctl {
    paused: bool,
    cancelled: bool,
}

/// Pausa/retoma/cancelamento partilhados pelos trabalhadores.
#[derive(Default)]
pub struct Control {
    st: Mutex<Ctl>,
    cv: Condvar,
}

impl Control {
    /// Espera enquanto estiver em pausa; `false` se foi cancelado.
    pub fn checkpoint(&self) -> bool {
        let mut g = self.st.lock().unwrap_or_else(|e| e.into_inner());
        while g.paused && !g.cancelled {
            g = self.cv.wait(g).unwrap_or_else(|e| e.into_inner());
        }
        !g.cancelled
    }
    fn set(&self, f: impl FnOnce(&mut Ctl)) {
        f(&mut self.st.lock().unwrap_or_else(|e| e.into_inner()));
        self.cv.notify_all();
    }
    fn state(&self) -> (bool, bool) {
        let g = self.st.lock().unwrap_or_else(|e| e.into_inner());
        (g.paused, g.cancelled)
    }
}

/// Fila em curso.
pub struct AutoEditJob {
    control: Arc<Control>,
    queue: Arc<Mutex<VecDeque<i64>>>,
    total: Arc<AtomicU64>,
    done: Arc<AtomicU64>,
    failed: Arc<AtomicU64>,
    /// Trabalhadores ainda ativos.
    active: Arc<AtomicU64>,
    task: BatchTask,
    handles: Vec<JoinHandle<()>>,
}

/// Trabalho de uma fila em segundo plano.
#[derive(Debug, Clone, PartialEq)]
pub enum BatchTask {
    Auto(AutoOptions),
    /// Sincronizar com uma fotografia de referência (ajustes da referência no início).
    Sync {
        reference: i64,
        settings: Box<Settings>,
        options: MatchOptions,
    },
}

impl BatchTask {
    fn kind(&self) -> &'static str {
        match self {
            BatchTask::Auto(_) => "auto",
            BatchTask::Sync { .. } => "sync",
        }
    }
}

impl AutoEditJob {
    fn is_finished(&self) -> bool {
        self.active.load(Ordering::SeqCst) == 0
    }
}

/// Retângulos dos rostos e íris (coordenadas normalizadas da fotografia orientada).
fn faces_and_eyes(models: &VisionModels, src: &Source) -> (Vec<FaceBox>, Vec<RedEye>) {
    let img: RgbImage = develop::render_rgb(src, &Settings::default(), FACE_EDGE, Quality::Fast);
    let (w, h) = (img.width() as f32, img.height() as f32);
    let long = w.max(h);
    let Ok(found) = models.analyze_faces(&img, true) else { return (vec![], vec![]) };
    let mut faces = Vec::new();
    let mut eyes = Vec::new();
    for f in &found {
        let d = &f.detection;
        faces.push(FaceBox { x: d.x / w, y: d.y / h, w: d.w / w, h: d.h / h });
        let l = &f.landmarks;
        // Íris (pontos 468–477 do MediaPipe): centro e raio pelo contorno.
        for (c, edge) in [(468usize, 469usize), (473, 474)] {
            if let (Some(p), Some(q)) = (l.get(c), l.get(edge)) {
                let r = ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
                if r >= 1.5 {
                    eyes.push(RedEye { x: p.0 / w, y: p.1 / h, r: r / long });
                }
            }
        }
    }
    (faces, eyes)
}

/// Tempo máximo à espera de um AI Denoise igual já em curso.
const AI_WAIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(600);

/// Marca "em curso" que se limpa sempre — mesmo se o cálculo entrar em pânico. Sem isto, um
/// pânico deixava a marca para sempre e qualquer pedido seguinte da mesma fotografia
/// (preview, exportação) ficava à espera indefinidamente (Fase 15).
struct InflightGuard<'a> {
    session: &'a ProjectSession,
    ak: AiKey,
}

impl<'a> InflightGuard<'a> {
    fn new(session: &'a ProjectSession, ak: AiKey) -> Self {
        session.ai_inflight_set(ak, true);
        InflightGuard { session, ak }
    }
}

impl Drop for InflightGuard<'_> {
    fn drop(&mut self) {
        self.session.ai_inflight_set(self.ak, false);
    }
}

impl ProjectSession {
    pub(crate) fn denoiser(&self) -> Option<Arc<Denoiser>> {
        self.denoiser
            .get_or_init(|| {
                let dir = crate::session::models_dir()?;
                match Denoiser::load(&dir) {
                    Ok(d) => d.map(Arc::new),
                    Err(e) => {
                        tracing::warn!(erro = %e, "AI Denoise indisponível");
                        None
                    }
                }
            })
            .clone()
    }

    /// O modelo do AI Denoise existe neste computador.
    pub fn ai_denoise_available(&self) -> bool {
        self.denoiser().is_some()
    }

    /// Fonte de edição com o AI Denoise da receita (se houver). `blocking = false`: se
    /// ainda não estiver calculado, devolve a fonte sem ele, pede o cálculo em segundo
    /// plano e indica `true` (pendente).
    pub(crate) fn source_for(
        self: &Arc<Self>,
        cat: &Catalog,
        asset: AssetId,
        settings: &Settings,
        full: bool,
        blocking: bool,
        key: i64,
    ) -> CoreResult<(Arc<Source>, bool)> {
        let base = if full {
            Arc::new(edit::load_source(cat, self.registry(), asset, None)?)
        } else {
            self.edits.source(cat, self.registry(), asset)?
        };
        let strength = settings.detail.ai_denoise;
        let Some(dn) = (strength > 0.0).then(|| self.denoiser()).flatten() else {
            return Ok((base, false));
        };
        let ak: AiKey = (asset, full, strength.round() as u32);
        if let Some(s) = self.edits.ai_cached(ak) {
            return Ok((s, false));
        }
        if blocking {
            // O mesmo cálculo já em curso (editor ou outra miniatura): espera por ele.
            // Com limite: se o outro cálculo nunca terminar, calcula-se aqui (Fase 15 —
            // antes esperava para sempre, com a vez da preview ocupada).
            let waited = std::time::Instant::now();
            while self.ai_inflight_has(ak) && waited.elapsed() < AI_WAIT_LIMIT {
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            if let Some(s) = self.edits.ai_cached(ak) {
                return Ok((s, false));
            }
            let t = std::time::Instant::now();
            let res = {
                let _g = InflightGuard::new(self, ak);
                edit::ai_denoised(&base, strength, &dn, &|_, _| true)
            };
            let out = res?.map(Arc::new).unwrap_or(base);
            tracing::info!(asset = asset.0, total = full, tempo = ?t.elapsed(), "AI Denoise");
            self.edits.ai_store(ak, out.clone());
            self.events.ai_denoise(&AiDenoiseProgress { key, done: 1, total: 1, finished: true });
            return Ok((out, false));
        }
        if !self.ai_inflight_has(ak) {
            self.request_ai(key, ak, base.clone(), dn);
        }
        Ok((base, true))
    }

    fn ai_inflight_has(&self, ak: AiKey) -> bool {
        self.ai_inflight.lock().unwrap_or_else(|e| e.into_inner()).contains(&ak)
    }

    fn ai_inflight_set(&self, ak: AiKey, on: bool) {
        let mut g = self.ai_inflight.lock().unwrap_or_else(|e| e.into_inner());
        if on {
            g.insert(ak);
        } else {
            g.remove(&ak);
        }
    }

    /// Pede o AI Denoise em segundo plano (só o pedido mais recente é feito).
    fn request_ai(self: &Arc<Self>, key: i64, ak: AiKey, base: Arc<Source>, dn: Arc<Denoiser>) {
        let mut slot = self.ai_next.lock().unwrap_or_else(|e| e.into_inner());
        let start = slot.is_none() && !self.ai_busy.load(Ordering::SeqCst);
        let same = slot.as_ref().is_some_and(|s| s.1 == ak);
        if !same {
            *slot = Some((key, ak, base));
        }
        drop(slot);
        if !start || self.ai_busy.swap(true, Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || loop {
            let job = me.ai_next.lock().unwrap_or_else(|e| e.into_inner()).take();
            let Some((key, ak, base)) = job else {
                me.ai_busy.store(false, Ordering::SeqCst);
                // Um pedido pode ter chegado entre o `take` e o `store`.
                if me.ai_next.lock().unwrap_or_else(|e| e.into_inner()).is_some()
                    && !me.ai_busy.swap(true, Ordering::SeqCst)
                {
                    continue;
                }
                return;
            };
            if me.edits.ai_cached(ak).is_some() {
                continue;
            }
            let ev = me.events.clone();
            // Só um pedido **diferente** (outra intensidade/fotografia) cancela este.
            let newer = || {
                me.ai_next
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .as_ref()
                    .is_some_and(|n| n.1 != ak)
            };
            let _g = InflightGuard::new(&me, ak);
            let progress = |d: usize, t: usize| {
                ev.ai_denoise(&AiDenoiseProgress {
                    key,
                    done: d as u64,
                    total: t as u64,
                    finished: false,
                });
                // Um pedido mais recente (outra intensidade/fotografia) cancela este.
                !newer()
            };
            let res = edit::ai_denoised(&base, ak.2 as f32, &dn, &progress);
            drop(_g);
            match res {
                Ok(Some(s)) => {
                    me.edits.ai_store(ak, Arc::new(s));
                    ev.ai_denoise(&AiDenoiseProgress { key, done: 1, total: 1, finished: true });
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(erro = %e, "AI Denoise falhou"),
            }
        });
    }

    /// Contexto da edição automática: ISO, câmara, formato, rostos e íris.
    fn auto_context(
        &self,
        cat: &Catalog,
        asset: AssetId,
        src: &Source,
        models: Option<&VisionModels>,
    ) -> CoreResult<AutoContext> {
        let d = cat
            .photo_details(asset)?
            .ok_or_else(|| CoreError::Other(format!("fotografia {asset} inexistente")))?;
        let is_raw = format_from_id(&d.format).is_some_and(|f| f.family() == FormatFamily::Raw);
        let (faces, eyes) = models.map(|m| faces_and_eyes(m, src)).unwrap_or_default();
        Ok(AutoContext {
            iso: d.iso,
            make: d.camera_make,
            model: d.camera_model,
            is_raw,
            faces,
            eyes,
        })
    }

    /// Analisa e grava a edição automática de uma fotografia (nova entrada no histórico).
    fn auto_edit_one(
        self: &Arc<Self>,
        cat: &mut Catalog,
        key: i64,
        options: &AutoOptions,
        models: Option<&VisionModels>,
        batch: bool,
    ) -> CoreResult<AutoReport> {
        let (variant, asset) = cat.resolve_key(key)?;
        let base_recipe = cat.recipe_of(variant)?;
        let base = Settings::from_recipe(&base_recipe);
        let src = if batch {
            Arc::new(edit::load_source(cat, self.registry(), asset, Some(edit::EDIT_EDGE))?)
        } else {
            self.edits.source(cat, self.registry(), asset)?
        };
        let ctx = self.auto_context(cat, asset, &src, models)?;
        let (mut settings, report) = auto_edit(&src, &base, &ctx, options);
        if settings.detail.ai_denoise > 0.0 && self.denoiser().is_none() {
            // Sem o modelo: compensa com a redução rápida.
            settings.detail.ai_denoise = 0.0;
            settings.detail.noise_luminance = (settings.detail.noise_luminance * 4.0).min(70.0);
        }
        let recipe = settings.to_recipe(&base_recipe);
        // A análise demora segundos: se o utilizador editou esta fotografia entretanto, o
        // resultado automático NÃO é gravado por cima (antes as edições dele eram
        // revertidas em silêncio — Fase 15).
        if cat.recipe_of(variant)? != base_recipe {
            return Err(CoreError::Other(
                "a fotografia foi editada durante a edição automática; não foi alterada".into(),
            ));
        }
        if recipe != base_recipe {
            cat.commit_edit(variant, AUTO_LABEL, &recipe)?;
        }
        Ok(report)
    }

    /// Edição automática da fotografia aberta no editor.
    pub fn auto_edit_photo(
        self: &Arc<Self>,
        key: i64,
        options: &AutoOptions,
    ) -> CoreResult<AutoEditResult> {
        let models = crate::session::models_dir().and_then(|d| VisionModels::load(&d).ok());
        let mut cat = self.open_catalog()?;
        let report = self.auto_edit_one(&mut cat, key, options, models.as_ref(), false)?;
        self.after_edit(vec![key]);
        Ok(AutoEditResult { view: self.edit_view(key)?, report })
    }

    /// Só a análise (sem gravar): o que a edição automática faria.
    pub fn auto_edit_preview(
        self: &Arc<Self>,
        key: i64,
        options: &AutoOptions,
    ) -> CoreResult<(Settings, AutoReport)> {
        let cat = self.open_catalog()?;
        let (variant, asset) = cat.resolve_key(key)?;
        let base = Settings::from_recipe(&cat.recipe_of(variant)?);
        let src = self.edits.source(&cat, self.registry(), asset)?;
        let models = crate::session::models_dir().and_then(|d| VisionModels::load(&d).ok());
        let ctx = self.auto_context(&cat, asset, &src, models.as_ref())?;
        Ok(auto_edit(&src, &base, &ctx, options))
    }

    // ------------------------------------------------------------------ fila

    pub fn auto_edit_running(&self) -> bool {
        self.auto_job
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|j| !j.is_finished())
    }

    fn auto_progress(&self, current: Option<String>, finished: bool) -> AutoEditProgress {
        let g = self.auto_job.lock().unwrap_or_else(|e| e.into_inner());
        match g.as_ref() {
            Some(j) => {
                let (paused, cancelled) = j.control.state();
                AutoEditProgress {
                    kind: j.task.kind().into(),
                    total: j.total.load(Ordering::SeqCst),
                    done: j.done.load(Ordering::SeqCst),
                    failed: j.failed.load(Ordering::SeqCst),
                    paused,
                    cancelled,
                    finished,
                    current,
                }
            }
            None => AutoEditProgress { kind: "auto".into(), finished: true, ..Default::default() },
        }
    }

    /// Estado atual da fila (para a interface ao abrir o projeto).
    pub fn auto_edit_status(&self) -> AutoEditProgress {
        let running = self.auto_edit_running();
        self.auto_progress(None, !running)
    }

    /// "Editar automaticamente seleção": processa as fotografias em segundo plano. Se já
    /// houver uma fila com as mesmas opções, acrescenta-as. Devolve o total da fila.
    pub fn start_auto_edit(
        self: &Arc<Self>,
        keys: Vec<i64>,
        options: AutoOptions,
    ) -> CoreResult<u64> {
        self.start_batch(keys, BatchTask::Auto(options))
    }

    /// Sincronizar fotografias com uma referência (fila em segundo plano): cada uma recebe
    /// o aspeto da referência e é ajustada até o resultado coincidir (ver
    /// [`develop::consistency`]).
    pub fn start_sync(
        self: &Arc<Self>,
        reference: i64,
        keys: Vec<i64>,
        options: MatchOptions,
    ) -> CoreResult<u64> {
        let keys: Vec<i64> = keys.into_iter().filter(|&k| k != reference).collect();
        let (variant, _) = self.open_catalog()?.resolve_key(reference)?;
        let settings = Settings::from_recipe(&self.open_catalog()?.recipe_of(variant)?);
        self.start_batch(keys, BatchTask::Sync { reference, settings: Box::new(settings), options })
    }

    fn start_batch(self: &Arc<Self>, keys: Vec<i64>, task: BatchTask) -> CoreResult<u64> {
        if keys.is_empty() {
            return Err(CoreError::Other("nenhuma fotografia selecionada".into()));
        }
        let mut g = self.auto_job.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(j) = g.as_ref() {
            if !j.is_finished() {
                if j.task != task {
                    return Err(CoreError::Other(
                        "já existe um trabalho em curso com outras opções".into(),
                    ));
                }
                let mut q = j.queue.lock().unwrap_or_else(|e| e.into_inner());
                q.extend(keys.iter().copied());
                return Ok(
                    j.total.fetch_add(keys.len() as u64, Ordering::SeqCst) + keys.len() as u64
                );
            }
        }
        let job = AutoEditJob {
            control: Arc::new(Control::default()),
            queue: Arc::new(Mutex::new(keys.iter().copied().collect())),
            total: Arc::new(AtomicU64::new(keys.len() as u64)),
            done: Arc::new(AtomicU64::new(0)),
            failed: Arc::new(AtomicU64::new(0)),
            active: Arc::new(AtomicU64::new(0)),
            task: task.clone(),
            handles: Vec::new(),
        };
        let workers =
            (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) / 3).clamp(1, 2);
        let mut handles = Vec::new();
        for _ in 0..workers {
            let me = self.clone();
            let (ctl, q, done, failed, active) = (
                job.control.clone(),
                job.queue.clone(),
                job.done.clone(),
                job.failed.clone(),
                job.active.clone(),
            );
            let task = task.clone();
            active.fetch_add(1, Ordering::SeqCst);
            handles.push(std::thread::spawn(move || {
                me.batch_worker(&ctl, &q, &done, &failed, &task);
                // O último trabalhador a sair anuncia o fim.
                if active.fetch_sub(1, Ordering::SeqCst) == 1 {
                    me.events.auto_edit(&me.auto_progress(None, true));
                }
            }));
        }
        let total = job.total.load(Ordering::SeqCst);
        *g = Some(AutoEditJob { handles, ..job });
        drop(g);
        self.events.auto_edit(&self.auto_progress(None, false));
        Ok(total)
    }

    fn batch_worker(
        self: &Arc<Self>,
        ctl: &Control,
        q: &Mutex<VecDeque<i64>>,
        done: &AtomicU64,
        failed: &AtomicU64,
        task: &BatchTask,
    ) {
        // Referência carregada uma vez por trabalhador.
        let mut reference: Option<(Arc<Source>, Vec<FaceBox>)> = None;
        let models = crate::session::models_dir().and_then(|d| VisionModels::load(&d).ok());
        let mut cat = match self.open_catalog() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(erro = %e, "edição automática: catálogo");
                return;
            }
        };
        loop {
            if !ctl.checkpoint() {
                break;
            }
            let Some(key) = q.lock().unwrap_or_else(|e| e.into_inner()).pop_front() else { break };
            momento_task_queue::foreground::yield_to_foreground();
            let name = cat
                .resolve_key(key)
                .ok()
                .and_then(|(_, a)| cat.photo_details(a).ok().flatten())
                .map(|d| d.file_name);
            self.events.auto_edit(&self.auto_progress(name, false));
            let result = match task {
                BatchTask::Auto(opts) => {
                    self.auto_edit_one(&mut cat, key, opts, models.as_ref(), true).map(|_| ())
                }
                BatchTask::Sync { reference: rk, settings, options } => {
                    if reference.is_none() {
                        match self.load_for_match(&cat, *rk) {
                            Ok(r) => reference = Some(r),
                            Err(e) => {
                                tracing::warn!(erro = %e, "referência da sincronização");
                                failed.fetch_add(1, Ordering::SeqCst);
                                continue;
                            }
                        }
                    }
                    let (rs, rf) = reference.as_ref().expect("referência");
                    self.sync_one(&mut cat, key, rs, settings, rf, options).map(|_| ())
                }
            };
            match result {
                Ok(_) => {
                    // Miniatura editada já com tudo (incluindo o AI Denoise), sidecar e aviso.
                    if let Err(e) = self.refresh_edited(&cat, key) {
                        tracing::warn!(erro = %e, "miniatura editada não gerada");
                    }
                    self.events.edits_changed(&[key]);
                    done.fetch_add(1, Ordering::SeqCst);
                }
                Err(e) => {
                    tracing::warn!(chave = key, erro = %e, "trabalho em lote falhou");
                    failed.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
    }

    /// Fonte de edição e rostos (catálogo da análise) de uma fotografia.
    fn load_for_match(&self, cat: &Catalog, key: i64) -> CoreResult<(Arc<Source>, Vec<FaceBox>)> {
        let (_, asset) = cat.resolve_key(key)?;
        let src = Arc::new(edit::load_source(cat, self.registry(), asset, Some(edit::EDIT_EDGE))?);
        let faces = cat
            .face_boxes(asset)?
            .into_iter()
            .map(|b| FaceBox { x: b[0], y: b[1], w: b[2], h: b[3] })
            .collect();
        Ok((src, faces))
    }

    fn sync_one(
        self: &Arc<Self>,
        cat: &mut Catalog,
        key: i64,
        ref_src: &Source,
        ref_settings: &Settings,
        ref_faces: &[FaceBox],
        options: &MatchOptions,
    ) -> CoreResult<MatchReport> {
        let (variant, _) = cat.resolve_key(key)?;
        let base_recipe = cat.recipe_of(variant)?;
        let base = Settings::from_recipe(&base_recipe);
        let (src, faces) = self.load_for_match(cat, key)?;
        let (s, rep) =
            match_reference(ref_src, ref_settings, ref_faces, &src, &base, &faces, options);
        let recipe = s.to_recipe(&base_recipe);
        if recipe != base_recipe {
            cat.commit_edit(variant, SYNC_LABEL, &recipe)?;
        }
        Ok(rep)
    }

    /// Sincronização imediata de uma fotografia (testes e pré-visualização).
    pub fn sync_photo(
        self: &Arc<Self>,
        reference: i64,
        key: i64,
        options: &MatchOptions,
    ) -> CoreResult<MatchReport> {
        let mut cat = self.open_catalog()?;
        let (rv, _) = cat.resolve_key(reference)?;
        let rs = Settings::from_recipe(&cat.recipe_of(rv)?);
        let (src, faces) = self.load_for_match(&cat, reference)?;
        let rep = self.sync_one(&mut cat, key, &src, &rs, &faces, options)?;
        self.after_edit(vec![key]);
        Ok(rep)
    }

    /// Sidecar + miniatura editada de uma chave (bloqueante).
    pub(crate) fn refresh_edited(self: &Arc<Self>, cat: &Catalog, key: i64) -> CoreResult<()> {
        let (variant, asset) = cat.resolve_key(key)?;
        let recipe = cat.recipe_of(variant)?;
        edit::write_sidecar(&self.paths, cat, asset, variant, &recipe)?;
        if recipe.is_identity() {
            return Ok(());
        }
        let settings = Settings::from_recipe(&recipe);
        let src = |edge: Option<u32>| -> CoreResult<Arc<Source>> {
            self.source_for(cat, asset, &settings, edge.is_none(), true, key).map(|s| s.0)
        };
        edit::ensure_edited(&self.paths, &src, variant, &recipe, "thumbs").map(|_| ())
    }

    pub fn pause_auto_edit(&self) {
        if let Some(j) = self.auto_job.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            j.control.set(|c| c.paused = true);
        } else {
            return;
        }
        self.events.auto_edit(&self.auto_progress(None, !self.auto_edit_running()));
    }

    pub fn resume_auto_edit(&self) {
        if let Some(j) = self.auto_job.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            j.control.set(|c| c.paused = false);
        } else {
            return;
        }
        self.events.auto_edit(&self.auto_progress(None, !self.auto_edit_running()));
    }

    /// Cancela: as fotografias já editadas ficam (cada uma pode ser desfeita); as restantes
    /// não são tocadas.
    pub fn cancel_auto_edit(&self) {
        if let Some(j) = self.auto_job.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            j.control.set(|c| c.cancelled = true);
            j.queue.lock().unwrap_or_else(|e| e.into_inner()).clear();
        } else {
            // Sem fila (p. ex. ao fechar o projeto): nada a anunciar — antes surgia
            // "Edição automática concluída: 0 fotografias".
            return;
        }
        self.events.auto_edit(&self.auto_progress(None, false));
    }

    /// Espera pelo fim da fila (testes e fecho do projeto).
    pub fn wait_auto_edit(&self) {
        let handles: Vec<JoinHandle<()>> = self
            .auto_job
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
            .map(|j| std::mem::take(&mut j.handles))
            .unwrap_or_default();
        for h in handles {
            let _ = h.join();
        }
    }
}
