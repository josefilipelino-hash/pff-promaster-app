use super::*;
use crate::{flip_to_exif_orientation, libraw_camera_count, libraw_version};
use image::RgbImage;
use momento_image_codecs::default_registry;

#[test]
fn version_and_camera_list() {
    assert!(libraw_version().starts_with("0.22.3"));
    assert!(libraw_camera_count() > 1000);
}

#[test]
fn flip_mapping() {
    assert_eq!(flip_to_exif_orientation(0), 1);
    assert_eq!(flip_to_exif_orientation(3), 3);
    assert_eq!(flip_to_exif_orientation(5), 8);
    assert_eq!(flip_to_exif_orientation(6), 6);
}

#[test]
fn invalid_and_truncated_data_never_crash() {
    let codec = RawCodec::new(default_registry());
    // Texto, zeros, cabeçalho TIFF sem nada, JPEG fingindo ser RAW.
    let inputs: Vec<Vec<u8>> = vec![
        b"isto nao e um raw".to_vec(),
        vec![0; 100_000],
        b"II*\0\x08\0\0\0\0\0\0\0".to_vec(),
        b"FUJIFILMCCD-RAW 0201FF383501".to_vec(),
        momento_test_fixtures::jpeg_bytes(
            &momento_test_fixtures::test_image(64, 64, 1, 1),
            None,
            80,
        ),
    ];
    for d in &inputs {
        assert!(codec.probe(d, FileFormat::TiffBasedRaw).is_err());
        assert!(codec.decode(d, FileFormat::TiffBasedRaw, Some(100)).is_err());
        assert!(codec.metadata(d, FileFormat::TiffBasedRaw).is_none());
    }
    // Pseudoaleatório com cabeçalhos RAW reais no início.
    let mut x: u32 = 0xdead_beef;
    for header in
        [&b"II*\0"[..], b"MM\0*", b"FUJIFILMCCD-RAW ", b"IIRO", b"IIU\0", b"FOVb", b"\0MRM"]
    {
        for _ in 0..30 {
            let mut buf = header.to_vec();
            for _ in 0..20_000 {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                buf.push(x as u8);
            }
            let _ = codec.decode(&buf, FileFormat::TiffBasedRaw, Some(100));
        }
    }
}

#[test]
fn pre_rotated_previews_are_normalized() {
    use crate::codec::normalize_preview_rotation;
    // Sensor paisagem 300×200, orientação 6, preview já rodada (retrato 100×150).
    let portrait = RgbImage::new(100, 150);
    let n = normalize_preview_rotation(portrait, 300, 200, 6);
    assert_eq!(n.dimensions(), (150, 100), "volta à orientação do sensor");
    // Preview na orientação do sensor: inalterada.
    let landscape = RgbImage::new(150, 100);
    assert_eq!(normalize_preview_rotation(landscape, 300, 200, 6).dimensions(), (150, 100));
    // Sem rotação de 90°: nunca mexe.
    assert_eq!(
        normalize_preview_rotation(RgbImage::new(100, 150), 300, 200, 3).dimensions(),
        (100, 150)
    );
}
