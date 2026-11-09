//! Avaliação visual da edição automática: importa uma pasta para um projeto temporário,
//! aplica a edição automática a cada fotografia e grava comparações antes|depois (JPEG)
//! numa pasta de saída, com o relatório de cada uma. Nada é escrito nos originais.
//! `cargo run --release -p momento-project-management --example auto_eval -- <fotos> <saída> [intensidade]`
use image::{imageops, RgbImage};
use momento_database::import::ImportMode;
use momento_image_processing::develop::auto::{AutoOptions, Intensity};
use momento_image_processing::develop::{render_rgb, Quality, Settings};
use momento_project_management::*;
use std::sync::Arc;

struct Quiet;
impl SessionEvents for Quiet {
    fn import_progress(&self, _: &momento_import_engine::ImportProgress) {}
    fn import_finished(&self, _: &ImportSummary) {}
    fn thumbnails(&self, _: &thumbnails::ThumbBatch) {}
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(&a[2]);
    std::fs::create_dir_all(&out).unwrap();
    let intensity = match a.get(3).map(String::as_str) {
        Some("muito-suave") => Intensity::VerySoft,
        Some("suave") => Intensity::Soft,
        Some("equilibrada") => Intensity::Balanced,
        Some("intensa") => Intensity::Strong,
        _ => Intensity::Natural,
    };
    let work = tempfile::tempdir().unwrap();
    let paths = create_project(&NewProject {
        name: "Avaliação".into(),
        client_name: "-".into(),
        event_type: "wedding".into(),
        event_date: None,
        location: String::new(),
        originals_dir: None,
        work_dir: work.path().to_string_lossy().into_owned(),
    })
    .unwrap();
    let s = ProjectSession::open(&paths.root, Arc::new(Quiet)).unwrap();
    s.start_import(&ImportRequest {
        paths: vec![a[1].clone()],
        mode: ImportMode::Reference,
        recursive: false,
    })
    .unwrap();
    s.wait_import();
    let opts = AutoOptions { intensity, red_eye: true, ..Default::default() };
    for p in s.list_photos().unwrap() {
        let t = std::time::Instant::now();
        let r = match s.auto_edit_photo(p.id.0, &opts) {
            Ok(r) => r,
            Err(e) => {
                println!("{}: erro {e}", p.file_name);
                continue;
            }
        };
        let el = t.elapsed();
        let render = |st: &Settings| -> RgbImage {
            let f = s.render_edit(p.id.0, st, 900, 900, RenderQuality::Final, false).unwrap();
            let (w, h) = (
                u32::from_le_bytes(f[0..4].try_into().unwrap()),
                u32::from_le_bytes(f[4..8].try_into().unwrap()),
            );
            let rgb: Vec<u8> =
                f[8 + 3 * 256 * 4..].chunks_exact(4).flat_map(|c| [c[0], c[1], c[2]]).collect();
            RgbImage::from_raw(w, h, rgb).unwrap()
        };
        let before = render(&Settings::default());
        let after = render(&r.view.settings);
        let after = imageops::resize(
            &after,
            before.width(),
            before.height(),
            imageops::FilterType::Triangle,
        );
        let mut both = RgbImage::new(before.width() * 2 + 8, before.height());
        imageops::replace(&mut both, &before, 0, 0);
        imageops::replace(&mut both, &after, before.width() as i64 + 8, 0);
        let name = format!("{}.jpg", p.file_name.replace('.', "_"));
        both.save(out.join(&name)).unwrap();
        let st = &r.view.settings;
        println!(
            "{} [{el:.1?}] exp {:+.2} wb {:+.0}/{:+.0} hl {} sh {} wh {} bl {} ct {} vib {} | ruído {:.1}/{:.1} {:?} iso {:?} → NR {} cor {} AI {} | nitidez {} | horizonte {:?}→{} | CA {}/{} vinheta {} | rostos {} olhos-v {}",
            p.file_name,
            st.light.exposure,
            st.wb.temperature,
            st.wb.tint,
            st.light.highlights,
            st.light.shadows,
            st.light.whites,
            st.light.blacks,
            st.light.contrast,
            st.color.vibrance,
            r.report.noise.luminance,
            r.report.noise.chroma,
            r.report.noise.level,
            r.report.noise.iso,
            st.detail.noise_luminance,
            st.detail.noise_color,
            st.detail.ai_denoise,
            st.detail.sharpen_amount,
            r.report.tilt,
            st.geometry.straighten,
            st.lens.ca_red,
            st.lens.ca_blue,
            st.lens.vignetting,
            r.report.faces,
            r.report.red_eyes,
        );
        let _ = render_rgb;
        let _ = Quality::Fast;
    }
    s.close();
}
