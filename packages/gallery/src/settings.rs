//! Definições da galeria de um projeto (guardadas no catálogo).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Light,
    Dark,
    Minimal,
    Editorial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Layout {
    #[default]
    Masonry,
    Grid,
}

/// Que fotografias entram na galeria (antes de retirar as ocultas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Selection {
    /// Favoritas (bandeira); se não houver nenhuma, todas as não rejeitadas.
    #[default]
    Picks,
    /// Todas exceto as rejeitadas.
    NotRejected,
    /// Com pelo menos `min_stars` estrelas.
    Stars,
    All,
}

/// Como agrupar por momentos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChapterMode {
    /// Capítulos de casamento: Preparação, Cerimónia, Sessão, Família, Cocktail, Receção, Festa.
    #[default]
    Grouped,
    /// Um capítulo por momento do projeto.
    Detailed,
    /// Uma só sequência, sem títulos.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Downloads {
    None,
    /// Cada fotografia (no visualizador).
    Individual,
    /// Individual + ficheiros ZIP (completo e, opcionalmente, por capítulo).
    #[default]
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadSize {
    /// O mesmo ficheiro que se vê (lado longo 2048 px).
    Web,
    /// Resolução total (JPEG q92).
    #[default]
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Protection {
    #[default]
    None,
    /// Código numérico (4–8 dígitos): barreira ligeira.
    Pin,
    Password,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GallerySettings {
    /// Nomes dos noivos (título da galeria).
    pub couple: String,
    /// Data do evento, "AAAA-MM-DD".
    pub date: String,
    pub location: String,
    pub intro: String,
    /// Chave da fotografia de capa (pré-visualização e entrada protegida).
    pub cover: Option<i64>,
    /// Chave da imagem hero (topo da galeria).
    pub hero: Option<i64>,
    /// Logótipo (PNG/SVG/JPEG) copiado para a galeria.
    pub logo_path: String,
    pub theme: Theme,
    pub layout: Layout,
    /// "pt" ou "en" (textos da galeria).
    pub language: String,
    pub selection: Selection,
    pub min_stars: u8,
    pub chapters: ChapterMode,
    /// Fotografias ocultas (chaves): nunca são escritas na galeria.
    pub hidden: Vec<i64>,
    pub favorites: bool,
    pub downloads: Downloads,
    pub download_size: DownloadSize,
    /// ZIP por capítulo, além do completo.
    pub chapter_zips: bool,
    pub protection: Protection,
    /// PIN ou palavra-passe (só no catálogo local; nunca nos registos).
    pub secret: String,
    /// Fotógrafo (rodapé) e email para receber a lista de favoritas.
    pub photographer: String,
    pub contact_email: String,
    pub slideshow_seconds: u32,
}

impl Default for GallerySettings {
    fn default() -> Self {
        GallerySettings {
            couple: String::new(),
            date: String::new(),
            location: String::new(),
            intro: String::new(),
            cover: None,
            hero: None,
            logo_path: String::new(),
            theme: Theme::Light,
            layout: Layout::Masonry,
            language: "pt".into(),
            selection: Selection::Picks,
            min_stars: 3,
            chapters: ChapterMode::Grouped,
            hidden: Vec::new(),
            favorites: true,
            downloads: Downloads::All,
            download_size: DownloadSize::High,
            chapter_zips: false,
            protection: Protection::None,
            secret: String::new(),
            photographer: String::new(),
            contact_email: String::new(),
            slideshow_seconds: 4,
        }
    }
}

impl GallerySettings {
    /// Problemas que impedem gerar (chaves i18n).
    pub fn problems(&self) -> Vec<&'static str> {
        let mut p = Vec::new();
        if self.couple.trim().is_empty() {
            p.push("gallery.problem.couple");
        }
        match self.protection {
            Protection::Pin => {
                let s = self.secret.trim();
                if !(4..=8).contains(&s.len()) || !s.chars().all(|c| c.is_ascii_digit()) {
                    p.push("gallery.problem.pin");
                }
            }
            Protection::Password => {
                if self.secret.chars().count() < 6 {
                    p.push("gallery.problem.password");
                }
            }
            Protection::None => {}
        }
        p
    }

    pub fn english(&self) -> bool {
        self.language == "en"
    }
}
