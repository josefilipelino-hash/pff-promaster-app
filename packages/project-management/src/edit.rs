//! Serviço de edição: fonte em memória (RAW linear ou outros formatos), render para a
//! interface com qualidade dinâmica, gravação não destrutiva (catálogo + sidecar JSON na
//! pasta do projeto) e miniaturas/previews editadas em cache.
//!
//! Os ficheiros originais são abertos **só para leitura**.

use crate::paths::ProjectPaths;
use image::{imageops, RgbImage};
use momento_core_types::recipe::EditRecipe;
use momento_core_types::{AssetId, CoreError, CoreResult, VariantId};
use momento_database::Catalog;
use momento_face_analysis::Denoiser;
use momento_image_codecs::{format_from_id, CodecRegistry, FormatFamily};
use momento_image_processing::develop::color::{eotf, oetf};
use momento_image_processing::develop::source::Plane;
use momento_image_processing::develop::{self, Quality, RenderOptions, Settings, Source};
use momento_image_processing::{apply_orientation, encode_jpeg, write_atomic};
use momento_raw_engine::RawFile;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Lado maior da fonte de edição para formatos não RAW (os RAW usam meia resolução do sensor).
pub const EDIT_EDGE: u32 = 3072;
/// Fontes em memória (≈ 70–110 MB cada).
const CACHE_ENTRIES: usize = 3;

pub const THUMB_EDGE: u32 = 640;
pub const PREVIEW_EDGE: u32 = 2560;

/// Carrega a fonte de edição de um asset. `max_edge = None` → resolução total.
pub fn load_source(
    catalog: &Catalog,
    registry: &CodecRegistry,
    asset: AssetId,
    max_edge: Option<u32>,
) -> CoreResult<Source> {
    let src = catalog
        .asset_source(asset)?
        .ok_or_else(|| CoreError::Other(format!("fotografia {asset} inexistente")))?;
    let format = format_from_id(&src.format)
        .ok_or_else(|| CoreError::UnsupportedFormat(src.format.clone()))?;
    let data = crate::availability::read_original(&src.path)?;
    if format.family() == FormatFamily::Raw {
        match RawFile::open(&data).and_then(|mut raw| {
            let info = raw.info().clone();
            let long = info.width.max(info.height);
            let half = max_edge.is_some_and(|m| long / 2 >= m.min(long));
            let bm = raw.develop_linear(half)?;
            let mut s = Source::from_linear16(
                bm.width,
                bm.height,
                &bm.rgb,
                src.orientation,
                info.width,
                info.height,
            );
            // Temperatura do balanço da câmara: a temperatura passa a ser em Kelvin.
            s.as_shot_k = info.as_shot_white.and_then(develop::color::kelvin_of_white);
            // Câmara que esta LibRaw não conhece (sem matriz de cor, ex.: Canon EOS C50):
            // calibrar pela preview JPEG que a própria câmara gravou no ficheiro.
            // Sem calibração fiável, o erro leva ao caminho da preview embutida (abaixo): as
            // cores nunca saem erradas.
            if !info.color_matrix_known {
                calibrate_unknown_camera(&mut s, registry, &data, format, src.orientation, asset)?;
            }
            Ok(s)
        }) {
            Ok(s) => return Ok(s),
            Err(e) => {
                tracing::warn!(asset = asset.0, erro = %e, "RAW sem dados do sensor: edição a partir da preview embutida")
            }
        }
    }
    let probe = registry.probe(&data, format)?;
    let min_short = max_edge.map(|m| {
        let (long, short) = (probe.width.max(probe.height), probe.width.min(probe.height));
        ((m as u64 * short as u64) / long.max(1) as u64).max(1) as u32
    });
    let decoded = registry.decode(&data, format, min_short)?;
    let mut img = apply_orientation(decoded.image, src.orientation);
    if let Some(m) = max_edge {
        let (w, h) = img.dimensions();
        if w.max(h) > m {
            let s = m as f32 / w.max(h) as f32;
            img = imageops::resize(
                &img,
                ((w as f32 * s) as u32).max(1),
                ((h as f32 * s) as u32).max(1),
                imageops::FilterType::Triangle,
            );
        }
    }
    let (fw, fh) = if (5..=8).contains(&src.orientation) {
        (decoded.source_height, decoded.source_width)
    } else {
        (decoded.source_width, decoded.source_height)
    };
    // Perfil ICC embutido (Adobe RGB, Display P3, ProPhoto…): converter para o espaço de
    // trabalho; sem perfil, ou sRGB, assume-se sRGB.
    if let Some(icc) = decoded.icc_profile.as_deref() {
        match momento_color_management::InputColor::new(icc) {
            Ok(Some(input)) => {
                let mut px: Vec<[f32; 3]> =
                    img.pixels().map(|p| p.0.map(|v| v as f32 / 255.0)).collect();
                input.to_linear_rec2020(&mut px);
                let plane = Plane { w: img.width() as usize, h: img.height() as usize, px };
                return Ok(Source::from_plane(plane, fw, fh, 0.0));
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(asset = asset.0, erro = %e, "perfil ICC embutido ignorado: assumido sRGB")
            }
        }
    }
    Ok(Source::from_srgb(&img, fw, fh))
}

