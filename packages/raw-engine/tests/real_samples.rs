#![allow(clippy::format_in_format_args)]
//! Teste de capacidade com ficheiros RAW **reais** (fora do Git).
//!
//! `MOMENTO_SAMPLES_DIR=/caminho cargo test -p momento-raw-engine --release --test real_samples -- --nocapture`
//!
//! Sem a variável o teste é ignorado (nunca se versionam fotografias). Para cada ficheiro:
//! deteção pelo conteúdo, abertura, metadados, preview embutida, revelação a meia
//! resolução e completa. Gera `capability_report.md` e miniaturas em
//! `MOMENTO_REPORT_DIR` (por omissão, a pasta temporária).

use momento_image_codecs::{
    default_registry, detect, DecodeKind, FileFormat, FormatFamily, ImageCodec, SupportLevel,
};
use momento_raw_engine::{RawCodec, RawFile};
use std::fmt::Write;
use std::path::PathBuf;
use std::time::Instant;

#[test]
fn real_raw_samples() {
    let Ok(dir) = std::env::var("MOMENTO_SAMPLES_DIR") else {
        eprintln!("MOMENTO_SAMPLES_DIR não definido: teste ignorado");
        return;
    };
    let out = PathBuf::from(std::env::var("MOMENTO_REPORT_DIR").unwrap_or_else(|_| {
        std::env::temp_dir().join("momento-report").to_string_lossy().into_owned()
    }));
    std::fs::create_dir_all(&out).unwrap();
    let codec = RawCodec::new(default_registry());
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();

    let mut md = String::from("| Ficheiro | Formato detetado | Câmara | Dimensões | Bits | Orient. | Objetiva | Exposição | Data | Preview embutida | Suporte | Miniatura | ½ revelação | Revelação total |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    let mut raw_count = 0;
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let data = std::fs::read(&path).unwrap();
        let ext = path.extension().and_then(|e| e.to_str());
        let det = detect(&data[..data.len().min(64)], ext);
        let Some(det) = det.filter(|d| d.format.family() == FormatFamily::Raw) else { continue };
        raw_count += 1;
        let fmt = format!("{:?} ({:?})", det.format, det.confidence);
        let opened = RawFile::open(&data);
        let Ok(raw) = opened else {
            let e = opened.err().unwrap();
            writeln!(md, "| {name} | {fmt} | — | — | — | — | — | — | — | — | ❌ {e} | — | — | — |")
                .unwrap();
            continue;
        };
        let i = raw.info().clone();
        drop(raw);
        let meta = codec.metadata(&data, det.format).unwrap();
        let support = match &meta.support {
            Some(SupportLevel::Full) => "✅ completo".to_string(),
            Some(SupportLevel::Partial(n)) => format!("⚠️ parcial — {n}"),
            None => "?".into(),
        };
        let t = Instant::now();
        let thumb = codec.decode(&data, det.format, Some(427));
        let thumb_cell = match &thumb {
            Ok(d) => {
                let small = image::imageops::thumbnail(
                    &d.image,
                    320,
                    320 * d.image.height() / d.image.width().max(1),
                );
                let oriented = match i.orientation {
                    3 => image::imageops::rotate180(&small),
                    6 => image::imageops::rotate90(&small),
                    8 => image::imageops::rotate270(&small),
                    _ => small,
                };
                oriented.save(out.join(format!("{name}.thumb.jpg"))).unwrap();
                format!(
                    "{} {}×{} ({:.0} ms)",
                    kind(d.kind),
                    d.image.width(),
                    d.image.height(),
                    t.elapsed().as_secs_f64() * 1000.0
                )
            }
            Err(e) => format!("❌ {e}"),
        };
        let t = Instant::now();
        let half = codec.decode(&data, FileFormat::TiffBasedRaw, Some(i.width.min(i.height) / 2));
        let half_cell = match &half {
            Ok(d) => format!(
                "{} {}×{} ({:.0} ms)",
                kind(d.kind),
                d.image.width(),
                d.image.height(),
                t.elapsed().as_secs_f64() * 1000.0
            ),
            Err(e) => format!("❌ {e}"),
        };
        let t = Instant::now();
        let full = codec.decode(&data, det.format, None);
        let full_cell = match &full {
            Ok(d) => format!(
                "{} {}×{} ({:.0} ms)",
                kind(d.kind),
                d.image.width(),
                d.image.height(),
                t.elapsed().as_secs_f64() * 1000.0
            ),
            Err(e) => format!("❌ {e}"),
        };
        writeln!(
            md,
            "| {name} | {fmt} | {} {} | {}×{} | {} | {} | {} | {} | {} | {} | {support} | {thumb_cell} | {half_cell} | {full_cell} |",
            i.make,
            i.model,
            i.width,
            i.height,
            i.bits,
            i.orientation,
            i.lens.clone().unwrap_or_else(|| "—".into()),
            format!(
                "ISO {} · {} · f/{} · {} mm",
                i.iso.map(|v| v.to_string()).unwrap_or("—".into()),
                i.shutter.map(|s| if s < 1.0 { format!("1/{:.0}", 1.0 / s) } else { format!("{s:.1} s") }).unwrap_or("—".into()),
                i.aperture.map(|a| format!("{a:.1}")).unwrap_or("—".into()),
                i.focal_length.map(|f| format!("{f:.0}")).unwrap_or("—".into())
            ),
            i.capture_time.clone().unwrap_or_else(|| "—".into()),
            i.preview.map(|(_, f, w, h)| format!("{} {w}×{h}", match f { 1 => "JPEG", 2 | 3 => "bitmap", 7 => "JPEG XL", _ => "?" })).unwrap_or_else(|| "nenhuma".into()),
        )
        .unwrap();
    }
    std::fs::write(out.join("capability_report.md"), &md).unwrap();
    println!("{md}");
    println!("RAW analisados: {raw_count}; relatório em {}", out.display());
    assert!(raw_count > 0);
}

fn kind(k: DecodeKind) -> &'static str {
    match k {
        DecodeKind::EmbeddedPreview => "preview",
        DecodeKind::HalfSize => "½",
        DecodeKind::Full => "total",
    }
}
