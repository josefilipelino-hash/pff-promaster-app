//! Perfis de objetiva da base de dados Lensfun (CC BY-SA 3.0, ver `data/LENSFUN.md`).
//!
//! Identifica câmara e objetiva pelo EXIF e calcula os coeficientes desta fotografia
//! (interpolados para a focal e a abertura) no formato de `develop::LensProfile`.
//!
//! Escolha da objetiva: o intervalo focal e a abertura do nome têm de coincidir com os do
//! EXIF; entre as candidatas (encaixe compatível com a câmara), ganha a que tem mais palavras
//! do nome em comum. Câmaras compactas (objetiva fixa) usam a objetiva do seu encaixe próprio.

use momento_image_processing::develop::LensProfile;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const DIAG_35MM: f32 = 43.267;

#[derive(Deserialize)]
struct Db {
    source: String,
    mounts: HashMap<String, Vec<String>>,
    /// [fabricante, modelo, variante, encaixe, fator de corte]
    cameras: Vec<(String, String, String, String, f32)>,
    lenses: Vec<DbLens>,
}

#[derive(Deserialize)]
struct DbLens {
    #[serde(rename = "k")]
    maker: String,
    #[serde(rename = "m")]
    model: String,
    #[serde(rename = "mt")]
    mounts: Vec<String>,
    #[serde(rename = "c")]
    crop: f32,
    #[serde(rename = "a")]
    aspect: f32,
    #[serde(rename = "t")]
    kind: String,
    /// [focal, modelo, a, b, c]
    #[serde(rename = "d")]
    dist: Vec<[f32; 5]>,
    /// [focal, vr, vb, cr, cb, br, bb]
    #[serde(rename = "tc")]
    tca: Vec<[f32; 7]>,
    /// [focal, abertura, distância, k1, k2, k3]
    #[serde(rename = "v")]
    vig: Vec<[f32; 6]>,
}

fn db() -> &'static Db {
    static DB: OnceLock<Db> = OnceLock::new();
    DB.get_or_init(|| {
        serde_json::from_str(include_str!("../data/lensfun.json")).expect("lensfun.json válido")
    })
}

/// Origem da base de dados (para mostrar a atribuição).
pub fn database_source() -> &'static str {
    &db().source
}

/// Dados do EXIF usados para escolher o perfil.
#[derive(Debug, Clone, Default)]
pub struct ProfileQuery {
    pub camera_make: String,
    pub camera_model: String,
    pub lens: String,
    pub focal: Option<f32>,
    pub focal35: Option<f32>,
    pub aperture: Option<f32>,
}

