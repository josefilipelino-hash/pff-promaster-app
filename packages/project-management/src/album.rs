//! Álbuns do projeto (Fase 11): guardados no catálogo, montagem automática com os dados
//! da organização e da análise, verificação e exportação pela fila (a partir dos originais).

use crate::export::{ExportRequest, JobState};
use crate::session::ProjectSession;
use momento_album::auto::{self, AutoOptions};
use momento_album::export::ExportKind;
use momento_album::layouts;
use momento_album::preflight::{self, Issue, PhotoInfo};
use momento_album::{Album, AlbumSpec, LabPreset};
use momento_core_types::{AssetId, CoreError, CoreResult};
use momento_database::Catalog;
use momento_image_processing::develop::{
    self, geometry::native_size, LinearImage, Quality, RenderOptions, Settings,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const PREFIX: &str = "album:";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    pub id: String,
    pub name: String,
    pub format: String,
    pub pages: u32,
    pub photos: usize,
}

/// Exportação de um álbum transportada pela fila.
#[derive(Debug, Clone, PartialEq)]
pub struct AlbumExportJob {
    pub album: Album,
    pub kind: ExportKind,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoReport {
    pub album: Album,
    pub used: usize,
    pub candidates: usize,
    pub skipped_low_res: usize,
    pub skipped_duplicates: usize,
}

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Other(e.to_string())
}

impl ProjectSession {
    pub fn list_albums(&self) -> CoreResult<Vec<AlbumSummary>> {
        let cat = self.open_catalog()?;
        let mut out = Vec::new();
        for (_, body) in cat.values_with_prefix(PREFIX)? {
            if let Ok(a) = serde_json::from_str::<Album>(&body) {
                out.push(AlbumSummary {
                    photos: a.photos().len(),
                    id: a.id,
                    name: a.name,
                    format: a.spec.format,
                    pages: a.spec.pages,
                });
            }
        }
        Ok(out)
    }

    pub fn get_album(&self, id: &str) -> CoreResult<Album> {
        let body = self
            .open_catalog()?
            .load_value(&format!("{PREFIX}{id}"))?
            .ok_or_else(|| CoreError::Other(format!("álbum {id} inexistente")))?;
        serde_json::from_str(&body).map_err(err)
    }

    pub fn save_album(&self, album: &Album) -> CoreResult<()> {
        if album.id.is_empty() || album.name.trim().is_empty() {
            return Err(CoreError::Other("o álbum precisa de um nome".into()));
        }
        let body = serde_json::to_string(album).map_err(err)?;
        self.open_catalog()?.save_value(&format!("{PREFIX}{}", album.id), &body)
    }

    pub fn create_album(&self, name: &str, spec: AlbumSpec) -> CoreResult<Album> {
        let a = Album::new(name, spec);
        self.save_album(&a)?;
        Ok(a)
    }

    pub fn delete_album(&self, id: &str) -> CoreResult<bool> {
        self.open_catalog()?.delete_value(&format!("{PREFIX}{id}"))
    }

    /// Tudo o que a montagem e a verificação precisam de cada fotografia.
    pub fn album_photo_infos(&self) -> CoreResult<HashMap<i64, PhotoInfo>> {
        let rows = self.list_photos()?;
        let cat = self.open_catalog()?;
        let moments: HashMap<i64, Option<String>> = cat
            .list_moments()?
            .into_iter()
            .map(|m| (m.id, if m.custom { None } else { m.kind }))
            .collect();
        let moment_of: HashMap<AssetId, Option<String>> = cat
            .asset_moments()?
            .into_iter()
            .filter_map(|m| Some((m.asset, moments.get(&m.moment_id?)?.clone())))
            .collect();
        let faces: HashMap<AssetId, Vec<[f32; 4]>> = cat
            .selection_rows()?
            .into_iter()
            .map(|r| {
                let (w, h) = (r.analysis.width.max(1) as f32, r.analysis.height.max(1) as f32);
                (r.asset, r.faces.iter().map(|f| [f.x / w, f.y / h, f.w / w, f.h / h]).collect())
            })
            .collect();
        let scores = cat.overall_scores()?;
        let mut group: HashMap<AssetId, i64> = HashMap::new();
        for g in cat.list_groups()? {
            for m in g.members {
                group.insert(m, g.id);
            }
        }
        let mut persons: HashMap<AssetId, Vec<i64>> = HashMap::new();
        for (a, p) in cat.photo_persons()? {
            persons.entry(a).or_default().push(p);
        }
        let mut out = HashMap::new();
        for r in rows {
            let (variant, _) = cat.resolve_key(r.id.0)?;
            let s = Settings::from_recipe(&cat.recipe_of(variant)?);
            let (w, h) = native_size(r.width as usize, r.height as usize, &s.geometry);
            let score = scores
                .get(&r.asset)
                .copied()
                .unwrap_or(0.1 + r.stars as f32 / 5.0 * 0.7 + if r.flag > 0 { 0.2 } else { 0.0 });
            out.insert(
                r.id.0,
                PhotoInfo {
                    key: r.id.0,
                    width: w,
                    height: h,
                    time: r.capture_time.clone(),
                    moment: moment_of.get(&r.asset).cloned().flatten(),
                    flag: r.flag,
                    stars: r.stars,
                    score,
                    faces: faces.get(&r.asset).cloned().unwrap_or_default(),
                    persons: persons.get(&r.asset).cloned().unwrap_or_default(),
                    group: group.get(&r.asset).copied(),
                },
            );
        }
        Ok(out)
    }

