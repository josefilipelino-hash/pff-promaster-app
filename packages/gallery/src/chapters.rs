//! Capítulos da galeria a partir dos momentos do projeto.
//!
//! Modo "agrupado": os 25 momentos finos da organização (Fase 4) juntam-se nos capítulos
//! de um casamento — Preparação, Cerimónia, Sessão, Família e amigos, Cocktail, Receção,
//! Festa — e os momentos personalizados ficam com o seu próprio capítulo. Fotografias sem
//! momento vão para o capítulo da fotografia classificada mais próxima no tempo (até
//! 45 min); as restantes ficam em "Mais momentos", no fim.

use crate::settings::ChapterMode;

#[derive(Debug, Clone, PartialEq)]
pub struct MomentInfo {
    pub id: i64,
    /// Tipo predefinido (ex.: "ceremony"); `None` = personalizado.
    pub kind: Option<String>,
    /// Nome mostrado (já traduzido).
    pub name: String,
    pub sort: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhotoInfo {
    pub key: i64,
    pub moment: Option<i64>,
    /// "AAAA-MM-DDTHH:MM:SS" (hora corrigida).
    pub time: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    pub id: String,
    pub title: String,
    /// Fotografias por ordem (hora de captura).
    pub keys: Vec<i64>,
}

const GROUPS: &[(&str, &[&str], &str, &str)] = &[
    (
        "preparacao",
        &["bride_prep", "groom_prep", "details", "dress", "shoes", "rings", "invitations"],
        "Preparação",
        "Getting ready",
    ),
    (
        "cerimonia",
        &["decoration", "arrival", "ceremony", "entrance", "ring_exchange", "kiss", "exit"],
        "Cerimónia",
        "Ceremony",
    ),
    ("sessao", &["couple_session"], "Sessão", "Portraits"),
    ("familia", &["groups"], "Família e amigos", "Family & friends"),
    ("cocktail", &["cocktail"], "Cocktail", "Cocktail hour"),
    ("rececao", &["reception", "speeches", "dinner", "cake"], "Receção", "Reception"),
    ("festa", &["first_dance", "party", "night", "closing"], "Festa", "Party"),
];

fn seconds(t: &str) -> Option<i64> {
    // "AAAA-MM-DDTHH:MM:SS" → segundos (só para comparar fotografias do mesmo dia/evento).
    let b = t.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let n = |r: std::ops::Range<usize>| t.get(r)?.parse::<i64>().ok();
    let days = n(0..4)? * 372 + n(5..7)? * 31 + n(8..10)?;
    Some(days * 86_400 + n(11..13)? * 3600 + n(14..16)? * 60 + n(17..19)?)
}

/// Divide as fotografias em capítulos, por ordem.
pub fn assign(
    photos: &[PhotoInfo],
    moments: &[MomentInfo],
    mode: ChapterMode,
    english: bool,
) -> Vec<Chapter> {
    let mut sorted: Vec<&PhotoInfo> = photos.iter().collect();
    sorted.sort_by(|a, b| a.time.cmp(&b.time).then(a.key.cmp(&b.key)));
    let more = if english { "More moments" } else { "Mais momentos" };
    if mode == ChapterMode::None {
        return vec![Chapter {
            id: "todas".into(),
            title: String::new(),
            keys: sorted.iter().map(|p| p.key).collect(),
        }];
    }
    // Capítulo (id, título, ordem) de cada momento.
    let chapter_of = |m: &MomentInfo| -> (String, String, i64) {
        if mode == ChapterMode::Grouped {
            if let Some(k) = &m.kind {
                for (id, kinds, pt, en) in GROUPS {
                    if kinds.contains(&k.as_str()) {
                        let first = moments
                            .iter()
                            .filter(|o| o.kind.as_deref().is_some_and(|ok| kinds.contains(&ok)))
                            .map(|o| o.sort)
                            .min()
                            .unwrap_or(m.sort);
                        return (
                            id.to_string(),
                            (if english { en } else { pt }).to_string(),
                            first,
                        );
                    }
                }
            }
        }
        (format!("m{}", m.id), m.name.clone(), m.sort)
    };
    let by_id: std::collections::HashMap<i64, (String, String, i64)> =
        moments.iter().map(|m| (m.id, chapter_of(m))).collect();
    // Fotografias com momento; as outras herdam o da mais próxima no tempo (≤ 45 min).
    let classified: Vec<(i64, &(String, String, i64))> = sorted
        .iter()
        .filter_map(|p| {
            let c = by_id.get(&p.moment?)?;
            Some((p.time.as_deref().and_then(seconds)?, c))
        })
        .collect();
    let mut chapters: Vec<(i64, Chapter)> = Vec::new();
    for p in &sorted {
        let c = p.moment.and_then(|m| by_id.get(&m)).or_else(|| {
            let t = seconds(p.time.as_deref()?)?;
            classified
                .iter()
                .map(|(ct, c)| ((ct - t).abs(), *c))
                .filter(|(d, _)| *d <= 45 * 60)
                .min_by_key(|(d, _)| *d)
                .map(|(_, c)| c)
        });
        let (id, title, order) = match c {
            Some((id, title, order)) => (id.clone(), title.clone(), *order),
            None => ("mais".to_string(), more.to_string(), i64::MAX),
        };
        match chapters.iter_mut().find(|(_, ch)| ch.id == id) {
            Some((_, ch)) => ch.keys.push(p.key),
            None => chapters.push((order, Chapter { id, title, keys: vec![p.key] })),
        }
    }
    chapters.sort_by_key(|(o, _)| *o);
    chapters.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: i64, kind: Option<&str>, name: &str, sort: i64) -> MomentInfo {
        MomentInfo { id, kind: kind.map(String::from), name: name.into(), sort }
    }