/// Normaliza um nome: minúsculas, "f/2.8" → "f2.8", vírgula decimal, só letras/números/.-
fn norm(s: &str) -> String {
    let s = s.to_lowercase().replace("f/", "f").replace(',', ".");
    let s: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' { c } else { ' ' })
        .collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tokens(s: &str) -> Vec<String> {
    norm(s).split(' ').filter(|t| !t.is_empty()).map(str::to_string).collect()
}

/// Intervalo focal "24-70mm" / "50mm" / "XF56mm" / "1:3.5-5.6 / 18-56"; `None` se não houver.
fn focal_range(s: &str) -> Option<(f32, f32)> {
    let n = norm(&s.replace("1:", "f"));
    let parse = |spec: &str| -> Option<(f32, f32)> {
        let mut it = spec
            .trim_matches(|c| c == '.' || c == '-')
            .split('-')
            .filter_map(|v| v.parse::<f32>().ok());
        let a = it.next()?;
        Some((a, it.next().unwrap_or(a)))
    };
    if let Some(i) = n.find("mm") {
        let head = &n[..i];
        let start = head
            .rfind(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
            .map_or(0, |p| p + 1);
        if let Some(r) = parse(&head[start..]) {
            return Some(r);
        }
    }
    // Sem "mm": o primeiro número (ou intervalo) que não é uma abertura.
    n.split(' ').filter(|t| !t.starts_with('f')).find_map(|t| {
        t.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '.').then(|| parse(t)).flatten()
    })
}

/// Abertura máxima do nome ("f2.8", "f3.5-5.6", "F1.2", "1:3.5" → 3.5).
fn max_aperture(s: &str) -> Option<f32> {
    let n = norm(&s.replace("1:", "f"));
    let bytes = n.as_bytes();
    for (i, w) in bytes.windows(2).enumerate() {
        if w[0] == b'f'
            && w[1].is_ascii_digit()
            && (i == 0 || !bytes[i - 1].is_ascii_alphabetic() || bytes[i - 1] == b'm')
        {
            let rest = &n[i + 1..];
            let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
            if let Ok(v) = rest[..end].trim_end_matches('.').parse::<f32>() {
                return Some(v);
            }
        }
    }
    None
}

/// Versão da objetiva (numeração romana: "GM II", "OSS I/II"); sem número = I.
fn versions(tok: &[String]) -> Vec<&str> {
    let v: Vec<&str> = tok
        .iter()
        .map(String::as_str)
        .filter(|t| ["i", "ii", "iii", "iv", "v", "vi"].contains(t))
        .collect();
    if v.is_empty() {
        vec!["i"]
    } else {
        v
    }
}

fn same_maker(a: &str, b: &str) -> bool {
    let (a, b) = (norm(a), norm(b));
    let first = |s: &str| s.split(' ').next().unwrap_or("").to_string();
    !a.is_empty() && first(&a) == first(&b)
}

/// Câmara da base de dados (fabricante + modelo do EXIF).
fn find_camera(make: &str, model: &str) -> Option<&'static (String, String, String, String, f32)> {
    let (mk, md) = (norm(make), norm(model));
    db().cameras
        .iter()
        .find(|c| same_maker(&c.0, make) && norm(&c.1) == md)
        .or_else(|| db().cameras.iter().find(|c| norm(&c.1) == md && !md.is_empty()))
        .or_else(|| {
            // "Canon EOS 40D" na base vs "EOS 40D" na LibRaw (ou o inverso): sem o fabricante.
            db().cameras.iter().find(|c| {
                let n = norm(&c.1);
                same_maker(&c.0, make)
                    && n.len() >= 4
                    && md.len() >= 4
                    && (md.ends_with(&n) || n.ends_with(&md))
            })
        })
        .filter(|_| !mk.is_empty() || !md.is_empty())
}

/// Objetiva escolhida e o seu fator de corte da câmara.
struct Match {
    lens: &'static DbLens,
    camera_crop: f32,
}

