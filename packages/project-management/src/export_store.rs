//! Receitas e planos de exportação do utilizador (biblioteca global) + os incluídos.

use momento_core_types::{CoreError, CoreResult};
use momento_database::library::{ExportPresetRow, Library};
use momento_export_engine::presets::{builtin, wedding_plan, Plan};
use momento_export_engine::Recipe;

fn new_id() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("x{:x}{:x}", t, n)
}

fn err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Other(e.to_string())
}

pub struct ExportStore<'a> {
    pub lib: &'a Library,
}

impl ExportStore<'_> {
    /// Receitas: incluídas primeiro, depois as do utilizador.
    pub fn recipes(&self) -> CoreResult<Vec<Recipe>> {
        let mut out = builtin();
        for row in self.lib.export_presets("recipe")? {
            match serde_json::from_str::<Recipe>(&row.body) {
                Ok(mut r) => {
                    r.id = row.id;
                    r.name = row.name;
                    out.push(r);
                }
                Err(e) => tracing::warn!(id = row.id, erro = %e, "receita de exportação ilegível"),
            }
        }
        Ok(out)
    }

    /// Grava uma receita do utilizador. Uma receita incluída (ou sem id) gera uma nova.
    pub fn save_recipe(&self, r: &Recipe) -> CoreResult<Recipe> {
        let mut r = r.clone();
        if r.id.is_empty() || r.id.starts_with("builtin:") {
            r.id = new_id();
        }
        if r.name.trim().is_empty() {
            return Err(CoreError::Other("a receita precisa de um nome".into()));
        }
        let position = self.lib.export_presets("recipe")?.len() as i64;
        self.lib.save_export_preset(&ExportPresetRow {
            id: r.id.clone(),
            kind: "recipe".into(),
            name: r.name.clone(),
            position,
            body: serde_json::to_string(&r).map_err(err)?,
        })?;
        Ok(r)
    }

    pub fn delete(&self, id: &str) -> CoreResult<bool> {
        if id.starts_with("builtin:") {
            return Err(CoreError::Other("as receitas incluídas não podem ser apagadas".into()));
        }
        self.lib.delete_export_preset(id)
    }

    pub fn plans(&self) -> CoreResult<Vec<Plan>> {
        let mut out = vec![wedding_plan()];
        for row in self.lib.export_presets("plan")? {
            if let Ok(mut p) = serde_json::from_str::<Plan>(&row.body) {
                p.id = row.id;
                p.name = row.name;
                out.push(p);
            }
        }
        Ok(out)
    }

    pub fn save_plan(&self, p: &Plan) -> CoreResult<Plan> {
        let mut p = p.clone();
        if p.id.is_empty() || p.id.starts_with("builtin:") {
            p.id = new_id();
        }
        if p.name.trim().is_empty() {
            return Err(CoreError::Other("o plano precisa de um nome".into()));
        }
        let position = self.lib.export_presets("plan")?.len() as i64;
        self.lib.save_export_preset(&ExportPresetRow {
            id: p.id.clone(),
            kind: "plan".into(),
            name: p.name.clone(),
            position,
            body: serde_json::to_string(&p).map_err(err)?,
        })?;
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_recipes_and_plans() {
        let lib = Library::open_in_memory().unwrap();
        let s = ExportStore { lib: &lib };
        assert_eq!(s.recipes().unwrap().len(), 10);
        let mut r = s.recipes().unwrap()[1].clone();
        r.name = "Galeria do cliente".into();
        r.quality = 88;
        let saved = s.save_recipe(&r).unwrap();
        assert!(!saved.id.starts_with("builtin:"), "copiar uma incluída gera uma nova");
        let all = s.recipes().unwrap();
        assert_eq!(all.len(), 11);
        assert_eq!(all[10].quality, 88);
        assert!(s.delete("builtin:galeria").is_err());
        let plan =
            Plan { name: "Só web".into(), recipes: vec![saved.clone()], ..Default::default() };
        let p = s.save_plan(&plan).unwrap();
        assert_eq!(s.plans().unwrap().len(), 2);
        assert!(s.delete(&p.id).unwrap());
        assert!(s.delete(&saved.id).unwrap());
        assert_eq!(s.recipes().unwrap().len(), 10);
    }
}