/// Erro de cor máximo aceite depois da calibração (nos RAW de teste: 0,02–0,065).
const MAX_CALIBRATED_ERROR: f32 = 0.09;

/// Calibração de cor de um RAW de câmara sem perfil na LibRaw (ver `develop::calibrate`).
/// Erro = sem calibração fiável (a edição usa então a preview embutida).
fn calibrate_unknown_camera(
    s: &mut Source,
    registry: &CodecRegistry,
    data: &[u8],
    format: momento_image_codecs::FileFormat,
    orientation: u8,
    asset: AssetId,
) -> CoreResult<()> {
    let preview = apply_orientation(registry.decode(data, format, Some(480))?.image, orientation);
    let level = s.levels.last().ok_or_else(|| CoreError::Other("fonte vazia".into()))?;
    let (lw, lh, pw, ph) = (level.w, level.h, preview.width(), preview.height());
    match develop::calibrate::calibrate_from_preview(level, &preview) {
        Some(c) if c.error_after <= MAX_CALIBRATED_ERROR => {
            tracing::info!(
                asset = asset.0,
                raw = %format!("{lw}x{lh}"),
                preview = %format!("{pw}x{ph}"),
                enquadramento = c.framing,
                alinhamento = c.alignment,
                amostras = c.samples,
                erro_antes = c.error_before,
                erro_depois = c.error_after,
                preto = ?c.offset,
                "câmara sem perfil de cor na LibRaw: cores calibradas pela preview embutida"
            );
            s.apply_calibration(c.offset, &c.matrix);
            Ok(())
        }
        other => {
            tracing::warn!(
                asset = asset.0,
                raw = %format!("{lw}x{lh}"),
                preview = %format!("{pw}x{ph}"),
                resultado = ?other.map(|c| (c.framing, c.alignment, c.error_after)),
                "câmara sem perfil de cor: calibração pouco fiável — edição a partir da preview embutida"
            );
            Err(CoreError::Other("câmara sem perfil de cor e calibração pouco fiável".into()))
        }
    }
}

/// Cabeçalho binário de um frame: largura, altura (u32 LE), histograma 3×256 (u32 LE), RGBA.
pub fn encode_frame(r: &develop::Rendered) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 3 * 256 * 4 + r.rgba.len());
    out.extend_from_slice(&r.width.to_le_bytes());
    out.extend_from_slice(&r.height.to_le_bytes());
    for ch in &r.histogram {
        for v in ch {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out.extend_from_slice(&r.rgba);
    out
}

fn recipe_hash(r: &EditRecipe) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    develop::PIPELINE_VERSION.hash(&mut h);
    serde_json::to_string(r).unwrap_or_default().hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Ficheiros editados em cache: `cache/edits/<nível>/<variante>-<hash>.jpg`.
pub fn edited_path(
    paths: &ProjectPaths,
    level: &str,
    variant: VariantId,
    recipe: &EditRecipe,
) -> PathBuf {
    paths.cache().join("edits").join(level).join(format!(
        "{}-{}.jpg",
        variant.0,
        recipe_hash(recipe)
    ))
}

/// Remove versões antigas do mesmo nível/variante.
fn prune_old(dir: &std::path::Path, variant: VariantId, keep: &std::path::Path) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        let prefix = format!("{}-", variant.0);
        for e in rd.flatten() {
            let p = e.path();
            if p != keep && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&prefix))
            {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}

