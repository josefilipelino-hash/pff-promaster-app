//! Miniaturas e previews.

use image::{imageops, ImageEncoder, RgbImage};
use momento_core_types::{CoreError, CoreResult};
use momento_image_codecs::{CodecRegistry, FileFormat};
use std::io::Write;
use std::path::Path;

/// Parâmetros de uma preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewSpec {
    /// Lado maior máximo do resultado (`None` = resolução total).
    pub max_edge: Option<u32>,
    pub quality: u8,
}

impl PreviewSpec {
    /// Miniatura da grelha: nítida em ecrãs Retina até ~320 px de célula.
    pub const THUMB: PreviewSpec = PreviewSpec { max_edge: Some(640), quality: 82 };
    /// Preview do visualizador em modo “ajustar”.
    pub const PREVIEW: PreviewSpec = PreviewSpec { max_edge: Some(2560), quality: 90 };
    /// Resolução total para zoom 100 % (formatos que o WebView não mostra diretamente).
    pub const FULL: PreviewSpec = PreviewSpec { max_edge: None, quality: 92 };
}

/// Aplica a orientação EXIF (1–8) aos píxeis.
pub fn apply_orientation(img: RgbImage, orientation: u8) -> RgbImage {
    match orientation {
        2 => imageops::flip_horizontal(&img),
        3 => imageops::rotate180(&img),
        4 => imageops::flip_vertical(&img),
        // 5 = transposição (troca x↔y); 7 = transversa. Ver EXIF 2.32, tag 0x0112.
        5 => imageops::rotate270(&imageops::flip_horizontal(&img)),
        6 => imageops::rotate90(&img),
        7 => imageops::rotate90(&imageops::flip_horizontal(&img)),
        8 => imageops::rotate270(&img),
        _ => img,
    }
}

/// Codifica em JPEG, embutindo o perfil ICC quando existe.
pub fn encode_jpeg(img: &RgbImage, quality: u8, icc: Option<&[u8]>) -> CoreResult<Vec<u8>> {
    let mut out = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    if let Some(p) = icc {
        // Um perfil inválido não impede a miniatura: apenas não é embutido.
        let _ = enc.set_icc_profile(p.to_vec());
    }
    enc.write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgb8)
        .map_err(|e| CoreError::Other(format!("codificação JPEG: {e}")))?;
    Ok(out)
}

/// Gera uma preview JPEG orientada a partir dos bytes do original.
pub fn render_preview(
    registry: &CodecRegistry,
    data: &[u8],
    format: FileFormat,
    orientation: u8,
    spec: PreviewSpec,
) -> CoreResult<Vec<u8>> {
    // Para reduzir no JPEG, o lado menor descodificado tem de cobrir o lado maior pedido
    // proporcionalmente; calcula-se a partir do probe.
    let min_short = match spec.max_edge {
        Some(max) => {
            let p = registry.probe(data, format)?;
            let (long, short) = (p.width.max(p.height), p.width.min(p.height));
            Some(((max as u64 * short as u64) / long.max(1) as u64).max(1) as u32)
        }
        None => None,
    };
    let decoded = registry.decode(data, format, min_short)?;
    let mut img = decoded.image;
    if let Some(max) = spec.max_edge {
        let (w, h) = img.dimensions();
        if w.max(h) > max {
            let scale = max as f64 / w.max(h) as f64;
            let nw = ((w as f64 * scale).round() as u32).max(1);
            let nh = ((h as f64 * scale).round() as u32).max(1);
            img = imageops::thumbnail(&img, nw, nh);
        }
    }
    let img = apply_orientation(img, orientation);
    encode_jpeg(&img, spec.quality, decoded.icc_profile.as_deref())
}