    fn p(key: i64, moment: Option<i64>, t: &str) -> PhotoInfo {
        PhotoInfo { key, moment, time: Some(format!("2026-06-15T{t}")) }
    }

    #[test]
    fn wedding_chapters_in_order_with_unclassified_by_time() {
        let moments = vec![
            m(1, Some("bride_prep"), "Preparação da noiva", 0),
            m(2, Some("dress"), "Vestido", 3),
            m(3, Some("ceremony"), "Cerimónia", 9),
            m(4, Some("kiss"), "Beijo", 12),
            m(5, Some("couple_session"), "Sessão do casal", 14),
            m(6, Some("cocktail"), "Cocktail", 16),
            m(7, Some("party"), "Festa", 22),
            m(8, None, "Fogo de artifício", 23),
        ];
        let photos = vec![
            p(10, Some(7), "23:10:00"),
            p(11, Some(1), "11:00:00"),
            p(12, Some(2), "11:30:00"),
            p(13, Some(3), "15:00:00"),
            p(14, None, "15:20:00"), // perto da cerimónia
            p(15, Some(4), "15:40:00"),
            p(16, Some(5), "17:00:00"),
            p(17, Some(6), "18:00:00"),
            p(18, Some(8), "23:59:00"),
            p(19, None, "05:00:00"), // sem vizinho próximo
        ];
        let ch = assign(&photos, &moments, ChapterMode::Grouped, false);
        let titles: Vec<&str> = ch.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Preparação",
                "Cerimónia",
                "Sessão",
                "Cocktail",
                "Festa",
                "Fogo de artifício",
                "Mais momentos"
            ]
        );
        assert_eq!(ch[0].keys, [11, 12]);
        assert_eq!(ch[1].keys, [13, 14, 15], "sem momento → capítulo mais próximo no tempo");
        assert_eq!(ch[6].keys, [19]);
        let en = assign(&photos, &moments, ChapterMode::Grouped, true);
        assert_eq!(en[0].title, "Getting ready");
        let det = assign(&photos, &moments, ChapterMode::Detailed, false);
        assert_eq!(det[0].title, "Preparação da noiva");
        assert_eq!(det[1].title, "Vestido");
        let none = assign(&photos, &moments, ChapterMode::None, false);
        assert_eq!(none.len(), 1);
        assert_eq!(none[0].keys.len(), 10);
        assert_eq!(none[0].keys[0], 19, "por ordem de hora");
    }
}