    /// "Criar álbum automaticamente": substitui os spreads do álbum (grava).
    pub fn auto_album(&self, id: &str, options: AutoOptions) -> CoreResult<AutoReport> {
        let current = self.get_album(id)?;
        let infos = self.album_photo_infos()?;
        let mut photos: Vec<PhotoInfo> = infos.into_values().collect();
        photos.sort_by_key(|p| p.key);
        let r = auto::build(&current.name, &current.spec, &photos, &options);
        let mut album = r.album;
        album.id = current.id;
        self.save_album(&album)?;
        Ok(AutoReport {
            album,
            used: r.used,
            candidates: photos.len(),
            skipped_low_res: r.skipped_low_res,
            skipped_duplicates: r.skipped_duplicates,
        })
    }

    /// Aplica um layout a um spread, com as fotografias que lá estão (pela ordem).
    pub fn apply_album_layout(
        &self,
        album: &Album,
        spread: usize,
        template: &str,
        margin: f32,
        spacing: f32,
        extra: &[i64],
    ) -> CoreResult<Album> {
        let infos = self.album_photo_infos()?;
        let mut a = album.clone();
        let sp =
            a.spreads.get(spread).ok_or_else(|| CoreError::Other("spread inexistente".into()))?;
        let mut keys: Vec<i64> = sp.slots.iter().filter_map(|s| s.photo).collect();
        keys.extend(extra.iter().filter(|k| !keys.contains(k)).copied().collect::<Vec<_>>());
        let photos: Vec<(i64, f32)> =
            keys.iter().map(|k| (*k, infos.get(k).map(|p| p.aspect()).unwrap_or(1.5))).collect();
        let mut new = layouts::apply(&a.spec, template, &photos, margin, spacing)
            .ok_or_else(|| CoreError::Other(format!("layout desconhecido: {template}")))?;
        // Recortes iniciais nos rostos.
        for sl in new.slots.iter_mut() {
            if let Some(p) = sl.photo.and_then(|k| infos.get(&k)) {
                if !p.faces.is_empty() {
                    let cx = p.faces.iter().map(|f| f[0] + f[2] / 2.0).sum::<f32>()
                        / p.faces.len() as f32;
                    let cy = p.faces.iter().map(|f| f[1] + f[3] / 2.0).sum::<f32>()
                        / p.faces.len() as f32;
                    sl.crop.cx = cx;
                    sl.crop.cy = (cy + 0.08).min(0.95);
                }
            }
        }
        new.id = a.spreads[spread].id.clone();
        a.spreads[spread] = new;
        Ok(a)
    }

    pub fn album_preflight(&self, album: &Album) -> CoreResult<Vec<Issue>> {
        Ok(preflight::check(album, &self.album_photo_infos()?))
    }

    /// Exporta pela fila (JPEG/TIFF por spread ou página, PDF de impressão, PDF de prova).
    pub fn start_album_export(
        self: &Arc<Self>,
        id: &str,
        kind: ExportKind,
        destination: &str,
    ) -> CoreResult<u64> {
        let album = self.get_album(id)?;
        if let Some(p) = album.spec.problems().first() {
            return Err(CoreError::Other(p.to_string()));
        }
        if destination.trim().is_empty() {
            return Err(CoreError::Other("escolha a pasta de destino".into()));
        }
        let name = format!(
            "{} · {}",
            album.name,
            match kind {
                ExportKind::Jpeg => "JPEG",
                ExportKind::Tiff => "TIFF",
                ExportKind::PrintPdf => "PDF",
                ExportKind::ProofPdf => "PDF (prova)",
            }
        );
        self.start_export(ExportRequest {
            keys: vec![],
            destination: destination.into(),
            name,
            album: Some(Box::new(AlbumExportJob { album, kind })),
            ..Default::default()
        })
    }

