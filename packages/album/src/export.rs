//! Exportação do álbum: JPEG ou TIFF por spread (ou por página), PDF de impressão (com
//! sangria e `TrimBox`) e PDF de prova (resolução reduzida, linhas de corte, zona segura,
//! dobra e "PROVA").

use crate::compose::{compose, split, Provider};
use crate::model::Album;
use crate::pdf::{self, PdfPage};
use crate::spec::ExportUnit;
use momento_core_types::{CoreError, CoreResult};
use momento_export_engine::{
    BitDepth, Compression, Format, Metadata, MetadataMode, PhotoMeta, Prepared, Recipe, Size,
    Watermark,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportKind {
    Jpeg,
    Tiff,
    PrintPdf,
    ProofPdf,
}

/// DPI da prova (leve para enviar ao cliente).
pub const PROOF_DPI: f32 = 120.0;

fn recipe(album: &Album, kind: ExportKind) -> Recipe {
    let s = &album.spec;
    let proof = kind == ExportKind::ProofPdf;
    Recipe {
        name: album.name.clone(),
        format: if kind == ExportKind::Tiff { Format::Tiff } else { Format::Jpeg },
        depth: BitDepth::B8,
        quality: if proof { 80 } else { 95 },
        compression: if kind == ExportKind::Tiff { Compression::Lzw } else { Compression::Lossy },
        color: s.color,
        icc_path: s.icc_path.clone(),
        intent: s.intent,
        size: Size { ppi: if proof { PROOF_DPI } else { s.dpi as f32 }, ..Default::default() },
        metadata: Metadata { mode: MetadataMode::None, ..Default::default() },
        watermark: if proof {
            Watermark {
                enabled: true,
                text: "PROVA".into(),
                size: 7.0,
                opacity: 22.0,
                anchor: momento_export_engine::Anchor::Center,
                color: "#d02020".into(),
                ..Default::default()
            }
        } else {
            Watermark::default()
        },
        ..Default::default()
    }
}

/// Exporta. `sink(nome, bytes)` grava cada ficheiro; `progress(feitos, total)` devolve
/// `false` para cancelar.
pub fn export(
    album: &Album,
    kind: ExportKind,
    photos: &Provider,
    sink: &mut dyn FnMut(&str, Vec<u8>) -> CoreResult<()>,
    progress: &dyn Fn(usize, usize) -> bool,
) -> CoreResult<usize> {
    let s = &album.spec;
    if let Some(p) = s.problems().first() {
        return Err(CoreError::Other(p.to_string()));
    }
    let prep = Prepared::new(&recipe(album, kind))?;
    let ppm = if kind == ExportKind::ProofPdf { PROOF_DPI / 25.4 } else { s.px_per_mm() };
    let total = album.spreads.len();
    let mut pages = Vec::new();
    let mut files = 0;
    let base = crate::slug(&album.name);
    for (i, sp) in album.spreads.iter().enumerate() {
        if !progress(i, total) {
            return Err(CoreError::Other("cancelada".into()));
        }
        let proof = kind == ExportKind::ProofPdf;
        let img = compose(s, sp, photos, ppm, proof);
        // Prova: sempre spreads (mostra a dobra); o resto conforme o laboratório.
        let parts = if proof { vec![img] } else { split(s, img, ppm) };
        let n_parts = parts.len();
        for (k, part) in parts.into_iter().enumerate() {
            let out = prep.produce(&part, &PhotoMeta::default())?;
            match kind {
                ExportKind::Jpeg | ExportKind::Tiff => {
                    let ext = if kind == ExportKind::Tiff { "tif" } else { "jpg" };
                    let name = if s.export_unit == ExportUnit::Pages {
                        format!("{base}-pagina-{:03}.{ext}", i * 2 + k + 1)
                    } else {
                        format!("{base}-spread-{:02}.{ext}", i + 1)
                    };
                    sink(&name, out.bytes)?;
                    files += 1;
                }
                ExportKind::PrintPdf | ExportKind::ProofPdf => {
                    let pt = |px: u32| px as f32 / ppm / 25.4 * 72.0;
                    pages.push(PdfPage {
                        width_pt: pt(out.width),
                        height_pt: pt(out.height),
                        bleed_pt: s.bleed_mm / 25.4 * 72.0,
                        jpeg: out.bytes,
                        width_px: out.width,
                        height_px: out.height,
                    });
                    let _ = n_parts;
                }
            }
        }
    }
    if matches!(kind, ExportKind::PrintPdf | ExportKind::ProofPdf) {
        let suffix = if kind == ExportKind::ProofPdf { "prova" } else { "impressao" };
        sink(&format!("{base}-{suffix}.pdf"), pdf::write(&pages, prep.icc(), &album.name))?;
        files += 1;
    }
    progress(total, total);
    Ok(files)
}
