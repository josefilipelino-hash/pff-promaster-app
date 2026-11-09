//! Presets do utilizador (biblioteca global, partilhados por todos os projetos) + presets
//! internos + LUT (`<dados da aplicação>/luts/<id>.cube`).

use momento_core_types::{CoreError, CoreResult};
use momento_database::library::{Library, PresetRow};
use momento_image_processing::develop::{lut, Settings};
use momento_presets::builtin::{builtin, BUILTIN_PREFIX};
use momento_presets::xmp::CompatReport;
use momento_presets::{capture, file, Component, Preset};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Resultado de uma importação.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub presets: Vec<Preset>,
    /// Só para XMP: o que foi interpretado e ignorado.
    pub report: Option<CompatReport>,
    /// Tipo de ficheiro: `momentopreset`, `xmp` ou `cube`.
    pub kind: String,
}

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Other(e.to_string())
}

fn new_id() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("u{:x}{:x}", t, n)
}

pub struct PresetStore<'a> {
    pub lib: &'a Library,
    /// Pasta das LUT importadas.
    pub lut_dir: PathBuf,
}

impl PresetStore<'_> {
    /// Internos primeiro, depois os do utilizador (por grupo e posição).
    pub fn list(&self) -> CoreResult<Vec<Preset>> {
        let mut out = builtin();
        for r in self.lib.presets()? {
            match serde_json::from_str::<Preset>(&r.body) {
                Ok(mut p) => {
                    p.id = r.id;
                    p.name = r.name;
                    p.group = r.group;
                    p.builtin = false;
                    out.push(p);
                }
                Err(e) => tracing::warn!(preset = %r.id, erro = %e, "preset ilegível ignorado"),
            }
        }
        Ok(out)
    }

    pub fn get(&self, id: &str) -> CoreResult<Preset> {
        self.list()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| err(format!("preset {id} inexistente")))
    }

    fn save(&self, p: &Preset, position: Option<i64>) -> CoreResult<()> {
        if p.id.starts_with(BUILTIN_PREFIX) {
            return Err(err("os presets internos não podem ser alterados (duplique-o)"));
        }
        let position = match position {
            Some(v) => v,
            None => self
                .lib
                .presets()?
                .into_iter()
                .find(|r| r.id == p.id && r.group == p.group)
                .map(|r| r.position)
                .map(Ok)
                .unwrap_or_else(|| self.lib.next_preset_position(&p.group))?,
        };
        let body = serde_json::to_string(p).map_err(err)?;
        self.lib.save_preset(&PresetRow {
            id: p.id.clone(),
            name: p.name.clone(),
            group: p.group.clone(),
            position,
            body,
        })
    }

    /// Cria um preset a partir dos ajustes atuais (só os componentes escolhidos).
    pub fn create(
        &self,
        name: &str,
        group: &str,
        components: &[Component],
        current: &Settings,
    ) -> CoreResult<Preset> {
        let name = name.trim();
        if name.is_empty() {
            return Err(err("o preset precisa de um nome"));
        }
        if components.is_empty() {
            return Err(err("escolha pelo menos um componente"));
        }
        let p = Preset {
            id: new_id(),
            name: name.into(),
            group: group.trim().into(),
            components: components.to_vec(),
            settings: capture(current, components),
            builtin: false,
            description: String::new(),
        };
        self.save(&p, None)?;
        Ok(p)
    }

    pub fn rename(&self, id: &str, name: &str) -> CoreResult<Preset> {
        let mut p = self.get(id)?;
        if name.trim().is_empty() {
            return Err(err("o preset precisa de um nome"));
        }
        p.name = name.trim().into();
        self.save(&p, None)?;
        Ok(p)
    }

    /// Muda de grupo e/ou posição (organizar).
    pub fn move_to(&self, id: &str, group: &str, position: Option<i64>) -> CoreResult<Preset> {
        let mut p = self.get(id)?;
        p.group = group.trim().into();
        self.save(&p, position)?;
        Ok(p)
    }

    /// Ordem dentro de um grupo (lista de ids pela nova ordem).
    pub fn reorder(&self, ids: &[String]) -> CoreResult<()> {
        for (i, id) in ids.iter().enumerate() {
            let p = self.get(id)?;
            self.save(&p, Some(i as i64))?;
        }
        Ok(())
    }

    /// Duplica (incluindo internos): o novo fica no mesmo grupo, ou "Os meus presets".
    pub fn duplicate(&self, id: &str, copy_suffix: &str) -> CoreResult<Preset> {
        let mut p = self.get(id)?;
        p.id = new_id();
        p.name = format!("{} {copy_suffix}", p.name);
        if p.builtin {
            p.builtin = false;
            p.group = String::new();
        }
        self.save(&p, None)?;
        Ok(p)
    }

    pub fn delete(&self, id: &str) -> CoreResult<()> {
        if id.starts_with(BUILTIN_PREFIX) {
            return Err(err("os presets internos não podem ser eliminados"));
        }
        if !self.lib.delete_preset(id)? {
            return Err(err(format!("preset {id} inexistente")));
        }
        Ok(())
    }

    fn store_lut(&self, text: &str) -> CoreResult<(String, lut::Lut)> {
        let l = lut::parse_cube(text).map_err(err)?;
        let id = lut::lut_id(text.as_bytes());
        std::fs::create_dir_all(&self.lut_dir).map_err(err)?;
        let path = self.lut_dir.join(format!("{id}.cube"));
        if !path.is_file() {
            std::fs::write(&path, text).map_err(err)?;
        }
        lut::register(&id, l.clone());
        Ok((id, l))
    }

    /// Importa `.momentopreset`, `.xmp` (aproximado, com relatório) ou `.cube` (LUT).
    pub fn import(&self, path: &Path) -> CoreResult<ImportResult> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| err(format!("não foi possível ler o ficheiro: {e}")))?;
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        match ext.as_str() {
            "cube" => {
                let (id, l) = self.store_lut(&text)?;
                let name = if l.title.is_empty() { stem } else { l.title.clone() };
                let mut s = Settings::default();
                s.lut.id = id;
                s.lut.name = name.clone();
                s.lut.amount = 100.0;
                let p = Preset {
                    id: new_id(),
                    name,
                    group: "LUT".into(),
                    components: vec![Component::Lut],
                    settings: s,
                    builtin: false,
                    description: format!(
                        "LUT {} {}³ (sRGB)",
                        if l.three_d { "3D" } else { "1D" },
                        l.size
                    ),
                };
                self.save(&p, None)?;
                Ok(ImportResult { presets: vec![p], report: None, kind: "cube".into() })
            }
            "xmp" => {
                let (mut p, report) = momento_presets::xmp::import(&text, &stem).map_err(err)?;
                p.id = new_id();
                self.save(&p, None)?;
                Ok(ImportResult { presets: vec![p], report: Some(report), kind: "xmp".into() })
            }
            "lrtemplate" => Err(err(
                "presets .lrtemplate (Lightroom antigo) não são suportados: exporte-os como XMP",
            )),
            _ => {
                let f = file::import(&text).map_err(err)?;
                for cube in f.luts.values() {
                    self.store_lut(cube)?;
                }
                let mut out = Vec::new();
                for mut p in f.presets {
                    p.id = new_id();
                    p.builtin = false;
                    self.save(&p, None)?;
                    out.push(p);
                }
                Ok(ImportResult { presets: out, report: None, kind: "momentopreset".into() })
            }
        }
    }

    /// Exporta presets (com as LUT que usam) para um ficheiro `.momentopreset`.
    pub fn export(&self, ids: &[String], path: &Path) -> CoreResult<usize> {
        let all = self.list()?;
        let chosen: Vec<Preset> =
            ids.iter().filter_map(|id| all.iter().find(|p| &p.id == id).cloned()).collect();
        if chosen.is_empty() {
            return Err(err("nenhum preset escolhido"));
        }
        let mut luts = BTreeMap::new();
        for p in &chosen {
            if p.components.contains(&Component::Lut) && !p.settings.lut.id.is_empty() {
                let id = &p.settings.lut.id;
                let f = self.lut_dir.join(format!("{id}.cube"));
                let text =
                    std::fs::read_to_string(&f).map_err(|_| err(format!("LUT {id} em falta")))?;
                luts.insert(id.clone(), text);
            }
        }
        std::fs::write(path, file::export(&chosen, luts)).map_err(err)?;
        Ok(chosen.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_lifecycle_import_export() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open_in_memory().unwrap();
        let st = PresetStore { lib: &lib, lut_dir: tmp.path().join("luts") };
        assert_eq!(st.list().unwrap().len(), 13);
        let mut cur = Settings::default();
        cur.light.contrast = 22.0;
        cur.light.exposure = 1.0;
        let p = st.create("Meu look", "Clientes", &[Component::Tone], &cur).unwrap();
        assert_eq!((p.settings.light.contrast, p.settings.light.exposure), (22.0, 0.0));
        assert!(st.create(" ", "", &[Component::Tone], &cur).is_err());
        let r = st.rename(&p.id, "Look da Ana").unwrap();
        assert_eq!(r.name, "Look da Ana");
        let d = st.duplicate(&p.id, "(cópia)").unwrap();
        assert_eq!(d.name, "Look da Ana (cópia)");
        let bd = st.duplicate("builtin:film-warm", "(cópia)").unwrap();
        assert!(!bd.builtin && bd.group.is_empty());
        st.move_to(&d.id, "Arquivo", None).unwrap();
        st.reorder(std::slice::from_ref(&d.id)).unwrap();
        assert!(st.delete("builtin:film-warm").is_err());
        assert!(st.rename("builtin:film-warm", "x").is_err());
        st.delete(&bd.id).unwrap();
        let list = st.list().unwrap();
        assert_eq!(list.len(), 15);
        assert_eq!(list.iter().find(|x| x.id == d.id).unwrap().group, "Arquivo");
        // LUT .cube → preset com o componente LUT; exportar e importar noutro "computador".
        let mut cube = String::from("TITLE \"Quente\"\nLUT_3D_SIZE 2\n");
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    cube +=
                        &format!("{} {} {}\n", (r as f32 * 0.9 + 0.1).min(1.0), g, b as f32 * 0.8);
                }
            }
        }
        let cpath = tmp.path().join("quente.cube");
        std::fs::write(&cpath, &cube).unwrap();
        let imp = st.import(&cpath).unwrap();
        assert_eq!((imp.kind.as_str(), imp.presets[0].name.as_str()), ("cube", "Quente"));
        let lp = imp.presets[0].clone();
        assert!(tmp.path().join("luts").join(format!("{}.cube", lp.settings.lut.id)).is_file());
        let out = tmp.path().join("export.momentopreset");
        assert_eq!(st.export(&[lp.id.clone(), r.id.clone()], &out).unwrap(), 2);
        let lib2 = Library::open_in_memory().unwrap();
        let st2 = PresetStore { lib: &lib2, lut_dir: tmp.path().join("luts2") };
        let back = st2.import(&out).unwrap();
        assert_eq!(back.presets.len(), 2);
        assert!(tmp.path().join("luts2").join(format!("{}.cube", lp.settings.lut.id)).is_file());
        // LUT log: recusada com explicação.
        let bad = tmp.path().join("slog.cube");
        std::fs::write(&bad, "LUT_3D_SIZE 2\nDOMAIN_MIN -0.1 -0.1 -0.1\n").unwrap();
        assert!(st.import(&bad).unwrap_err().to_string().contains("log"));
        let lr = tmp.path().join("x.lrtemplate");
        std::fs::write(&lr, "s = {}").unwrap();
        assert!(st.import(&lr).is_err());
    }
}