/// AI Denoise aplicado a uma fonte (em memória; nunca escrito sobre o original).
///
/// O nível de ruído é medido na própria fonte; `strength` (1–100) escala-o (50 = o nível
/// medido). A rede trabalha no domínio de visualização (0–1); valores acima de 1 (realces
/// recuperáveis dos RAW) recebem a mesma correção, como diferença.
pub fn ai_denoised(
    src: &Source,
    strength: f32,
    dn: &Denoiser,
    progress: &(dyn Fn(usize, usize) -> bool + Sync),
) -> CoreResult<Option<Source>> {
    let p = &src.levels[0];
    let mut disp: Vec<[f32; 3]> = p.px.iter().map(|q| q.map(|c| oetf(c.clamp(0.0, 1.0)))).collect();
    // Modelo de ruído do sensor estimado na fotografia → nível por píxel (mais forte nas
    // sombras) e, se o ruído for correlacionado (grão da desmatrização), rede aplicada às
    // sub-imagens intercaladas. Ganho medido em `tests/denoise_quality.rs`.
    let done = match develop::noise::denoise_plan(p, strength) {
        Some(plan) => {
            tracing::info!(
                correlacionado = plan.shuffled,
                ruido_18 = plan.model.sigma(1, 0.18),
                "AI Denoise: modelo de ruído estimado"
            );
            if plan.shuffled {
                dn.denoise_map_shuffled(&mut disp, p.w, p.h, &plan.map, progress)?
            } else {
                dn.denoise_map(&mut disp, p.w, p.h, &plan.map, progress)?
            }
        }
        None => {
            let (lum, chroma) = develop::auto::measure_noise(src, 1.0);
            let measured = lum.max(chroma * 0.5).max(1.0) / 255.0;
            let sigma =
                (measured * (strength.clamp(1.0, 100.0) / 50.0)).clamp(1.0 / 255.0, 50.0 / 255.0);
            dn.denoise(&mut disp, p.w, p.h, sigma, progress)?
        }
    };
    if !done {
        return Ok(None);
    }
    let px =
        p.px.iter()
            .zip(&disp)
            .map(|(q, d)| {
                std::array::from_fn(|c| {
                    let before = q[c].clamp(0.0, 1.0);
                    (q[c] + eotf(d[c].clamp(0.0, 1.0)) - before).max(0.0)
                })
            })
            .collect();
    let mut out = Source::from_plane(
        Plane { w: p.w, h: p.h, px },
        src.full_width,
        src.full_height,
        src.baseline_ev,
    );
    out.as_shot_k = src.as_shot_k;
    Ok(Some(out))
}

/// Chave de uma fonte com AI Denoise: asset, resolução total?, intensidade.
pub type AiKey = (AssetId, bool, u32);

pub struct EditService {
    cache: Mutex<VecDeque<(AssetId, Arc<Source>)>>,
    /// Fonte em resolução total (zoom a 100 % e exportação): só uma em memória (~300–600 MB).
    full: Mutex<Option<(AssetId, Arc<Source>)>>,
    ai: Mutex<VecDeque<(AiKey, Arc<Source>)>>,
}

impl Default for EditService {
    fn default() -> Self {
        Self::new()
    }
}

impl EditService {
    pub fn new() -> Self {
        EditService {
            cache: Mutex::new(VecDeque::new()),
            full: Mutex::new(None),
            ai: Mutex::new(VecDeque::new()),
        }
    }

