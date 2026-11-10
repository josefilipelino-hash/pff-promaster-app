//! Galeria do projeto (Fase 10): definições no catálogo, escolha das fotografias,
//! geração pela fila de exportação (a partir dos originais) e montagem do site estático.

use crate::export::{ExportRequest, JobState};
use crate::session::ProjectSession;
use momento_color_management::ColorSpace;
use momento_core_types::{AssetId, CoreError, CoreResult};
use momento_export_engine::{
    Compression, Conflict, Format, Metadata, MetadataMode, Naming, Recipe, Sharpen, SharpenAmount,
    SharpenTarget, Size, SizeMode, Watermark,
};
use momento_gallery::chapters::{self, Chapter, MomentInfo, PhotoInfo};
use momento_gallery::site::{self, SiteImage, SitePhoto, MARKER};
use momento_gallery::{DownloadSize, Downloads, GallerySettings, Selection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const KEY: &str = "gallery_config";

/// O que o fotógrafo escolhe para a galeria deste projeto.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GalleryConfig {
    pub settings: GallerySettings,
    /// Marca de água nas imagens da galeria.
    pub watermark: Watermark,
    /// Pasta onde a galeria é criada (`<destino>/<noivos>-galeria`).
    pub destination: String,
}

/// Trabalho de galeria transportado pela fila de exportação.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GalleryJob {
    pub config: GalleryConfig,
    pub folder: PathBuf,
    pub chapters: Vec<Chapter>,
    pub tokens: HashMap<i64, String>,
    /// Nome do ficheiro original (sem extensão) de cada chave.
    pub names: HashMap<i64, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterCount {
    pub title: String,
    pub count: usize,
}

/// Antevisão antes de gerar.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryPreview {
    pub photos: usize,
    pub hidden: usize,
    pub chapters: Vec<ChapterCount>,
    pub folder: String,
    /// A pasta já existe e é uma galeria do Momento (pode ser substituída).
    pub exists: bool,
    /// A pasta existe e **não** é uma galeria do Momento (nunca é tocada).
    pub blocked: bool,
    pub problems: Vec<String>,
    pub estimate_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoritesImport {
    pub matched: usize,
    pub unmatched: Vec<String>,
}

/// Receitas das três versões de cada fotografia.
fn recipes(cfg: &GalleryConfig) -> Vec<Recipe> {
    let s = &cfg.settings;
    let base = |id: &str, folder: &str, q: u8, size: Size, sharpen: Sharpen| Recipe {
        id: id.into(),
        name: format!("Galeria · {id}"),
        format: Format::Jpeg,
        quality: q,
        compression: Compression::Lossy,
        color: ColorSpace::Srgb,
        size,
        sharpen,
        metadata: Metadata {
            mode: MetadataMode::Copyright,
            remove_gps: true,
            creator: s.photographer.clone(),
            copyright: if s.photographer.trim().is_empty() {
                String::new()
            } else {
                format!("© {}", s.photographer.trim())
            },
            contact: s.contact_email.clone(),
        },
        watermark: cfg.watermark.clone(),
        naming: Naming {
            template: "{token}".into(),
            subfolder: folder.into(),
            conflict: Conflict::Overwrite,
            ..Default::default()
        },
        ..Default::default()
    };
    let long =
        |px: f32| Size { mode: SizeMode::LongEdge, a: px, b: px, ppi: 72.0, ..Default::default() };
    let screen = |a| Sharpen { target: SharpenTarget::Screen, amount: a };
    let mut v = vec![
        base("miniatura", "img/t", 80, long(900.0), screen(SharpenAmount::Low)),
        base("ecra", "img/w", 85, long(2048.0), screen(SharpenAmount::Standard)),
    ];
    if s.downloads != Downloads::None && s.download_size == DownloadSize::High {
        v.push(base(
            "download",
            "img/d",
            92,
            Size { ppi: 300.0, ..Default::default() },
            Sharpen::default(),
        ));
    }
    v
}

