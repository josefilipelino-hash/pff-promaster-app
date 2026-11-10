//! Montagem do site estático: HTML, CSS, JS, fontes, dados (simples ou cifrados) e ZIP.
//!
//! As imagens (miniatura, ecrã, download) já foram escritas pela fila de exportação em
//! `img/t`, `img/w` e `img/d` com nomes aleatórios; aqui junta-se tudo.

use crate::chapters::Chapter;
use crate::crypto;
use crate::settings::{DownloadSize, Downloads, GallerySettings, Protection};
use momento_core_types::{CoreError, CoreResult};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Marcador de uma pasta criada pelo Momento (permite substituí-la com segurança).
pub const MARKER: &str = ".momento-gallery";

static CSS: &str = include_str!("../assets/gallery.css");
static JS: &str = include_str!("../assets/gallery.js");
static HTML: &str = include_str!("../assets/index.html");
static FONTS: &[(&str, &[u8])] = &[
    (
        "InstrumentSerif-Regular.woff2",
        include_bytes!("../assets/fonts/InstrumentSerif-Regular.woff2"),
    ),
    (
        "InstrumentSerif-Italic.woff2",
        include_bytes!("../assets/fonts/InstrumentSerif-Italic.woff2"),
    ),
    (
        "InstrumentSans-Regular.woff2",
        include_bytes!("../assets/fonts/InstrumentSans-Regular.woff2"),
    ),
    ("InstrumentSerif-OFL.txt", include_bytes!("../assets/fonts/InstrumentSerif-OFL.txt")),
    ("InstrumentSans-OFL.txt", include_bytes!("../assets/fonts/InstrumentSans-OFL.txt")),
];