fn find_lens(q: &ProfileQuery) -> Option<Match> {
    let camera = find_camera(&q.camera_make, &q.camera_model);
    // Fator de corte: da câmara, ou do EXIF (focal 35 mm / focal); por último o da calibração.
    let camera_crop = camera
        .map(|c| c.4)
        .or_else(|| Some(q.focal35? / q.focal?).filter(|c| c.is_finite() && *c > 0.1));
    let allowed: Option<Vec<&str>> = camera.map(|c| {
        let mut v = vec![c.3.as_str()];
        if let Some(extra) = db().mounts.get(&c.3) {
            v.extend(extra.iter().map(String::as_str));
        }
        v
    });
    let rectilinear = |l: &&DbLens| l.kind == "rectilinear";
    let mount_ok = |l: &&DbLens| {
        allowed.as_ref().is_none_or(|a| l.mounts.iter().any(|m| a.contains(&m.as_str())))
    };
    // Objetiva fixa: o encaixe da câmara é só dela (ex.: "sonyRX100").
    if let Some(c) = camera {
        if !db().cameras.iter().any(|o| o.3 == c.3 && o.1 != c.1) {
            if let Some(l) =
                db().lenses.iter().filter(rectilinear).find(|l| l.mounts.iter().any(|m| m == &c.3))
            {
                return Some(Match { lens: l, camera_crop: c.4 });
            }
        }
    }
    if q.lens.trim().is_empty() {
        return None;
    }
    let e_focal = focal_range(&q.lens);
    let e_ap = max_aperture(&q.lens);
    let e_tok = tokens(&q.lens);
    let mut best: Option<(f32, &DbLens)> = None;
    for l in db().lenses.iter().filter(rectilinear).filter(mount_ok) {
        let l_focal = focal_range(&l.model);
        match (e_focal, l_focal) {
            (Some(a), Some(b)) if (a.0 - b.0).abs() > 0.6 || (a.1 - b.1).abs() > 0.6 => continue,
            (None, _) | (_, None) => continue,
            _ => {}
        }
        if let (Some(a), Some(b)) = (e_ap, max_aperture(&l.model)) {
            if (a - b).abs() > 0.06 {
                continue;
            }
        }
        let l_tok = tokens(&l.model);
        // "GM II" é outra objetiva que "GM": a versão tem de existir na calibração.
        let lv = versions(&l_tok);
        if !versions(&e_tok).iter().all(|v| lv.contains(v)) {
            continue;
        }
        let common = l_tok.iter().filter(|t| e_tok.contains(t)).count() as f32;
        let mut score = common / l_tok.len().max(1) as f32;
        if same_maker(&l.maker, &q.lens)
            || e_tok.iter().any(|t| norm(&l.maker).split(' ').any(|m| m == t))
        {
            score += 0.3;
        } else if same_maker(&l.maker, &q.camera_make) {
            score += 0.15;
        }
        // Calibração feita num sensor do mesmo tamanho: ligeiramente preferida.
        score -= (l.crop - camera_crop.unwrap_or(l.crop)).abs().min(1.0) * 0.05;
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, l));
        }
    }
    best.filter(|b| b.0 >= 0.45)
        .map(|b| Match { lens: b.1, camera_crop: camera_crop.unwrap_or(b.1.crop) })
}

/// Interpolação linear por focal entre as duas entradas vizinhas (fora do intervalo: a mais
/// próxima). `key` dá a focal de cada entrada.
fn interp_focal<const N: usize>(rows: &[[f32; N]], focal: f32, skip: usize) -> Option<[f32; N]> {
    if rows.is_empty() {
        return None;
    }
    let mut sorted: Vec<&[f32; N]> = rows.iter().collect();
    sorted.sort_by(|a, b| a[0].total_cmp(&b[0]));
    if focal <= sorted[0][0] {
        return Some(*sorted[0]);
    }
    if focal >= sorted[sorted.len() - 1][0] {
        return Some(*sorted[sorted.len() - 1]);
    }
    let i = sorted.iter().position(|r| r[0] >= focal)?;
    let (a, b) = (sorted[i - 1], sorted[i]);
    let t = if b[0] > a[0] { (focal - a[0]) / (b[0] - a[0]) } else { 0.0 };
    let mut out = *a;
    for k in skip..N {
        out[k] = a[k] + (b[k] - a[k]) * t;
    }
    out[0] = focal;
    Some(out)
}

/// Vinheta para (focal, abertura): distância mais longa calibrada; interpolação na abertura
/// (escala logarítmica) e depois na focal.
fn vignetting(rows: &[[f32; 6]], focal: f32, aperture: f32) -> Option<[f32; 3]> {
    let mut focals: Vec<f32> = rows.iter().map(|r| r[0]).collect();
    focals.sort_by(f32::total_cmp);
    focals.dedup();
    let at_focal = |f: f32| -> Option<[f32; 3]> {
        let mut by_ap: Vec<[f32; 4]> = Vec::new();
        let mut aps: Vec<f32> = rows.iter().filter(|r| r[0] == f).map(|r| r[1]).collect();
        aps.sort_by(f32::total_cmp);
        aps.dedup();
        for a in aps {
            let r = rows
                .iter()
                .filter(|r| r[0] == f && r[1] == a)
                .max_by(|x, y| x[2].total_cmp(&y[2]))?;
            by_ap.push([a.ln(), r[3], r[4], r[5]]);
        }
        interp_focal(&by_ap, aperture.ln(), 1).map(|v| [v[1], v[2], v[3]])
    };
    let per_focal: Vec<[f32; 4]> =
        focals.iter().filter_map(|&f| at_focal(f).map(|k| [f, k[0], k[1], k[2]])).collect();
    interp_focal(&per_focal, focal, 1).map(|v| [v[1], v[2], v[3]])
}