pub fn folder_for(cfg: &GalleryConfig) -> PathBuf {
    PathBuf::from(&cfg.destination).join(format!("{}-galeria", site::slug(&cfg.settings.couple)))
}

impl ProjectSession {
    pub fn gallery_config(&self) -> CoreResult<GalleryConfig> {
        let cat = self.open_catalog()?;
        let raw = cat.load_value(KEY)?;
        let mut cfg: GalleryConfig =
            raw.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default();
        if cfg.settings.couple.is_empty() {
            // Valores iniciais a partir do projeto.
            let info = self.info()?;
            cfg.settings.couple = info.record.client_name.clone();
            cfg.settings.date = info.record.event_date.clone().unwrap_or_default();
            cfg.settings.location = info.record.location.clone();
        }
        Ok(cfg)
    }

    pub fn save_gallery_config(&self, cfg: &GalleryConfig) -> CoreResult<()> {
        let body = serde_json::to_string(cfg).map_err(|e| CoreError::Other(e.to_string()))?;
        self.open_catalog()?.save_value(KEY, &body)
    }

    /// Oculta (ou volta a mostrar) fotografias na galeria.
    pub fn set_gallery_hidden(&self, keys: &[i64], hidden: bool) -> CoreResult<GalleryConfig> {
        let mut cfg = self.gallery_config()?;
        let mut set: HashSet<i64> = cfg.settings.hidden.iter().copied().collect();
        for k in keys {
            if hidden {
                set.insert(*k);
            } else {
                set.remove(k);
            }
        }
        let mut v: Vec<i64> = set.into_iter().collect();
        v.sort();
        cfg.settings.hidden = v;
        self.save_gallery_config(&cfg)?;
        Ok(cfg)
    }

    /// Fotografias que entram na galeria (sem as ocultas).
    fn gallery_keys(
        &self,
        s: &GallerySettings,
    ) -> CoreResult<Vec<momento_database::assets::PhotoRow>> {
        let rows = self.list_photos()?;
        let hidden: HashSet<i64> = s.hidden.iter().copied().collect();
        let not_rejected = |r: &&momento_database::assets::PhotoRow| r.flag >= 0;
        let chosen: Vec<_> = match s.selection {
            Selection::Picks => {
                let picks: Vec<_> = rows.iter().filter(|r| r.flag > 0).collect();
                if picks.is_empty() {
                    rows.iter().filter(not_rejected).collect()
                } else {
                    picks
                }
            }
            Selection::NotRejected => rows.iter().filter(not_rejected).collect(),
            Selection::Stars => {
                rows.iter().filter(|r| r.stars >= s.min_stars && r.flag >= 0).collect()
            }
            Selection::All => rows.iter().collect(),
        };
        Ok(chosen.into_iter().filter(|r| !hidden.contains(&r.id.0)).cloned().collect())
    }

    fn gallery_chapters(
        &self,
        s: &GallerySettings,
        rows: &[momento_database::assets::PhotoRow],
        moment_names: &HashMap<i64, String>,
    ) -> CoreResult<Vec<Chapter>> {
        let cat = self.open_catalog()?;
        let of_asset: HashMap<AssetId, i64> = cat
            .asset_moments()?
            .into_iter()
            .filter_map(|m| m.moment_id.map(|id| (m.asset, id)))
            .collect();
        let moments: Vec<MomentInfo> = cat
            .list_moments()?
            .into_iter()
            .map(|m| MomentInfo {
                id: m.id,
                name: moment_names
                    .get(&m.id)
                    .cloned()
                    .or(m.name.clone())
                    .or(m.kind.clone())
                    .unwrap_or_default(),
                kind: if m.custom { None } else { m.kind },
                sort: m.sort,
            })
            .collect();
        let photos: Vec<PhotoInfo> = rows
            .iter()
            .map(|r| PhotoInfo {
                key: r.id.0,
                moment: of_asset.get(&r.asset).copied(),
                time: r.capture_time.clone(),
            })
            .collect();
        Ok(chapters::assign(&photos, &moments, s.chapters, s.english()))
    }

