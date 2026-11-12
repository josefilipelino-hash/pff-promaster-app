//! Motor de revelação **não destrutivo**: os parâmetros ([`Settings`]) são aplicados a uma
//! cópia em memória do original ([`Source`]); o ficheiro original nunca é escrito.
//!
//! Ver `render` para a ordem das operações e `color` para a gestão de cor.

/// Versão do motor de revelação. Entra na chave das edições em cache: quando o motor muda
/// (ex.: auditoria final — realces/sombras locais e perfis ICC de entrada), as imagens
/// guardadas com a versão anterior deixam de ser usadas e são geradas de novo.
pub const PIPELINE_VERSION: u32 = 4;

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
        lens: s.lens,
        ..Default::default()
    };
    let img = render_rgb(src, &base, 1024, Quality::Fast);
    let tilt = momento_ai_analysis::metrics::technical(&img).tilt_degrees?;
    Some(-tilt)
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