    pub fn ai_cached(&self, key: AiKey) -> Option<Arc<Source>> {
        self.ai
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|e| e.0 == key)
            .map(|e| e.1.clone())
    }

    pub fn ai_store(&self, key: AiKey, src: Arc<Source>) {
        let mut c = self.ai.lock().unwrap_or_else(|e| e.into_inner());
        c.retain(|e| e.0 != key);
        c.push_front((key, src));
        c.truncate(2);
        // Resolução total (exportação): ~400 MB a 24 MP — no máximo uma em memória.
        let mut full_seen = false;
        c.retain(|e| {
            let full = e.0 .1;
            let keep = !(full && full_seen);
            full_seen |= full;
            keep
        });
    }

    /// Fonte em resolução total (em cache, uma de cada vez).
    pub fn source_full(
        &self,
        catalog: &Catalog,
        registry: &CodecRegistry,
        asset: AssetId,
    ) -> CoreResult<Arc<Source>> {
        if let Some((a, s)) = self.full.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if *a == asset {
                return Ok(s.clone());
            }
        }
        let s = Arc::new(load_source(catalog, registry, asset, None)?);
        *self.full.lock().unwrap_or_else(|e| e.into_inner()) = Some((asset, s.clone()));
        Ok(s)
    }

    /// Fonte de edição (em cache).
    pub fn source(
        &self,
        catalog: &Catalog,
        registry: &CodecRegistry,
        asset: AssetId,
    ) -> CoreResult<Arc<Source>> {
        {
            // LRU: um acerto passa para a frente (antes saía pela ordem de entrada).
            let mut c = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(i) = c.iter().position(|e| e.0 == asset) {
                let e = c.remove(i).expect("posição válida");
                let s = e.1.clone();
                c.push_front(e);
                return Ok(s);
            }
        }
        let s = Arc::new(load_source(catalog, registry, asset, Some(EDIT_EDGE))?);
        let mut c = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        c.retain(|e| e.0 != asset);
        c.push_front((asset, s.clone()));
        c.truncate(CACHE_ENTRIES);
        Ok(s)
    }

    pub fn forget(&self) {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).clear();
        self.ai.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    pub fn render_frame(
        &self,
        src: &Source,
        s: &Settings,
        max_w: u32,
        max_h: u32,
        quality: Quality,
        uncropped: bool,
    ) -> Vec<u8> {
        self.render_frame_overlay(src, s, max_w, max_h, quality, uncropped, None)
    }

    /// Render com a máscara `overlay` (índice em `s.masks`) a vermelho.
    #[allow(clippy::too_many_arguments)]
    pub fn render_frame_overlay(
        &self,
        src: &Source,
        s: &Settings,
        max_w: u32,
        max_h: u32,
        quality: Quality,
        uncropped: bool,
        overlay: Option<usize>,
    ) -> Vec<u8> {
        let r = develop::render_with_overlay(
            src,
            s,
            &RenderOptions {
                max_width: max_w.clamp(16, 8192),
                max_height: max_h.clamp(16, 8192),
                quality,
                uncropped,
            },
            overlay,
        );
        encode_frame(&r)
    }
}

/// Gera (se necessário) o ficheiro editado de um nível e devolve o caminho.
pub fn ensure_edited(
    paths: &ProjectPaths,
    src: &dyn Fn(Option<u32>) -> CoreResult<Arc<Source>>,
    variant: VariantId,
    recipe: &EditRecipe,
    level: &str,
) -> CoreResult<PathBuf> {
    let path = edited_path(paths, level, variant, recipe);
    if path.is_file() {
        return Ok(path);
    }
    let settings = Settings::from_recipe(recipe);
    let (edge, source) = match level {
        "thumbs" => (THUMB_EDGE, src(Some(EDIT_EDGE))?),
        "previews" => (PREVIEW_EDGE, src(Some(EDIT_EDGE))?),
        _ => (u32::MAX, src(None)?),
    };
    // Nunca ampliar: no máximo a resolução da fonte.
    let edge = edge.min(source.width().max(source.height()) as u32);
    let img: RgbImage = develop::render_rgb(&source, &settings, edge, Quality::Final);
    let jpg = encode_jpeg(&img, if level == "thumbs" { 85 } else { 92 }, None)?;
    write_atomic(&path, &jpg)?;
    if let Some(dir) = path.parent() {
        prune_old(dir, variant, &path);
    }
    Ok(path)
}

/// Sidecar JSON da receita na pasta do projeto (cópia legível, recuperável sem o catálogo).
pub fn write_sidecar(
    paths: &ProjectPaths,
    catalog: &Catalog,
    asset: AssetId,
    variant: VariantId,
    recipe: &EditRecipe,
) -> CoreResult<()> {
    let src = catalog.asset_source(asset)?;
    let path = paths.root.join("edits").join(format!("{}-{}.json", asset.0, variant.0));
    let body = serde_json::json!({
        "generator": "PFF ProMaster",
        "asset": asset.0,
        "variant": variant.0,
        "original": src.map(|s| s.path),
        "recipe": recipe,
    });
    write_atomic(&path, serde_json::to_string_pretty(&body).unwrap_or_default().as_bytes())
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    fn src() -> Arc<Source> {
        Arc::new(Source::from_srgb(&image::RgbImage::new(8, 8), 8, 8))
    }

    #[test]
    fn at_most_one_full_resolution_ai_result_in_memory() {
        let e = EditService::new();
        e.ai_store((AssetId(1), true, 50), src());
        e.ai_store((AssetId(2), true, 50), src());
        assert!(e.ai_cached((AssetId(2), true, 50)).is_some());
        assert!(e.ai_cached((AssetId(1), true, 50)).is_none(), "a anterior em resolução total sai");
        e.ai_store((AssetId(3), false, 50), src());
        assert!(e.ai_cached((AssetId(2), true, 50)).is_some());
        assert!(e.ai_cached((AssetId(3), false, 50)).is_some());
    }
}