    /// Antevisão: fotografias, capítulos, pasta, problemas e tamanho estimado.
    pub fn gallery_preview(
        &self,
        cfg: &GalleryConfig,
        moment_names: &HashMap<i64, String>,
    ) -> CoreResult<GalleryPreview> {
        let rows = self.gallery_keys(&cfg.settings)?;
        let chapters = self.gallery_chapters(&cfg.settings, &rows, moment_names)?;
        let folder = folder_for(cfg);
        let exists = folder.join(MARKER).is_file();
        let blocked = folder.exists() && !exists;
        let mut problems: Vec<String> =
            cfg.settings.problems().iter().map(|s| s.to_string()).collect();
        if cfg.destination.trim().is_empty() {
            problems.push("gallery.problem.destination".into());
        }
        if rows.is_empty() {
            problems.push("gallery.problem.empty".into());
        }
        if blocked {
            problems.push("gallery.problem.blocked".into());
        }
        if !cfg.settings.logo_path.is_empty() && !Path::new(&cfg.settings.logo_path).is_file() {
            problems.push("gallery.problem.logo".into());
        }
        // Estimativa: miniatura + ecrã + download (+ ZIP).
        let per: u64 = recipes(cfg)
            .iter()
            .filter_map(|r| momento_export_engine::Prepared::new(r).ok())
            .map(|p| rows.iter().map(|r| p.estimate(r.width, r.height)).sum::<u64>())
            .sum();
        let zip_copy = if cfg.settings.downloads == Downloads::All {
            let last =
                recipes(cfg).pop().and_then(|r| momento_export_engine::Prepared::new(&r).ok());
            last.map(|p| rows.iter().map(|r| p.estimate(r.width, r.height)).sum::<u64>())
                .unwrap_or(0)
                * if cfg.settings.chapter_zips { 2 } else { 1 }
        } else {
            0
        };
        Ok(GalleryPreview {
            photos: rows.len(),
            hidden: cfg.settings.hidden.len(),
            chapters: chapters
                .iter()
                .map(|c| ChapterCount { title: c.title.clone(), count: c.keys.len() })
                .collect(),
            folder: folder.to_string_lossy().into_owned(),
            exists,
            blocked,
            problems,
            estimate_bytes: per + zip_copy,
        })
    }

    /// Gera a galeria em segundo plano (fila de exportação). `replace` substitui uma galeria
    /// anterior do Momento na mesma pasta (o utilizador confirmou).
    pub fn start_gallery(
        self: &Arc<Self>,
        cfg: GalleryConfig,
        moment_names: HashMap<i64, String>,
        replace: bool,
    ) -> CoreResult<u64> {
        let preview = self.gallery_preview(&cfg, &moment_names)?;
        if let Some(p) = preview.problems.first() {
            return Err(CoreError::Other(p.clone()));
        }
        self.save_gallery_config(&cfg)?;
        let folder = folder_for(&cfg);
        if folder.exists() {
            // Só se apaga uma pasta que o Momento criou (tem o marcador) e com confirmação.
            if !folder.join(MARKER).is_file() {
                return Err(CoreError::Other("gallery.problem.blocked".into()));
            }
            if !replace {
                return Err(CoreError::Other("gallery.problem.exists".into()));
            }
            std::fs::remove_dir_all(&folder)?;
        }
        std::fs::create_dir_all(&folder)?;
        std::fs::write(folder.join(MARKER), "Momento Studio\n")?;
        let rows = self.gallery_keys(&cfg.settings)?;
        let chapters = self.gallery_chapters(&cfg.settings, &rows, &moment_names)?;
        let mut keys: Vec<i64> = chapters.iter().flat_map(|c| c.keys.iter().copied()).collect();
        for extra in [cfg.settings.hero, cfg.settings.cover].into_iter().flatten() {
            if !keys.contains(&extra) {
                keys.push(extra);
            }
        }
        let mut tokens = HashMap::new();
        for k in &keys {
            tokens.insert(*k, momento_gallery::crypto::token()?);
        }
        let all = self.list_photos()?;
        let names: HashMap<i64, String> = all
            .iter()
            .map(|r| {
                let stem = Path::new(&r.file_name)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                (r.id.0, if r.virtual_copy.is_some() { format!("{stem}-copia") } else { stem })
            })
            .collect();
        let req = ExportRequest {
            keys,
            recipes: recipes(&cfg),
            destination: folder.to_string_lossy().into_owned(),
            name: if cfg.settings.english() { "Gallery".into() } else { "Galeria".into() },
            moment_names,
            gallery: Some(Box::new(GalleryJob { config: cfg, folder, chapters, tokens, names })),
            album: None,
        };
        self.start_export(req)
    }

