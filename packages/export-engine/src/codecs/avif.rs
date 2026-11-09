//! AVIF (AV1 em HEIF) com o codificador rav1e e o contentor avif-serialize.
//!
//! Cor sinalizada por CICP (sem ICC — o avif-serialize não escreve perfis): sRGB,
//! Display P3, Rec. 709 e Rec. 2020; 8, 10 ou 12 bits; 4:4:4, gama completa.

use super::{err, Ctx, Pixels};
use crate::recipe::BitDepth;
use avif_serialize::constants as k;
use momento_color_management::ColorSpace;
use momento_core_types::{CoreError, CoreResult};
use rav1e::prelude::*;

/// Qualidade (1–100) → quantizador do AV1 (0–255), a mesma escala do ravif.
fn quantizer(quality: u8) -> u8 {
    let q = quality as f32 / 100.0;
    let x = if q >= 0.82 {
        (1.0 - q) * 2.6
    } else if q > 0.25 {
        q.mul_add(-0.5, 1.0 - 0.125)
    } else {
        1.0 - q
    };
    (x * 255.0).round() as u8
}

fn frame<P: Pixel + Default>(
    ctx: &mut Context<P>,
    planes: &[Vec<P>; 3],
    w: usize,
) -> Result<Vec<u8>, EncoderStatus> {
    let mut f = ctx.new_frame();
    for (pl, src) in f.planes.iter_mut().zip(planes) {
        let mut s = pl.mut_slice(Default::default());
        for (row, line) in s.rows_iter_mut().zip(src.chunks_exact(w)) {
            row[..w].copy_from_slice(line);
        }
    }
    ctx.send_frame(f)?;
    ctx.flush();
    let mut out = Vec::new();
    loop {
        match ctx.receive_packet() {
            Ok(mut p) => {
                if p.frame_type == FrameType::KEY {
                    out.append(&mut p.data);
                }
            }
            Err(EncoderStatus::Encoded) => continue,
            Err(EncoderStatus::LimitReached) => break,
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

pub fn encode(p: &Pixels, c: &Ctx) -> CoreResult<Vec<u8>> {
    let (prim, transfer, matrix, kprim, ktrans, kmat) = match c.space {
        ColorSpace::Srgb => (
            ColorPrimaries::BT709,
            TransferCharacteristics::SRGB,
            MatrixCoefficients::BT709,
            k::ColorPrimaries::Bt709,
            k::TransferCharacteristics::Srgb,
            k::MatrixCoefficients::Bt709,
        ),
        ColorSpace::DisplayP3 => (
            ColorPrimaries::SMPTE432,
            TransferCharacteristics::SRGB,
            MatrixCoefficients::BT709,
            k::ColorPrimaries::DisplayP3,
            k::TransferCharacteristics::Srgb,
            k::MatrixCoefficients::Bt709,
        ),
        ColorSpace::Rec709 => (
            ColorPrimaries::BT709,
            TransferCharacteristics::BT709,
            MatrixCoefficients::BT709,
            k::ColorPrimaries::Bt709,
            k::TransferCharacteristics::Bt709,
            k::MatrixCoefficients::Bt709,
        ),
        ColorSpace::Rec2020 => (
            ColorPrimaries::BT2020,
            TransferCharacteristics::BT2020_10Bit,
            MatrixCoefficients::BT2020NCL,
            k::ColorPrimaries::Bt2020,
            k::TransferCharacteristics::Bt2020_10,
            k::MatrixCoefficients::Bt2020Ncl,
        ),
        _ => {
            return Err(CoreError::UnsupportedFormat(
                "AVIF: só sRGB, Display P3, Rec. 709 e Rec. 2020".into(),
            ))
        }
    };
    let (kr, kb) = if matrix == MatrixCoefficients::BT2020NCL {
        (0.2627f32, 0.0593f32)
    } else {
        (0.2126, 0.0722)
    };
    let bits: usize = match c.recipe.depth {
        BitDepth::B10 => 10,
        BitDepth::B12 => 12,
        _ => 8,
    };
    let (w, h) = (p.w as usize, p.h as usize);
    let max = ((1u32 << bits) - 1) as f32;
    // RGB (0–1, já codificado) → Y'CbCr de gama completa.
    let n = w * h;
    let mut planes: [Vec<u16>; 3] = [vec![0; n], vec![0; n], vec![0; n]];
    for (i, q) in p.px.iter().enumerate() {
        let [r, g, b] = q.map(|v| v.clamp(0.0, 1.0));
        let y = kr * r + (1.0 - kr - kb) * g + kb * b;
        let cb = (b - y) / (2.0 * (1.0 - kb)) + 0.5;
        let cr = (r - y) / (2.0 * (1.0 - kr)) + 0.5;
        planes[0][i] = (y * max).round().clamp(0.0, max) as u16;
        planes[1][i] = (cb * max).round().clamp(0.0, max) as u16;
        planes[2][i] = (cr * max).round().clamp(0.0, max) as u16;
    }
    let qz = quantizer(c.recipe.quality);
    let threads = rayon::current_num_threads();
    let mut speed = SpeedSettings::from_preset(6);
    speed.scene_detection_mode = SceneDetectionSpeed::None;
    let cfg = Config::new()
        .with_encoder_config(EncoderConfig {
            width: w,
            height: h,
            bit_depth: bits,
            chroma_sampling: ChromaSampling::Cs444,
            pixel_range: PixelRange::Full,
            color_description: Some(ColorDescription {
                color_primaries: prim,
                transfer_characteristics: transfer,
                matrix_coefficients: matrix,
            }),
            still_picture: true,
            quantizer: qz as usize,
            min_quantizer: qz,
            speed_settings: speed,
            tiles: threads.min(n / (256 * 256)).max(1),
            ..Default::default()
        })
        .with_threads(threads);
    let av1 = if bits == 8 {
        let pl = planes.map(|v| v.into_iter().map(|x| x as u8).collect::<Vec<u8>>());
        let mut ctx: Context<u8> = cfg.new_context().map_err(|e| err("AVIF", e))?;
        frame(&mut ctx, &pl, w)
    } else {
        let mut ctx: Context<u16> = cfg.new_context().map_err(|e| err("AVIF", e))?;
        frame(&mut ctx, &planes, w)
    }
    .map_err(|e| err("AVIF", format!("{e:?}")))?;
    let mut a = avif_serialize::Aviffy::new();
    a.set_color_primaries(kprim)
        .set_transfer_characteristics(ktrans)
        .set_matrix_coefficients(kmat)
        .set_full_color_range(true);
    if let Some(e) = &c.exif {
        a.set_exif(e.clone());
    }
    Ok(a.to_vec(&av1, None, p.w, p.h, bits as u8))
}