    /// Revelação de uma fotografia para o álbum, no tamanho pedido (lado maior).
    pub(crate) fn render_for_album(
        self: &Arc<Self>,
        cat: &Catalog,
        key: i64,
        need: u32,
    ) -> CoreResult<LinearImage> {
        let (variant, asset) = cat.resolve_key(key)?;
        let settings = Settings::from_recipe(&cat.recipe_of(variant)?);
        let (src, _) = self.source_for(cat, asset, &settings, need > 2500, true, key)?;
        let (nw, nh) = native_size(src.width(), src.height(), &settings.geometry);
        let k = (need as f32 / nw.max(nh) as f32).min(1.0);
        Ok(develop::render_linear(
            &src,
            &settings,
            &RenderOptions {
                max_width: ((nw as f32 * k).round() as u32).max(1),
                max_height: ((nh as f32 * k).round() as u32).max(1),
                quality: Quality::Final,
                uncropped: false,
            },
        ))
    }

    /// Corre a exportação de um álbum (chamado pela fila).
    pub(crate) fn run_album_export(
        self: &Arc<Self>,
        a: &AlbumExportJob,
        dest: &std::path::Path,
        progress: &dyn Fn(usize, usize, u64) -> bool,
    ) -> CoreResult<Vec<String>> {
        let cat = self.open_catalog()?;
        // Cache das revelações (as mesmas fotografias aparecem em spreads seguidos raramente,
        // mas um spread pede-as várias vezes ao recompor).
        let cache: Mutex<Vec<(i64, u32, Arc<LinearImage>)>> = Mutex::new(Vec::new());
        let me = self.clone();
        let provider = |key: i64, need: u32| -> Option<Arc<LinearImage>> {
            let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(hit) = c.iter().find(|e| e.0 == key && e.1 >= need) {
                return Some(hit.2.clone());
            }
            let img = match me.render_for_album(&cat, key, need) {
                Ok(i) => Arc::new(i),
                Err(e) => {
                    tracing::warn!(chave = key, erro = %e, "fotografia do álbum não revelada");
                    return None;
                }
            };
            c.retain(|e| e.0 != key);
            c.insert(0, (key, need, img.clone()));
            c.truncate(6);
            Some(img)
        };
        std::fs::create_dir_all(dest)?;
        let mut written = Vec::new();
        let mut bytes = 0u64;
        momento_album::export::export(
            &a.album,
            a.kind,
            &provider,
            &mut |name, data| {
                let path = dest.join(momento_export_engine::naming::sanitize(name));
                bytes += data.len() as u64;
                momento_image_processing::write_atomic(&path, &data)?;
                written.push(path.to_string_lossy().into_owned());
                Ok(())
            },
            &|done, total| progress(done, total, 0),
        )?;
        progress(a.album.spreads.len(), a.album.spreads.len(), bytes);
        Ok(written)
    }

    /// Estado de um trabalho (testes e interface).
    pub fn job_state(&self, id: u64) -> Option<JobState> {
        self.export_jobs().into_iter().find(|p| p.id == id).map(|p| p.state)
    }
}

/// Presets de laboratório (biblioteca global).
pub struct LabStore<'a> {
    pub lib: &'a momento_database::library::Library,
}

impl LabStore<'_> {
    pub fn list(&self) -> CoreResult<Vec<LabPreset>> {
        Ok(self
            .lib
            .lab_presets()?
            .into_iter()
            .filter_map(|(id, name, body)| {
                let mut p: LabPreset = serde_json::from_str(&body).ok()?;
                p.id = id;
                p.name = name;
                Some(p)
            })
            .collect())
    }

    pub fn save(&self, p: &LabPreset) -> CoreResult<LabPreset> {
        let mut p = p.clone();
        if p.name.trim().is_empty() {
            return Err(CoreError::Other("o preset precisa de um nome".into()));
        }
        if p.id.is_empty() {
            p.id = momento_album::new_id();
        }
        self.lib.save_lab_preset(&p.id, &p.name, &serde_json::to_string(&p).map_err(err)?)?;
        Ok(p)
    }

    pub fn delete(&self, id: &str) -> CoreResult<bool> {
        self.lib.delete_lab_preset(id)
    }
}
