//! Operações sobre originais (Fase 13). Regras:
//! - nada é apagado automaticamente;
//! - remover do catálogo **não toca** em ficheiros;
//! - mover para o lixo usa o lixo do sistema (recuperável) e exige `confirmed = true`
//!   (a interface pede confirmação explícita; o núcleo recusa sem ela);
//! - a cópia de segurança feita na importação nunca é tocada;
//! - relink só aceita um ficheiro com **o mesmo conteúdo** (hash BLAKE3).

use crate::paths::CacheLevel;
use crate::ProjectSession;
use momento_core_types::{decode_key, AssetId, CoreError, CoreResult, PhotoRef};
use momento_database::safety::{Availability, Original};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveReport {
    /// Fotografias retiradas do catálogo.
    pub removed: u64,
    /// Originais enviados para o lixo do sistema.
    pub trashed: u64,
    /// Fotografias que ficaram (ex.: o lixo recusou o ficheiro), com o motivo.
    pub failed: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelinkReport {
    pub relinked: u64,
    /// Ainda indisponíveis depois da procura.
    pub still_unavailable: u64,
}

impl ProjectSession {
    /// Chaves → (assets principais a remover, cópias virtuais a remover).
    fn split_keys(&self, keys: &[i64]) -> (Vec<AssetId>, Vec<i64>) {
        let mut assets = Vec::new();
        let mut virtuals = Vec::new();
        for &k in keys {
            match decode_key(k) {
                PhotoRef::Master(a) => assets.push(a),
                PhotoRef::Variant(_) => virtuals.push(k),
            }
        }
        assets.sort();
        assets.dedup();
        (assets, virtuals)
    }

    fn forget_caches(&self, ids: &[AssetId]) {
        for id in ids {
            for l in [CacheLevel::Thumb, CacheLevel::Preview, CacheLevel::Full] {
                let _ = std::fs::remove_file(self.paths.cached(l, *id));
            }
        }
        self.edits.forget();
    }

    /// Remove fotografias **só do catálogo**. Os ficheiros ficam onde estão.
    pub fn remove_from_catalog(&self, keys: &[i64], confirmed: bool) -> CoreResult<RemoveReport> {
        if !confirmed {
            return Err(CoreError::Other("remoção não confirmada".into()));
        }
        let (assets, virtuals) = self.split_keys(keys);
        for k in &virtuals {
            self.catalog().delete_virtual_copy(*k)?;
        }
        let n = self.catalog().remove_assets(&assets)?;
        self.forget_caches(&assets);
        let _ = self.catalog().log_event(
            "removidas_do_catalogo",
            &format!("{n} fotografias, {} cópias virtuais", virtuals.len()),
        );
        Ok(RemoveReport { removed: (n + virtuals.len()) as u64, ..Default::default() })
    }

    /// Envia os originais para o lixo do sistema e retira as fotografias do catálogo. Uma
    /// fotografia cujo ficheiro o lixo recusar fica no catálogo (com o motivo).
    pub fn move_to_trash(&self, keys: &[i64], confirmed: bool) -> CoreResult<RemoveReport> {
        if !confirmed {
            return Err(CoreError::Other("envio para o lixo não confirmado".into()));
        }
        let (assets, virtuals) = self.split_keys(keys);
        let mut report = RemoveReport::default();
        // Uma cópia virtual não tem ficheiro próprio: o original é partilhado com a versão
        // principal, por isso só a cópia sai do catálogo (o ficheiro nunca vai para o lixo
        // por causa de uma cópia).
        for k in &virtuals {
            self.catalog().delete_virtual_copy(*k)?;
        }
        report.removed += virtuals.len() as u64;
        let mut gone = Vec::new();
        for id in assets {
            let Some(o) = self.catalog().original(id)? else { continue };
            let path = Path::new(&o.path);
            if !path.is_file() {
                report.failed.push((
                    o.file_name.clone(),
                    "original indisponível (disco desligado ou ficheiro em falta)".into(),
                ));
                continue;
            }
            match trash::delete(path) {
                Ok(()) => {
                    report.trashed += 1;
                    gone.push(id);
                }
                Err(e) => report.failed.push((o.file_name.clone(), e.to_string())),
            }
        }
        report.removed += self.catalog().remove_assets(&gone)? as u64;
        self.forget_caches(&gone);
        let _ = self.catalog().log_event(
            "enviadas_para_o_lixo",
            &format!("{} enviadas; {} recusadas", report.trashed, report.failed.len()),
        );
        Ok(report)
    }

    /// Caminho do original (para o mostrar no gestor de ficheiros). Indisponível → erro.
    pub fn original_path(&self, key: i64) -> CoreResult<PathBuf> {
        let (_, asset) = self.catalog().resolve_key(key)?;
        let o = self
            .catalog()
            .original(asset)?
            .ok_or_else(|| CoreError::Other("fotografia inexistente".into()))?;
        let p = PathBuf::from(&o.path);
        if !p.is_file() {
            return Err(crate::availability::unavailable(&o.path));
        }
        Ok(p)
    }

    /// Relink de uma fotografia para um ficheiro escolhido (tem de ter o mesmo conteúdo).
    /// Depois, procura na mesma pasta as outras fotografias indisponíveis (pasta movida).
    pub fn relink_file(self: &Arc<Self>, key: i64, file: &Path) -> CoreResult<RelinkReport> {
        let (_, asset) = self.catalog().resolve_key(key)?;
        let o = self
            .catalog()
            .original(asset)?
            .ok_or_else(|| CoreError::Other("fotografia inexistente".into()))?;
        if !crate::availability::same_content(file, &o)? {
            return Err(CoreError::Other(
                "este ficheiro não é o original desta fotografia (o conteúdo é diferente)".into(),
            ));
        }
        self.catalog().relink(asset, &file.to_string_lossy(), file_mtime(file))?;
        let mut report = RelinkReport { relinked: 1, ..Default::default() };
        if let Some(dir) = file.parent() {
            report.relinked += self.relink_folder_inner(dir, 1)?.relinked;
        }
        self.after_relink(&mut report)?;
        Ok(report)
    }

    /// Procura numa pasta (e subpastas) os originais indisponíveis e faz relink dos que
    /// tiverem o mesmo nome, tamanho e conteúdo.
    pub fn relink_folder(self: &Arc<Self>, folder: &Path) -> CoreResult<RelinkReport> {
        let mut r = self.relink_folder_inner(folder, 8)?;
        self.after_relink(&mut r)?;
        Ok(r)
    }

    fn relink_folder_inner(&self, folder: &Path, depth: usize) -> CoreResult<RelinkReport> {
        let wanted: Vec<Original> = self
            .catalog()
            .originals()?
            .into_iter()
            .filter(|o| o.status != Availability::Ok || !Path::new(&o.path).is_file())
            .collect();
        let found = crate::availability::find_in_folder(folder, &wanted, depth);
        let mut n = 0;
        for (id, p) in found {
            if self.catalog().relink(id, &p.to_string_lossy(), file_mtime(&p)).is_ok() {
                n += 1;
            }
        }
        Ok(RelinkReport { relinked: n, ..Default::default() })
    }

    fn after_relink(&self, r: &mut RelinkReport) -> CoreResult<()> {
        let _ = self.catalog().log_event("relink", &format!("{} fotografias", r.relinked));
        let s = self.check_availability()?;
        r.still_unavailable = s.offline + s.missing;
        let pending = self.catalog().pending_thumbnail_ids()?;
        self.enqueue_thumbnails(pending);
        Ok(())
    }
}

fn file_mtime(p: &Path) -> Option<i64> {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}