/// Perfil desta fotografia, ou `None` (sem objetiva identificada ou sem focal).
pub fn profile_for(q: &ProfileQuery) -> Option<LensProfile> {
    let m = find_lens(q)?;
    let l = m.lens;
    let focal = q.focal.filter(|f| *f > 0.5).or_else(|| focal_range(&l.model).map(|r| r.0))?;
    let mut p = LensProfile { enabled: true, ..Default::default() };
    p.name = if norm(&l.model).starts_with(&norm(&l.maker)) || l.maker.is_empty() {
        l.model.clone()
    } else {
        format!("{} {}", l.maker, l.model)
    };
    // Raio na unidade Hugin da calibração (meia-largura menor do sensor de calibração) por
    // unidade de meia-diagonal desta imagem.
    p.dist_scale = l.crop * (l.aspect * l.aspect + 1.0).sqrt() / m.camera_crop;
    p.vig_scale = l.crop / m.camera_crop;
    // Distorção: interpola só entradas do modelo da mais próxima.
    if let Some(nearest) =
        l.dist.iter().min_by(|a, b| (a[0] - focal).abs().total_cmp(&(b[0] - focal).abs()))
    {
        let same: Vec<[f32; 5]> = l.dist.iter().filter(|r| r[1] == nearest[1]).copied().collect();
        if let Some(d) = interp_focal(&same, focal, 2) {
            p.dist_model = d[1] as u8;
            p.dist = [d[2], d[3], d[4]];
        }
    }
    if let Some(t) = interp_focal(&l.tca, focal, 1) {
        p.tca_model = if t[3] == 0.0 && t[4] == 0.0 && t[5] == 0.0 && t[6] == 0.0 { 2 } else { 1 };
        p.tca = [t[1], t[2], t[3], t[4], t[5], t[6]];
    }
    if let Some(ap) = q.aperture.filter(|a| *a > 0.5) {
        if let Some(v) = vignetting(&l.vig, focal, ap) {
            p.vig = v;
        }
    }
    let _ = DIAG_35MM;
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(make: &str, model: &str, lens: &str, focal: f32, ap: f32) -> ProfileQuery {
        ProfileQuery {
            camera_make: make.into(),
            camera_model: model.into(),
            lens: lens.into(),
            focal: Some(focal),
            focal35: None,
            aperture: Some(ap),
        }
    }

    #[test]
    fn parses_names() {
        assert_eq!(focal_range("FE 24-70mm F2.8 GM"), Some((24.0, 70.0)));
        assert_eq!(focal_range("XF56mmF1.2 R WR"), Some((56.0, 56.0)));
        assert_eq!(focal_range("EF 50mm f/1.4 USM"), Some((50.0, 50.0)));
        assert_eq!(max_aperture("E 18-55mm f/3.5-5.6 OSS"), Some(3.5));
        assert_eq!(max_aperture("XF56mmF1.2 R WR"), Some(1.2));
        assert_eq!(max_aperture("RF24-70mm F2.8 L IS USM"), Some(2.8));
        assert_eq!(focal_range("OLYMPUS M.14-42mm F3.5-5.6 EZ"), Some((14.0, 42.0)));
        assert_eq!(focal_range("Vario-Elmar T 1:3.5-5.6 / 18-56 ASPH."), Some((18.0, 56.0)));
        assert_eq!(max_aperture("Vario-Elmar T 1:3.5-5.6 / 18-56 ASPH."), Some(3.5));
    }

    #[test]
    fn finds_lenses_by_exif_names() {
        // Nome do EXIF diferente do da base de dados (maiúsculas, "F2.8" vs "f/2.8").
        let p = profile_for(&q("Sony", "ILCE-7M4", "FE 24-70mm F2.8 GM", 35.0, 2.8))
            .expect("perfil Sony");
        assert!(p.name.contains("24-70mm") && p.name.to_lowercase().contains("gm"), "{}", p.name);
        assert!(p.dist_model != 0);
        let p = profile_for(&q("Canon", "Canon EOS R3", "RF24-70mm F2.8 L IS USM", 24.0, 4.0))
            .expect("perfil Canon RF");
        assert!(p.name.contains("24-70"), "{}", p.name);
        // A versão II é outra objetiva: não usa o perfil da versão I.
        let ii = profile_for(&q("Sony", "ILCE-7M4", "FE 24-70mm F2.8 GM II", 35.0, 2.8));
        assert!(ii.as_ref().is_none_or(|p| p.name.ends_with("II")), "{:?}", ii.map(|p| p.name));
        // Uma objetiva que não existe não recebe um perfil "parecido".
        assert!(profile_for(&q("Sony", "ILCE-7M4", "FE 23-71mm F2.9 XX", 35.0, 2.9)).is_none());
        assert!(profile_for(&q("Sony", "ILCE-7M4", "", 35.0, 2.8)).is_none());
        // Modelo sem o fabricante (como a LibRaw o dá): a câmara é encontrada (APS-C 1.622).
        let p =
            profile_for(&q("Canon", "EOS 40D", "EF-S 17-85mm f/4-5.6 IS USM", 17.0, 4.0)).unwrap();
        assert!(
            (p.dist_scale - 1.611 * (1.5f32 * 1.5 + 1.0).sqrt() / 1.622).abs() < 0.01,
            "{}",
            p.dist_scale
        );
    }

    #[test]
    fn coefficients_follow_focal_and_aperture() {
        // E 18-55mm (Sony, calibrada na NEX-7): ptlens 18 mm → a=0.02897; a 55 mm → 0.0022.
        let wide = profile_for(&q("Sony", "NEX-7", "E 18-55mm F3.5-5.6 OSS", 18.0, 3.5)).unwrap();
        assert_eq!(wide.dist_model, 3);
        assert!((wide.dist[0] - 0.02897).abs() < 1e-5, "{:?}", wide.dist);
        let tele = profile_for(&q("Sony", "NEX-7", "E 18-55mm F3.5-5.6 OSS", 55.0, 5.6)).unwrap();
        assert!((tele.dist[0] - 0.00220).abs() < 1e-5);
        let mid = profile_for(&q("Sony", "NEX-7", "E 18-55mm F3.5-5.6 OSS", 19.0, 3.5)).unwrap();
        assert!((mid.dist[0] - 0.02895).abs() < 1e-5);
        // Mesmo sensor da calibração (APS-C 1.534): escala = meia-diagonal / meia-altura.
        assert!(
            (wide.dist_scale - (1.5f32 * 1.5 + 1.0).sqrt()).abs() < 0.01,
            "{}",
            wide.dist_scale
        );
        // Vinheta mais forte em plena abertura do que fechada.
        assert!(wide.vignetting_gain(1.0) > 1.2, "{}", wide.vignetting_gain(1.0));
        let closed = profile_for(&q("Sony", "NEX-7", "E 18-55mm F3.5-5.6 OSS", 18.0, 8.0)).unwrap();
        assert!(closed.vignetting_gain(1.0) < wide.vignetting_gain(1.0));
    }
}