/// Gera a preview (2560 px) e a miniatura (640 px) a partir de **uma** descodificação.
pub fn render_preview_and_thumb(
    registry: &CodecRegistry,
    data: &[u8],
    format: FileFormat,
    orientation: u8,
) -> CoreResult<(Vec<u8>, Vec<u8>)> {
    let pmax = PreviewSpec::PREVIEW.max_edge.unwrap_or(2560);
    let p = registry.probe(data, format)?;
    let (long, short) = (p.width.max(p.height), p.width.min(p.height));
    let min_short = ((pmax as u64 * short as u64) / long.max(1) as u64).max(1) as u32;
    let decoded = registry.decode(data, format, Some(min_short))?;
    let icc = decoded.icc_profile.clone();
    let fit = |img: &RgbImage, max: u32| {
        let (w, h) = img.dimensions();
        if w.max(h) <= max {
            return img.clone();
        }
        let s = max as f64 / w.max(h) as f64;
        imageops::thumbnail(
            img,
            ((w as f64 * s).round() as u32).max(1),
            ((h as f64 * s).round() as u32).max(1),
        )
    };
    let preview = apply_orientation(fit(&decoded.image, pmax), orientation);
    let thumb = fit(&preview, PreviewSpec::THUMB.max_edge.unwrap_or(640));
    Ok((
        encode_jpeg(&preview, PreviewSpec::PREVIEW.quality, icc.as_deref())?,
        encode_jpeg(&thumb, PreviewSpec::THUMB.quality, icc.as_deref())?,
    ))
}

/// Escrita atómica: ficheiro temporário na mesma pasta + `rename`. Um processo terminado
/// a meio nunca deixa uma miniatura corrompida no lugar final.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> CoreResult<()> {
    let dir = path.parent().ok_or_else(|| CoreError::Other("caminho sem pasta".into()))?;
    std::fs::create_dir_all(dir)?;
    // Nome temporário único por processo + contador global: duas threads a gerar a
    // mesma preview nunca escrevem no mesmo ficheiro temporário.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!(
        ".{}.{}-{n}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("f"),
        std::process::id()
    ));
    // Qualquer erro (disco cheio a meio, por exemplo) remove o temporário: nunca fica um
    // ficheiro parcial escondido na pasta de entrega (Fase 15).
    let written = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_data()
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    // No Windows e no macOS, `rename` substitui o destino existente de forma atómica.
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use momento_image_codecs::default_registry;
    use momento_test_fixtures::*;

    fn decode(bytes: &[u8]) -> RgbImage {
        image::load_from_memory(bytes).unwrap().to_rgb8()
    }

    /// A barra vermelha deve estar no topo visual em todas as orientações.
    #[test]
    fn thumbnails_are_correctly_oriented_for_all_8_orientations() {
        let reg = default_registry();
        for o in 1..=8u16 {
            let raw = test_image(600, 400, 2, o);
            let bytes = jpeg_bytes(&raw, None, 92);
            let out = render_preview(
                &reg,
                &bytes,
                FileFormat::Jpeg,
                o as u8,
                PreviewSpec { max_edge: Some(300), quality: 90 },
            )
            .unwrap();
            let t = decode(&out);
            assert_eq!(t.dimensions(), (300, 200), "orientação {o}");
            let top = t.get_pixel(150, 3).0;
            assert!(top[0] > 170 && top[2] < 90, "orientação {o}: topo {top:?}");
            let corner = t.get_pixel(5, 40).0;
            assert!(corner[2] > 150 && corner[0] < 90, "orientação {o}: canto {corner:?}");
        }
    }

    #[test]
    fn preview_never_upscales() {
        let reg = default_registry();
        let bytes = png_bytes(&test_image(200, 100, 1, 1), None);
        let out = render_preview(&reg, &bytes, FileFormat::Png, 1, PreviewSpec::PREVIEW).unwrap();
        assert_eq!(decode(&out).dimensions(), (200, 100));
    }

    #[test]
    fn icc_profile_is_preserved() {
        // Perfil ICC sintético mínimo: o conteúdo não é validado pelo codificador.
        let fake_icc = vec![0u8; 128];
        let img = test_image(64, 64, 1, 1);
        let jpg = encode_jpeg(&img, 80, Some(&fake_icc)).unwrap();
        let reg = default_registry();
        let d = reg.decode(&jpg, FileFormat::Jpeg, None).unwrap();
        assert_eq!(d.icc_profile.as_deref(), Some(&fake_icc[..]));
    }

    #[test]
    fn atomic_write_replaces_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a/b/thumb.jpg");
        write_atomic(&p, b"1").unwrap();
        write_atomic(&p, b"22").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"22");
        assert_eq!(std::fs::read_dir(p.parent().unwrap()).unwrap().count(), 1, "sem temporários");
    }
}