    /// Montagem final (chamada pela fila depois de escrever as imagens).
    pub(crate) fn finish_gallery(
        &self,
        g: &GalleryJob,
        outputs: &[(i64, usize, PathBuf, u32, u32)],
    ) -> CoreResult<String> {
        let img = |key: i64, recipe: usize| {
            outputs.iter().find(|o| o.0 == key && o.1 == recipe).map(|o| SiteImage {
                path: o.2.clone(),
                width: o.3,
                height: o.4,
            })
        };
        let has_download = recipes(&g.config).len() > 2;
        let mut photos = HashMap::new();
        for c in &g.chapters {
            for &k in &c.keys {
                let (Some(thumb), Some(web)) = (img(k, 0), img(k, 1)) else { continue };
                photos.insert(
                    k,
                    SitePhoto {
                        key: k,
                        token: g.tokens.get(&k).cloned().unwrap_or_default(),
                        name: g.names.get(&k).cloned().unwrap_or_default(),
                        thumb,
                        web,
                        download: if has_download { img(k, 2) } else { None },
                    },
                );
            }
        }
        let s = &g.config.settings;
        let hero = s.hero.and_then(|k| img(k, 1));
        let cover = s.cover.and_then(|k| img(k, 1));
        let logo = (!s.logo_path.is_empty()).then(|| PathBuf::from(&s.logo_path));
        let report = site::build(
            &g.folder,
            &site::SiteInput {
                settings: s,
                chapters: &g.chapters,
                photos: &photos,
                hero: hero.as_ref(),
                cover: cover.as_ref(),
                logo: logo.as_deref(),
            },
        )?;
        tracing::info!(fotografias = report.photos, ficheiros = report.files, "galeria gerada");
        Ok(report.index)
    }

    /// Favoritas escolhidas pelo cliente (lista de nomes copiada da galeria): marca as
    /// fotografias com a etiqueta roxa.
    pub fn import_client_favorites(&self, text: &str) -> CoreResult<FavoritesImport> {
        let rows = self.list_photos()?;
        let by_name: HashMap<String, AssetId> = rows
            .iter()
            .filter(|r| r.virtual_copy.is_none())
            .map(|r| {
                let stem = Path::new(&r.file_name)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                (stem, r.asset)
            })
            .collect();
        let mut matched = Vec::new();
        let mut unmatched = Vec::new();
        for raw in text.split(['\n', ',', ';', '\t']) {
            let name = raw.trim();
            if name.is_empty() {
                continue;
            }
            let stem = Path::new(name)
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let stem = stem.trim_end_matches("-copia").to_string();
            match by_name.get(&stem) {
                Some(a) => {
                    if !matched.contains(a) {
                        matched.push(*a);
                    }
                }
                None => unmatched.push(name.to_string()),
            }
        }
        if !matched.is_empty() {
            self.set_color(&matched, Some("purple"))?;
        }
        Ok(FavoritesImport { matched: matched.len(), unmatched })
    }

    /// Estado do último trabalho de galeria.
    pub fn gallery_job_done(&self, id: u64) -> Option<bool> {
        self.export_jobs().into_iter().find(|p| p.id == id).map(|p| p.state == JobState::Done)
    }
}
