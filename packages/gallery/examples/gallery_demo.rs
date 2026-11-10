//! Galeria de demonstração a partir de uma pasta de JPEG (sem a aplicação): para ver os
//! temas e testar a interface num navegador.
//! `cargo run --release -p momento-gallery --example gallery_demo -- fotos/ saida/ [light|dark|minimal|editorial] [pin]`
use momento_gallery::chapters::Chapter;
use momento_gallery::site::{build, SiteImage, SiteInput, SitePhoto};
use momento_gallery::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn save(img: &image::DynamicImage, edge: u32, path: &Path) -> SiteImage {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let r = img.resize(edge, edge, image::imageops::FilterType::Lanczos3).to_rgb8();
    r.save(path).unwrap();
    SiteImage { path: path.to_path_buf(), width: r.width(), height: r.height() }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(&a[2]);
    let _ = std::fs::remove_dir_all(&out);
    let theme = match a.get(3).map(String::as_str) {
        Some("dark") => Theme::Dark,
        Some("minimal") => Theme::Minimal,
        Some("editorial") => Theme::Editorial,
        _ => Theme::Light,
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&a[1])
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jpg")))
        .collect();
    files.sort();
    let mut photos = HashMap::new();
    for (k, f) in files.iter().enumerate() {
        let img = image::open(f).unwrap();
        let tok = crypto::token().unwrap();
        photos.insert(
            k as i64,
            SitePhoto {
                key: k as i64,
                token: tok.clone(),
                name: f.file_stem().unwrap().to_string_lossy().into_owned(),
                thumb: save(&img, 900, &out.join(format!("img/t/{tok}.jpg"))),
                web: save(&img, 2048, &out.join(format!("img/w/{tok}.jpg"))),
                download: Some(save(&img, 4000, &out.join(format!("img/d/{tok}.jpg")))),
            },
        );
    }
    let n = files.len() as i64;
    let names = ["Preparação", "Cerimónia", "Sessão", "Receção", "Festa"];
    let chapters: Vec<Chapter> = names
        .iter()
        .enumerate()
        .map(|(i, t)| Chapter {
            id: format!("c{i}"),
            title: t.to_string(),
            keys: (0..n).filter(|k| (*k as usize) * names.len() / n as usize == i).collect(),
        })
        .collect();
    let pin = a.get(4).is_some();
    let s = GallerySettings {
        couple: "Ana & Pedro".into(),
        date: "2026-06-15".into(),
        location: "Quinta da Regaleira, Sintra".into(),
        intro: "Obrigado por nos deixarem fazer parte de um dia tão especial.\n\nAqui estão as memórias do vosso casamento — escolham as favoritas com o coração.".into(),
        photographer: "Estúdio Lino".into(),
        contact_email: "ola@example.com".into(),
        theme,
        chapter_zips: true,
        protection: if pin { Protection::Pin } else { Protection::None },
        secret: if pin { "2468".into() } else { String::new() },
        ..Default::default()
    };
    let hero = photos[&(n / 2)].web.clone();
    let r = build(
        &out,
        &SiteInput {
            settings: &s,
            chapters: &chapters,
            photos: &photos,
            hero: Some(&hero),
            cover: Some(&hero),
            logo: None,
            gallery_id: "0123456789abcdef0123456789abcdef",
        },
    )
    .unwrap();
    println!(
        "{} fotografias, {} capítulos, {} ZIP, {} ficheiros, {:.1} MB → {}",
        r.photos,
        r.chapters,
        r.zips,
        r.files,
        r.bytes as f64 / 1e6,
        r.index
    );
}
