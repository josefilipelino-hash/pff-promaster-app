//! Motor de revelação **não destrutivo**: os parâmetros ([`Settings`]) são aplicados a uma
//! cópia em memória do original ([`Source`]); o ficheiro original nunca é escrito.
//!
//! Ver `render` para a ordem das operações e `color` para a gestão de cor.

/// Versão do motor de revelação. Entra na chave das edições em cache: quando o motor muda
/// (ex.: auditoria final — realces/sombras locais e perfis ICC de entrada), as imagens
/// guardadas com a versão anterior deixam de ser usadas e são geradas de novo.
pub const PIPELINE_VERSION: u32 = 8;

pub mod auto;
pub mod calibrate;
pub mod color;
pub mod consistency;
pub mod curves;
pub mod filters;
pub mod geometry;
pub mod local;
pub mod lut;
pub mod portrait;
pub mod render;
pub mod settings;
pub mod sky;
pub mod source;
pub mod upright;

pub use render::{
    render, render_linear, render_with_overlay, sample_for_white_balance, LinearImage, Quality,
    RenderOptions, Rendered,
};
pub use settings::*;
pub use source::Source;

use image::RgbImage;

/// Render em `RgbImage` sRGB (miniaturas, previews, ficheiros).
pub fn render_rgb(src: &Source, s: &Settings, max_edge: u32, quality: Quality) -> RgbImage {
    let r = render(
        src,
        s,
        &RenderOptions { max_width: max_edge, max_height: max_edge, quality, uncropped: false },
    );
    let rgb: Vec<u8> = r.rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    RgbImage::from_raw(r.width, r.height, rgb).expect("dimensões")
}

/// Endireitar automático: mede a inclinação das linhas da imagem (horizonte/verticais) e
/// devolve o ângulo de `straighten` que a corrige (`None` se não houver linhas fiáveis).
pub fn auto_straighten(src: &Source, s: &Settings) -> Option<f32> {
    let base = Settings {
        geometry: Geometry {
            quarter_turns: s.geometry.quarter_turns,
            flip_horizontal: s.geometry.flip_horizontal,
            ..Default::default()
        },
        lens: s.lens.clone(),
        ..Default::default()
    };
    let img = render_rgb(src, &base, 1024, Quality::Fast);
    let tilt = momento_ai_analysis::metrics::technical(&img).tilt_degrees?;
    Some(-tilt)
}

/// Upright: correção automática da perspetiva pelas linhas da fotografia (ver `upright`).
/// Analisa a fotografia orientada (rotações de 90°/espelho e correções da objetiva, sem
/// perspetiva nem endireitar). No modo guiado usa as guias de `s.geometry.guides`.
pub fn upright(
    src: &Source,
    s: &Settings,
    mode: upright::UprightMode,
    focal35: Option<f32>,
) -> Option<upright::UprightResult> {
    let base = Settings {
        geometry: Geometry {
            quarter_turns: s.geometry.quarter_turns,
            flip_horizontal: s.geometry.flip_horizontal,
            ..Default::default()
        },
        lens: s.lens.clone(),
        ..Default::default()
    };
    let img = render_rgb(src, &base, 1280, Quality::Fast);
    let (w, h) = (img.width() as usize, img.height() as usize);
    let segments = if mode == upright::UprightMode::Guided {
        s.geometry
            .guides
            .iter()
            .map(|g| upright::Segment {
                x1: g[0] * w as f32,
                y1: g[1] * h as f32,
                x2: g[2] * w as f32,
                y2: g[3] * h as f32,
            })
            .collect()
    } else {
        let gray: Vec<f32> = img
            .pixels()
            .map(|p| (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0)
            .collect();
        upright::detect_segments(&gray, w, h)
    };
    upright::solve_upright(&segments, w, h, focal35, mode)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod local_tests;

/// Recursos externos de uma edição que não estão disponíveis (LUT, máscaras calculadas).
/// O render ignora-os sem erro — no ecrã é aceitável, numa exportação não: a fotografia
/// sairia com outras cores ou sem os ajustes locais (Fase 15). Vazio = tudo disponível.
pub fn missing_resources(s: &Settings) -> Vec<String> {
    let mut out = Vec::new();
    if !s.lut.id.is_empty() && s.lut.amount > 0.0 && lut::get(&s.lut.id).is_none() {
        out.push(format!("LUT «{}»", if s.lut.name.is_empty() { &s.lut.id } else { &s.lut.name }));
    }
    for m in s.masks.iter().filter(|m| m.enabled) {
        for c in &m.components {
            if let MaskShape::Raster { raster, .. } = &c.shape {
                if local::get_raster(raster).is_none() {
                    out.push(format!("máscara «{}»", m.name));
                }
            }
        }
    }
    out
}
