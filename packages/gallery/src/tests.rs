//! Montagem completa de uma galeria com imagens reais (JPEG) numa pasta temporária.

use crate::chapters::Chapter;
use crate::site::{build, read_manifest, SiteImage, SiteInput, SitePhoto, MARKER};
use crate::*;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

fn jpeg(path: &Path, w: u32, h: u32, rgb: [u8; 3]) -> SiteImage {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbImage::from_pixel(w, h, image::Rgb(rgb)).save(path).unwrap();
    SiteImage { path: path.to_path_buf(), width: w, height: h }
}

fn fixture(root: &Path) -> (Vec<Chapter>, HashMap<i64, SitePhoto>) {
    let mut photos = HashMap::new();
    for k in 1..=5i64 {
        let tok = format!("{:032x}", k * 7919);
        let (w, h) = if k % 2 == 0 { (60, 90) } else { (90, 60) };
        photos.insert(
            k,
            SitePhoto {
                key: k,
                token: tok.clone(),
                name: if k == 5 { "IMG_0001".into() } else { format!("IMG_000{k}") },
                thumb: jpeg(
                    &root.join(format!("img/t/{tok}.jpg")),
                    w / 3,
                    h / 3,
                    [200, 40 * k as u8, 30],
                ),
                web: jpeg(&root.join(format!("img/w/{tok}.jpg")), w, h, [200, 40 * k as u8, 30]),
                download: Some(jpeg(
                    &root.join(format!("img/d/{tok}.jpg")),
                    w * 2,
                    h * 2,
                    [10, 20, 30],
                )),
            },
        );
    }
    let chapters = vec![
        Chapter { id: "preparacao".into(), title: "Preparação".into(), keys: vec![1, 2] },
        Chapter { id: "cerimonia".into(), title: "Cerimónia".into(), keys: vec![3, 4, 5] },
    ];
    (chapters, photos)
}

fn settings() -> GallerySettings {
    GallerySettings {
        couple: "Ana & Pedro <script>alert(1)</script>".into(),
        date: "2026-06-15".into(),
        location: "Sintra".into(),
        intro: "Um dia bonito.\n\nObrigado.".into(),
        photographer: "Estúdio Lino".into(),
        contact_email: "ola@example.com".into(),
        chapter_zips: true,
        ..Default::default()
    }
}

#[test]
fn builds_a_complete_static_site() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let (chapters, photos) = fixture(root);
    let s = settings();
    let hero = photos[&3].web.clone();
    let logo = root.join("logo-src.png");
    image::RgbaImage::from_pixel(20, 10, image::Rgba([255, 255, 255, 200])).save(&logo).unwrap();
    let r = build(
        root,
        &SiteInput {
            settings: &s,
            chapters: &chapters,
            photos: &photos,
            hero: Some(&hero),
            cover: Some(&hero),
            logo: Some(&logo),
            gallery_id: "0123456789abcdef0123456789abcdef",
        },
    )
    .unwrap();
    assert_eq!((r.photos, r.chapters, r.zips), (5, 2, 3));
    for f in [
        "index.html",
        "assets/gallery.css",
        "assets/gallery.js",
        "data/gallery.js",
        MARKER,
        "assets/fonts/InstrumentSerif-Regular.woff2",
        "assets/fonts/InstrumentSerif-OFL.txt",
        "assets/logo.png",
    ] {
        assert!(root.join(f).is_file(), "{f}");
    }
    let html = std::fs::read_to_string(root.join("index.html")).unwrap();
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"), "título escapado");
    assert!(!html.contains("<script>alert"), "sem injeção");
    assert!(html.contains("og:image") && html.contains("data-theme=\"light\""));
    // Manifesto: capítulos por ordem, nomes e downloads com nomes únicos.
    let m = read_manifest(root, None).unwrap();
    assert_eq!(m["chapters"][1]["title"], "Cerimónia");
    assert_eq!(m["chapters"][1]["photos"].as_array().unwrap().len(), 3);
    let p = &m["photos"][0];
    assert_eq!(p["t"].as_str().unwrap(), format!("img/t/{:032x}.jpg", 7919));
    assert!(p["c"].as_str().unwrap().starts_with('#'));
    let dn: Vec<&str> =
        m["photos"].as_array().unwrap().iter().map(|p| p["dn"].as_str().unwrap()).collect();
    assert!(dn.contains(&"IMG_0001.jpg") && dn.contains(&"IMG_0001-2.jpg"), "{dn:?}");
    // O "</script>" do nome não fecha o script dos dados.
    let data = std::fs::read_to_string(root.join("data/gallery.js")).unwrap();
    assert!(!data.contains("</script>"));
    // ZIP completo: 5 ficheiros em pastas por capítulo, alta resolução.
    let zip_path = root.join(m["zips"][0]["href"].as_str().unwrap());
    assert!(zip_path.to_string_lossy().contains("download/"));
    let mut z = zip::ZipArchive::new(std::fs::File::open(&zip_path).unwrap()).unwrap();
    assert_eq!(z.len(), 5);
    let mut names: Vec<String> =
        (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect();
    names.sort();
    assert_eq!(names[0], "ana-e-pedro-script-alert-1-script/01 Preparação/IMG_0001.jpg");
    let mut buf = Vec::new();
    z.by_index(0).unwrap().read_to_end(&mut buf).unwrap();
    assert_eq!(
        image::load_from_memory(&buf).unwrap().width() % 60,
        0,
        "download em alta resolução"
    );
    assert_eq!(m["zips"].as_array().unwrap().len(), 3, "completo + 2 capítulos");
}

