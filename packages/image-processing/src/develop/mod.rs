//! Motor de revelação **não destrutivo**: os parâmetros ([`Settings`]) são aplicados a uma
//! cópia em memória do original ([`Source`]); o ficheiro original nunca é escrito.
//!
//! Ver `render` para a ordem das operações e `color` para a gestão de cor.

pub mod auto;
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
    render, render_with_overlay, sample_for_white_balance, Quality, RenderOptions, Rendered,
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