#[derive(Debug, Clone, PartialEq)]
pub struct SiteImage {
    /// Caminho absoluto (dentro da pasta da galeria).
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SitePhoto {
    pub key: i64,
    pub token: String,
    /// Nome do ficheiro original sem extensão (lista de favoritas, nome do download).
    pub name: String,
    pub thumb: SiteImage,
    pub web: SiteImage,
    pub download: Option<SiteImage>,
}

pub struct SiteInput<'a> {
    pub settings: &'a GallerySettings,
    pub chapters: &'a [Chapter],
    pub photos: &'a HashMap<i64, SitePhoto>,
    pub hero: Option<&'a SiteImage>,
    pub cover: Option<&'a SiteImage>,
    pub logo: Option<&'a Path>,
    /// Identificador estável da galeria: as favoritas do cliente (guardadas no browser por
    /// este id e pelos tokens das fotografias) sobrevivem a uma nova geração.
    pub gallery_id: &'a str,
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildReport {
    pub photos: usize,
    pub chapters: usize,
    pub zips: usize,
    pub files: usize,
    pub bytes: u64,
    pub index: String,
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// "Ana & Pedro" → "ana-e-pedro" (nomes de ZIP).
pub fn slug(s: &str) -> String {
    const FROM: &str = "áàâãäåçéèêëíìîïñóòôõöúùûüýÁÀÂÃÄÅÇÉÈÊËÍÌÎÏÑÓÒÔÕÖÚÙÛÜÝ";
    const TO: &str = "aaaaaaceeeeiiiinooooouuuuyaaaaaaceeeeiiiinooooouuuuy";
    let mut out = String::new();
    for c in s.replace('&', " e ").chars() {
        let c = FROM.chars().position(|f| f == c).map(|i| TO.as_bytes()[i] as char).unwrap_or(c);
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let t = out.trim_matches('-').to_string();
    if t.is_empty() {
        "galeria".into()
    } else {
        t
    }
}

fn rel(root: &Path, p: &Path) -> CoreResult<String> {
    let r = p
        .strip_prefix(root)
        .map_err(|_| CoreError::Other(format!("ficheiro fora da galeria: {}", p.display())))?;
    Ok(r.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"))
}

/// Cor média de uma miniatura (fundo enquanto carrega).
fn mean_color(p: &Path) -> String {
    let Ok(img) = image::open(p) else { return "#888888".into() };
    let small = img.thumbnail(16, 16).to_rgb8();
    let n = (small.width() * small.height()).max(1) as u64;
    let s = small
        .pixels()
        .fold([0u64; 3], |a, q| [a[0] + q[0] as u64, a[1] + q[1] as u64, a[2] + q[2] as u64]);
    format!("#{:02x}{:02x}{:02x}", s[0] / n, s[1] / n, s[2] / n)
}

/// JSON seguro dentro de `<script>` (sem "</", U+2028, U+2029).
fn js_json(v: &Value) -> String {
    serde_json::to_string(v)
        .unwrap_or_default()
        .replace("</", "<\\/")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

fn zip_files(path: &Path, entries: &[(String, PathBuf)]) -> CoreResult<u64> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("zip.tmp");
    {
        let f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        let mut z = zip::ZipWriter::new(f);
        // JPEG já está comprimido: guardar sem compressão (rápido, mesmo tamanho).
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true);
        for (name, src) in entries {
            z.start_file(name.as_str(), opts).map_err(|e| CoreError::Other(format!("zip: {e}")))?;
            let mut f = std::fs::File::open(src)?;
            std::io::copy(&mut f, &mut z)?;
        }
        z.finish().map_err(|e| CoreError::Other(format!("zip: {e}")))?.flush()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(std::fs::metadata(path)?.len())
}

fn texts(english: bool) -> (&'static str, &'static str) {
    if english {
        ("Private gallery", "This gallery needs JavaScript.")
    } else {
        ("Galeria privada", "Esta galeria precisa de JavaScript.")
    }
}

/// Escreve o site na pasta `root` (que já contém `img/`).
pub fn build(root: &Path, input: &SiteInput) -> CoreResult<BuildReport> {
    let s = input.settings;
    let protected = s.protection != Protection::None;
    let gallery_id = input.gallery_id;
    std::fs::create_dir_all(root.join("assets/fonts"))?;
    std::fs::create_dir_all(root.join("data"))?;
    std::fs::write(root.join(MARKER), "PFF ProMaster\n")?;
    std::fs::write(root.join("assets/gallery.css"), CSS)?;
    std::fs::write(root.join("assets/gallery.js"), JS)?;
    for (name, data) in FONTS {
        std::fs::write(root.join("assets/fonts").join(name), data)?;
    }
    // Logótipo com nome aleatório quando a galeria é protegida.
    let logo = match input.logo {
        Some(p) => {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("png").to_lowercase();
            if !["png", "jpg", "jpeg", "svg", "webp"].contains(&ext.as_str()) {
                return Err(CoreError::UnsupportedFormat(format!("logótipo .{ext}")));
            }
            let name = if protected {
                format!("{}.{ext}", crypto::token()?)
            } else {
                format!("logo.{ext}")
            };
            std::fs::copy(p, root.join("assets").join(&name))?;
            Some(format!("assets/{name}"))
        }
        None => None,
    };
    // Fotografias (índices por ordem dos capítulos).
    let mut photos = Vec::new();
    let mut chapters = Vec::new();
    let mut downloads: Vec<(usize, String, PathBuf)> = Vec::new(); // (capítulo, nome no zip, ficheiro)
    let mut used_names: HashMap<String, usize> = HashMap::new();
    let downloads_on = s.downloads != Downloads::None;
    for (ci, ch) in input.chapters.iter().enumerate() {
        let mut idx = Vec::new();
        for key in &ch.keys {
            let Some(p) = input.photos.get(key) else { continue };
            let dl = if !downloads_on {
                None
            } else if s.download_size == DownloadSize::High {
                p.download.as_ref().or(Some(&p.web))
            } else {
                Some(&p.web)
            };
            // Nome do ficheiro descarregado: original, sem repetições.
            let n = used_names.entry(p.name.to_lowercase()).or_insert(0);
            *n += 1;
            let dl_name =
                if *n == 1 { format!("{}.jpg", p.name) } else { format!("{}-{}.jpg", p.name, n) };
            let mut v = json!({
                "id": p.token,
                "n": p.name,
                "t": rel(root, &p.thumb.path)?,
                "tw": p.thumb.width,
                "th": p.thumb.height,
                "s": rel(root, &p.web.path)?,
                "w": p.web.width,
                "h": p.web.height,
                "c": mean_color(&p.thumb.path),
            });
            if let Some(d) = dl {
                v["d"] = json!(rel(root, &d.path)?);
                v["dn"] = json!(dl_name);
                v["ds"] = json!(std::fs::metadata(&d.path).map(|m| m.len()).unwrap_or(0));
                downloads.push((ci, dl_name.clone(), d.path.clone()));
            }
            idx.push(photos.len());
            photos.push(v);
        }
        if !idx.is_empty() {
            chapters.push(json!({ "id": ch.id, "title": ch.title, "photos": idx }));
        }
    }
    if photos.is_empty() {
        return Err(CoreError::Other("a galeria não tem fotografias".into()));
    }
    // ZIP: completo + por capítulo (opcional), numa pasta aleatória.
    let mut zips = Vec::new();
    if s.downloads == Downloads::All {
        let dir = root.join("download").join(crypto::token()?);
        let base = slug(&s.couple);
        let folder = |ci: usize| {
            let t = &input.chapters[ci].title;
            if t.is_empty() {
                String::new()
            } else {
                format!("{:02} {}/", ci + 1, t.replace(['/', '\\'], "-"))
            }
        };
        let all: Vec<(String, PathBuf)> = downloads
            .iter()
            .map(|(ci, n, p)| (format!("{base}/{}{n}", folder(*ci)), p.clone()))
            .collect();
        let path = dir.join(format!("{base}.zip"));
        let bytes = zip_files(&path, &all)?;
        let all_label = if s.english() { "Complete gallery" } else { "Galeria completa" };
        zips.push(json!({ "label": all_label, "href": rel(root, &path)?, "bytes": bytes, "count": all.len() }));
        if s.chapter_zips && input.chapters.len() > 1 {
            for (ci, ch) in input.chapters.iter().enumerate() {
                let e: Vec<(String, PathBuf)> = downloads
                    .iter()
                    .filter(|d| d.0 == ci)
                    .map(|(_, n, p)| (format!("{base}/{}{n}", folder(ci)), p.clone()))
                    .collect();
                if e.is_empty() {
                    continue;
                }
                let path = dir.join(format!("{base}-{:02}-{}.zip", ci + 1, slug(&ch.title)));
                let bytes = zip_files(&path, &e)?;
                zips.push(json!({ "label": ch.title, "href": rel(root, &path)?, "bytes": bytes, "count": e.len() }));
            }
        }
    }
    let img = |i: Option<&SiteImage>| -> CoreResult<Value> {
        Ok(match i {
            Some(i) => json!({ "src": rel(root, &i.path)?, "w": i.width, "h": i.height }),
            None => Value::Null,
        })
    };
    let manifest = json!({
        "v": 1,
        "id": gallery_id,
        "lang": if s.english() { "en" } else { "pt" },
        "theme": s.theme,
        "layout": s.layout,
        "couple": s.couple,
        "date": s.date,
        "location": s.location,
        "intro": s.intro,
        "photographer": s.photographer,
        "contact": s.contact_email,
        "hero": img(input.hero)?,
        "cover": img(input.cover)?,
        "logo": logo,
        "features": {
            "favorites": s.favorites,
            "downloads": s.downloads,
            "slideshow": s.slideshow_seconds.clamp(2, 20),
        },
        "chapters": chapters,
        "photos": photos,
        "zips": zips,
    });
    // Dados: script (funciona aberto do disco, sem pedidos de rede).
    let data = if protected {
        let sealed = crypto::seal(
            serde_json::to_string(&manifest).unwrap_or_default().as_bytes(),
            s.secret.trim(),
        )?;
        let gate = json!({
            "lang": if s.english() { "en" } else { "pt" },
            "theme": s.theme,
            "pin": s.protection == Protection::Pin,
            "sealed": sealed,
        });
        format!("window.MOMENTO_GALLERY_ENC = {};\n", js_json(&gate))
    } else {
        format!("window.MOMENTO_GALLERY = {};\n", js_json(&manifest))
    };
    std::fs::write(root.join("data/gallery.js"), data)?;
    // HTML: com o título e a pré-visualização só quando não é protegida.
    let (private_title, noscript) = texts(s.english());
    let title = if protected { private_title.to_string() } else { s.couple.clone() };
    let mut meta = String::new();
    if !protected {
        let desc = [s.date.as_str(), s.location.as_str()]
            .iter()
            .filter(|x| !x.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" · ");
        meta += &format!("<meta property=\"og:title\" content=\"{}\">\n", html_escape(&s.couple));
        if !desc.is_empty() {
            meta +=
                &format!("<meta property=\"og:description\" content=\"{}\">\n", html_escape(&desc));
        }
        if let Some(c) = input.cover {
            meta += &format!(
                "<meta property=\"og:image\" content=\"{}\">\n",
                html_escape(&rel(root, &c.path)?)
            );
        }
    } else {
        meta += "<meta name=\"robots\" content=\"noindex, nofollow\">\n";
    }
    let html = HTML
        .replace("{{LANG}}", if s.english() { "en" } else { "pt-PT" })
        .replace("{{TITLE}}", &html_escape(&title))
        .replace("{{META}}", &meta)
        .replace(
            "{{THEME}}",
            match s.theme {
                crate::settings::Theme::Light => "light",
                crate::settings::Theme::Dark => "dark",
                crate::settings::Theme::Minimal => "minimal",
                crate::settings::Theme::Editorial => "editorial",
            },
        )
        .replace("{{NOSCRIPT}}", noscript);
    let index = root.join("index.html");
    std::fs::write(&index, html)?;
    // Relatório.
    let (mut files, mut bytes) = (0usize, 0u64);
    for e in walkdir::WalkDir::new(root).into_iter().flatten().filter(|e| e.file_type().is_file()) {
        files += 1;
        bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
    }
    Ok(BuildReport {
        photos: photos.len(),
        chapters: chapters.len(),
        zips: zips.len(),
        files,
        bytes,
        index: index.to_string_lossy().into_owned(),
    })
}

/// Lê os dados gerados (para verificação): devolve o manifesto, decifrando se preciso.
pub fn read_manifest(root: &Path, secret: Option<&str>) -> CoreResult<Value> {
    let js = std::fs::read_to_string(root.join("data/gallery.js"))?;
    let body = js.split_once(" = ").map(|x| x.1).unwrap_or("").trim().trim_end_matches(';');
    let v: Value = serde_json::from_str(body).map_err(|e| CoreError::Corrupt(e.to_string()))?;
    if js.starts_with("window.MOMENTO_GALLERY_ENC") {
        let sealed: crypto::Sealed = serde_json::from_value(v["sealed"].clone())
            .map_err(|e| CoreError::Corrupt(e.to_string()))?;
        let plain = crypto::open(&sealed, secret.unwrap_or(""))?;
        serde_json::from_slice(&plain).map_err(|e| CoreError::Corrupt(e.to_string()))
    } else {
        Ok(v)
    }
}