#[test]
fn protected_gallery_reveals_nothing_without_the_code() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let (chapters, photos) = fixture(root);
    let s = GallerySettings { protection: Protection::Pin, secret: "2468".into(), ..settings() };
    assert!(s.problems().is_empty());
    let logo = root.join("logo-src.png");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 255])).save(&logo).unwrap();
    build(
        root,
        &SiteInput {
            settings: &s,
            chapters: &chapters,
            photos: &photos,
            hero: None,
            cover: None,
            logo: Some(&logo),
            gallery_id: "0123456789abcdef0123456789abcdef",
        },
    )
    .unwrap();
    let html = std::fs::read_to_string(root.join("index.html")).unwrap();
    let data = std::fs::read_to_string(root.join("data/gallery.js")).unwrap();
    // O conteúdo cifrado (base64) pode conter "Ana" por acaso: procura-se fora dele.
    let plain = strip_base64_blobs(&data);
    for leak in ["Ana", "Sintra", "img/", "IMG_", "Cerimónia", "og:image"] {
        assert!(!html.contains(leak) && !plain.contains(leak), "{leak} visível sem o código");
    }
    assert!(html.contains("noindex"));
    assert!(read_manifest(root, Some("1357")).is_err(), "código errado");
    let m = read_manifest(root, Some("2468")).unwrap();
    assert_eq!(m["photos"].as_array().unwrap().len(), 5);
    assert!(!root.join("assets/logo.png").exists(), "logótipo com nome aleatório");
    // Problemas de configuração.
    let bad = GallerySettings { protection: Protection::Pin, secret: "12a".into(), ..settings() };
    assert_eq!(bad.problems(), ["gallery.problem.pin"]);
    let bad = GallerySettings {
        protection: Protection::Password,
        secret: "abc".into(),
        couple: String::new(),
        ..settings()
    };
    assert_eq!(bad.problems(), ["gallery.problem.couple", "gallery.problem.password"]);
}

#[test]
fn downloads_off_and_web_size() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let (chapters, photos) = fixture(root);
    let s = GallerySettings { downloads: Downloads::None, ..settings() };
    let r = build(
        root,
        &SiteInput {
            settings: &s,
            chapters: &chapters,
            photos: &photos,
            hero: None,
            cover: None,
            logo: None,
            gallery_id: "0123456789abcdef0123456789abcdef",
        },
    )
    .unwrap();
    assert_eq!(r.zips, 0);
    let m = read_manifest(root, None).unwrap();
    assert!(m["photos"][0].get("d").is_none(), "sem downloads");
    let d2 = tempfile::tempdir().unwrap();
    let (chapters, photos) = fixture(d2.path());
    let s = GallerySettings {
        downloads: Downloads::Individual,
        download_size: DownloadSize::Web,
        ..settings()
    };
    let r = build(
        d2.path(),
        &SiteInput {
            settings: &s,
            chapters: &chapters,
            photos: &photos,
            hero: None,
            cover: None,
            logo: None,
            gallery_id: "0123456789abcdef0123456789abcdef",
        },
    )
    .unwrap();
    assert_eq!(r.zips, 0);
    let m = read_manifest(d2.path(), None).unwrap();
    assert!(
        m["photos"][0]["d"].as_str().unwrap().starts_with("img/w/"),
        "download = imagem de ecrã"
    );
}

/// Substitui textos entre aspas que são blocos base64 (≥ 24 caracteres) por `"…"`.
fn strip_base64_blobs(s: &str) -> String {
    let mut out = String::new();
    for (i, part) in s.split('"').enumerate() {
        let blob = i % 2 == 1
            && part.len() >= 24
            && part.bytes().all(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'-' | b'_')
            });
        if i > 0 {
            out.push('"');
        }
        out.push_str(if blob { "…" } else { part });
    }
    out
}
