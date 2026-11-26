//! Diagnóstico da calibração de câmaras sem perfil (ex.: Canon EOS C50), em meia e em
//! resolução total. Uso: `calib_diag <ficheiro.raw>`
use momento_image_codecs::FileFormat;
use momento_image_processing::develop::{calibrate, Source};
use momento_project_management::codecs::full_registry as default_registry;

fn main() {
    let path = std::env::args().nth(1).expect("uso: calib_diag <ficheiro>");
    let data = std::fs::read(&path).unwrap();
    let reg = default_registry();
    let format = FileFormat::CanonCr3;
    let preview = reg.decode(&data, format, Some(480)).unwrap().image;
    for half in [true, false] {
        let mut raw = momento_raw_engine::RawFile::open(&data).unwrap();
        let info = raw.info().clone();
        let bm = raw.develop_linear(half).unwrap();
        let s = Source::from_linear16(
            bm.width,
            bm.height,
            &bm.rgb,
            info.orientation,
            info.width,
            info.height,
        );
        let level = s.levels.last().unwrap();
        let c = calibrate::calibrate_from_preview(level, &preview);
        println!(
            "meia={half} revelado {}x{} nível {}x{} preview {}x{} → {:?}",
            bm.width,
            bm.height,
            level.w,
            level.h,
            preview.width(),
            preview.height(),
            c.map(|c| (c.framing, c.alignment, c.error_before, c.error_after, c.offset))
        );
    }
}
