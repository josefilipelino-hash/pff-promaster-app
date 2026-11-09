//! Receitas incluídas e o plano "Exportar casamento".

use crate::recipe::*;
use momento_color_management::ColorSpace;
use serde::{Deserialize, Serialize};

fn jpeg(id: &str, name: &str, q: u8, size: Size, folder: &str) -> Recipe {
    Recipe {
        id: id.into(),
        name: name.into(),
        format: Format::Jpeg,
        quality: q,
        size,
        naming: Naming { subfolder: folder.into(), ..Default::default() },
        ..Default::default()
    }
}

fn long(px: f32) -> Size {
    Size { mode: SizeMode::LongEdge, a: px, b: px, ..Default::default() }
}

fn fill(w: f32, h: f32) -> Size {
    // Redes sociais: medidas exatas, mesmo que seja preciso ampliar uma imagem pequena.
    Size { mode: SizeMode::Fill, a: w, b: h, ppi: 72.0, no_enlarge: false, ..Default::default() }
}

fn screen(amount: SharpenAmount) -> Sharpen {
    Sharpen { target: SharpenTarget::Screen, amount }
}

fn web_meta() -> Metadata {
    Metadata { mode: MetadataMode::Copyright, ..Default::default() }
}

/// As 10 receitas incluídas (ids estáveis, prefixo "builtin:").
pub fn builtin() -> Vec<Recipe> {
    vec![
        Recipe {
            metadata: Metadata { mode: MetadataMode::All, ..Default::default() },
            ..jpeg(
                "builtin:alta-resolucao",
                "Alta Resolução",
                95,
                Size::default(),
                "Entrega/AltaResolucao",
            )
        },
        Recipe {
            sharpen: screen(SharpenAmount::Standard),
            ..jpeg("builtin:galeria", "Galeria", 85, long(2560.0), "Galeria")
        },
        Recipe {
            sharpen: screen(SharpenAmount::Standard),
            metadata: web_meta(),
            ..jpeg("builtin:website", "Website", 80, long(2048.0), "Website")
        },
        Recipe {
            sharpen: screen(SharpenAmount::Low),
            metadata: web_meta(),
            ..jpeg("builtin:email", "Email", 75, long(1200.0), "Email")
        },
        Recipe {
            sharpen: screen(SharpenAmount::Standard),
            metadata: web_meta(),
            ..jpeg("builtin:instagram-4x5", "Instagram 4:5", 90, fill(1080.0, 1350.0), "Instagram")
        },
        Recipe {
            sharpen: screen(SharpenAmount::Standard),
            metadata: web_meta(),
            ..jpeg(
                "builtin:instagram-square",
                "Instagram Square",
                90,
                fill(1080.0, 1080.0),
                "Instagram",
            )
        },
        Recipe {
            sharpen: Sharpen { target: SharpenTarget::Glossy, amount: SharpenAmount::Standard },
            ..jpeg("builtin:impressao", "Impressão", 100, Size::default(), "Impressao")
        },
        Recipe {
            sharpen: Sharpen { target: SharpenTarget::Matte, amount: SharpenAmount::Low },
            ..jpeg("builtin:album", "Album", 95, long(4000.0), "Album")
        },
        Recipe {
            id: "builtin:arquivo-tiff".into(),
            name: "Arquivo TIFF".into(),
            format: Format::Tiff,
            depth: BitDepth::B16,
            compression: Compression::Zip,
            color: ColorSpace::AdobeRgb,
            metadata: Metadata { mode: MetadataMode::All, remove_gps: false, ..Default::default() },
            naming: Naming { subfolder: "Arquivo".into(), ..Default::default() },
            ..Default::default()
        },
        Recipe {
            id: "builtin:arquivo-dng".into(),
            name: "Arquivo DNG".into(),
            format: Format::Dng,
            depth: BitDepth::B16,
            compression: Compression::None,
            metadata: Metadata { mode: MetadataMode::All, remove_gps: false, ..Default::default() },
            naming: Naming { subfolder: "Arquivo".into(), ..Default::default() },
            ..Default::default()
        },
    ]
}

/// Plano de exportação: várias receitas executadas numa só operação.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Plan {
    pub id: String,
    pub name: String,
    pub recipes: Vec<Recipe>,
}

/// "Exportar casamento": entrega em alta resolução, galeria, Instagram, impressão e
/// arquivo — cada uma na sua pasta.
pub fn wedding_plan() -> Plan {
    let all = builtin();
    let pick = |id: &str| all.iter().find(|r| r.id == id).cloned().expect("receita incluída");
    Plan {
        id: "builtin:casamento".into(),
        name: "Exportar casamento".into(),
        recipes: vec![
            pick("builtin:alta-resolucao"),
            pick("builtin:galeria"),
            pick("builtin:instagram-4x5"),
            pick("builtin:impressao"),
            pick("builtin:arquivo-tiff"),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_consistent() {
        let all = builtin();
        assert_eq!(all.len(), 10);
        let mut ids: Vec<_> = all.iter().map(|r| r.id.clone()).collect();
        ids.dedup();
        assert_eq!(ids.len(), 10);
        for r in &all {
            let (n, notes) = r.normalized();
            if r.format.caps().available {
                assert!(notes.is_empty(), "{}: {notes:?}", r.name);
                assert_eq!(&n, r);
            }
        }
        let plan = wedding_plan();
        let folders: Vec<_> = plan.recipes.iter().map(|r| r.naming.subfolder.as_str()).collect();
        assert_eq!(
            folders,
            ["Entrega/AltaResolucao", "Galeria", "Instagram", "Impressao", "Arquivo"]
        );
    }
}
