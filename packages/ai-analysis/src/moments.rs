//! Classificação de momentos de um casamento.
//!
//! Combina, por fotografia:
//! - **hora corrigida** (sincronização de câmaras) e a posição no dia em relação à
//!   cerimónia detetada;
//! - **localização** (GPS) e intervalos de tempo para dividir o dia em cenas;
//! - **cena e objetos** (classificador ImageNet: igreja, altar, vestido, sapatos,
//!   microfone, pratos, carros, palco…);
//! - **rostos** (quantos, tamanhos, disposição, beijo) e **pessoas identificadas**
//!   (noiva/noivo e pais);
//! - **semelhança visual** (embedding) com fotografias que o utilizador já classificou.
//!
//! Só atribui um momento quando a confiança é suficiente; caso contrário a fotografia
//! fica em “Por classificar”. As decisões do utilizador nunca são alteradas.

use crate::selection::Reason;
use std::collections::HashMap;

/// Chaves dos momentos predefinidos (iguais às da base de dados).
pub const KINDS: &[&str] = &[
    "bride_prep",
    "groom_prep",
    "details",
    "dress",
    "shoes",
    "rings",
    "invitations",
    "decoration",
    "arrival",
    "ceremony",
    "entrance",
    "ring_exchange",
    "kiss",
    "exit",
    "couple_session",
    "groups",
    "cocktail",
    "reception",
    "speeches",
    "dinner",
    "cake",
    "first_dance",
    "party",
    "night",
    "closing",
];

/// Confiança mínima para atribuir um momento automaticamente.
pub const MIN_CONFIDENCE: f32 = 0.5;
/// Diferença mínima para o segundo candidato (evita escolher entre empates).
pub const MIN_MARGIN: f32 = 0.08;
/// Intervalo que separa cenas.
pub const SCENE_GAP_MS: i64 = 10 * 60 * 1000;
pub const SCENE_DISTANCE_M: f64 = 400.0;

static LABELS: &str = include_str!("imagenet_labels.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MomentLabel {
    Kind(&'static str),
    Custom(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FaceBrief {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub px: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MomentPhoto {
    pub id: i64,
    /// Hora corrigida (ms, relógio local).
    pub time_ms: Option<i64>,
    pub gps: Option<(f64, f64)>,
    /// Classes ImageNet mais prováveis (índice, probabilidade).
    pub scene: Vec<(u16, f32)>,
    pub embedding: Option<Vec<f32>>,
    pub faces: Vec<FaceBrief>,
    /// Papéis das pessoas identificadas presentes (`bride`, `groom`, `bride_mother`…).
    pub roles: Vec<String>,
    pub brightness: f32,
    pub iso: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MomentResult {
    pub id: i64,
    /// `None` = “Por classificar”.
    pub label: Option<MomentLabel>,
    pub confidence: f32,
    pub reasons: Vec<Reason>,
}

fn r(code: &str) -> Reason {
    Reason { code: code.into(), value: None }
}
fn rv(code: &str, v: f32) -> Reason {
    Reason { code: code.into(), value: Some(v) }
}

/// Classes ImageNet por pista, com peso. Uma classe inexistente é ignorada.
const CUES: &[(&str, &[(&str, f32)])] = &[
    (
        "dress",
        &[
            ("gown", 1.0),
            ("hoopskirt", 1.0),
            ("overskirt", 1.0),
            ("mosquito net", 0.5),
            ("kimono", 0.3),
            ("abaya", 0.3),
        ],
    ),
    (
        "shoes",
        &[
            ("Loafer", 1.0),
            ("sandal", 1.0),
            ("clog", 0.9),
            ("cowboy boot", 0.8),
            ("running shoe", 0.7),
            ("shoe shop", 0.8),
            ("sock", 0.4),
        ],
    ),
    ("rings", &[("necklace", 0.6), ("chain", 0.5), ("buckle", 0.3), ("chain mail", 0.3)]),
    (
        "invitations",
        &[
            ("envelope", 1.0),
            ("menu", 1.0),
            ("book jacket", 0.6),
            ("packet", 0.6),
            ("notebook", 0.4),
            ("binder", 0.4),
            ("comic book", 0.3),
        ],
    ),
    (
        "decoration",
        &[
            ("vase", 0.8),
            ("candle", 0.7),
            ("daisy", 0.6),
            ("balloon", 0.6),
            ("lampshade", 0.5),
            ("table lamp", 0.5),
            ("pot", 0.5),
            ("greenhouse", 0.4),
            ("dining table", 0.4),
            ("restaurant", 0.3),
            ("fountain", 0.3),
        ],
    ),
    (
        "details",
        &[
            ("perfume", 1.0),
            ("lipstick", 0.8),
            ("face powder", 0.8),
            ("hair spray", 0.8),
            ("digital watch", 0.6),
            ("wallet", 0.6),
            ("purse", 0.6),
            ("bow tie", 0.5),
            ("Windsor tie", 0.5),
            ("sunglasses", 0.3),
            ("suit", 0.3),
        ],
    ),
    (
        "cake",
        &[
            ("trifle", 0.8),
            ("ice cream", 0.6),
            ("chocolate sauce", 0.6),
            ("bakery", 0.6),
            ("confectionery", 0.6),
            ("potpie", 0.3),
            ("cleaver", 0.3),
        ],
    ),
    (
        "dinner",
        &[
            ("plate", 0.8),
            ("restaurant", 0.8),
            ("dining table", 0.7),
            ("soup bowl", 0.6),
            ("consomme", 0.6),
            ("wine bottle", 0.5),
            ("red wine", 0.5),
            ("goblet", 0.5),
            ("carbonara", 0.5),
            ("plate rack", 0.3),
            ("tray", 0.3),
            ("beer glass", 0.3),
        ],
    ),
    (
        "cocktail",
        &[
            ("cocktail shaker", 0.8),
            ("goblet", 0.4),
            ("red wine", 0.4),
            ("wine bottle", 0.4),
            ("beer bottle", 0.4),
            ("beer glass", 0.4),
            ("eggnog", 0.3),
            ("patio", 0.3),
        ],
    ),
    ("speeches", &[("microphone", 1.0), ("loudspeaker", 0.5)]),
    (
        "ceremony",
        &[
            ("church", 1.0),
            ("altar", 1.0),
            ("organ", 0.6),
            ("monastery", 0.6),
            ("bell cote", 0.5),
            ("throne", 0.3),
            ("prayer rug", 0.3),
        ],
    ),
    (
        "arrival",
        &[
            ("limousine", 1.0),
            ("convertible", 0.8),
            ("sports car", 0.6),
            ("grille", 0.6),
            ("car wheel", 0.5),
            ("cab", 0.5),
            ("beach wagon", 0.5),
            ("minivan", 0.4),
            ("jeep", 0.3),
        ],
    ),
    (
        "party",
        &[
            ("stage", 0.6),
            ("spotlight", 0.6),
            ("electric guitar", 0.4),
            ("theater curtain", 0.4),
            ("drum", 0.3),
            ("acoustic guitar", 0.3),
            ("cinema", 0.3),
        ],
    ),
    (
        "scenic",
        &[
            ("lakeside", 0.6),
            ("seashore", 0.6),
            ("valley", 0.6),
            ("alp", 0.5),
            ("cliff", 0.5),
            ("promontory", 0.5),
            ("park bench", 0.5),
            ("breakwater", 0.4),
            ("sandbar", 0.4),
            ("castle", 0.3),
            ("palace", 0.3),
            ("boathouse", 0.3),
        ],
    ),
    (
        "bride_prep",
        &[
            ("four-poster", 0.4),
            ("wardrobe", 0.4),
            ("hair spray", 0.5),
            ("lipstick", 0.5),
            ("face powder", 0.5),
            ("medicine chest", 0.4),
            ("studio couch", 0.3),
            ("quilt", 0.3),
            ("bathtub", 0.3),
            ("gown", 0.4),
            ("brassiere", 0.4),
        ],
    ),
    (
        "groom_prep",
        &[
            ("bow tie", 0.7),
            ("Windsor tie", 0.7),
            ("suit", 0.6),
            ("groom", 0.6),
            ("military uniform", 0.2),
        ],
    ),
];

/// Tabela de pistas resolvida para índices ImageNet.
pub struct Cues {
    table: Vec<(&'static str, Vec<(usize, f32)>)>,
}

impl Default for Cues {
    fn default() -> Self {
        Self::new()
    }
}

impl Cues {
    pub fn new() -> Self {
        let labels: Vec<&str> = LABELS.lines().collect();
        let table = CUES
            .iter()
            .map(|(k, list)| {
                (
                    *k,
                    list.iter()
                        .filter_map(|(name, w)| {
                            labels.iter().position(|l| l == name).map(|i| (i, *w))
                        })
                        .collect(),
                )
            })
            .collect();
        Cues { table }
    }

    /// Evidência 0–1 de cada pista para uma fotografia (+ a classe que mais contribuiu).
    fn evidence(&self, scene: &[(u16, f32)]) -> HashMap<&'static str, (f32, usize)> {
        let probs: HashMap<usize, f32> = scene.iter().map(|(i, p)| (*i as usize, *p)).collect();
        let mut out = HashMap::new();
        for (k, list) in &self.table {
            let mut raw = 0.0;
            let mut top = (0.0, 0usize);
            for (i, w) in list {
                let v = probs.get(i).copied().unwrap_or(0.0) * w;
                raw += v;
                if v > top.0 {
                    top = (v, *i);
                }
            }
            let e = (raw / 0.35).min(1.0);
            if e > 0.0 {
                out.insert(*k, (e, top.1));
            }
        }
        out
    }
}

pub fn label_name(i: usize) -> &'static str {
    LABELS.lines().nth(i).unwrap_or("?")
}

fn haversine_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1, la2, lo2) =
        (a.0.to_radians(), a.1.to_radians(), b.0.to_radians(), b.1.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2)
        + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * 6_371_000.0 * h.sqrt().asin()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Hora do dia (0–24) a partir de ms locais.
fn hour_of(t: i64) -> f32 {
    (t.rem_euclid(86_400_000) as f32) / 3_600_000.0
}

/// Pistas a partir dos rostos.
struct FaceCues {
    main: usize,
    all: usize,
    group: f32,
    kiss: f32,
    couple: bool,
    close_up_no_faces: bool,
    /// Largura (px) do maior rosto.
    biggest_px: f32,
}

fn face_cues(p: &MomentPhoto) -> FaceCues {
    let mut f: Vec<&FaceBrief> = p.faces.iter().filter(|f| f.px >= 24.0).collect();
    f.sort_by(|a, b| b.w.total_cmp(&a.w));
    let biggest = f.first().map(|x| x.w).unwrap_or(0.0);
    let main: Vec<&&FaceBrief> = f.iter().filter(|x| x.w >= biggest * 0.45).collect();
    // Grupo posado: ≥ 5 rostos principais, em 1–2 filas (pouca dispersão vertical).
    let group = if main.len() >= 5 {
        let ys: Vec<f32> = main.iter().map(|x| x.y + x.h / 2.0).collect();
        let mean = ys.iter().sum::<f32>() / ys.len() as f32;
        let sd = (ys.iter().map(|y| (y - mean).powi(2)).sum::<f32>() / ys.len() as f32).sqrt();
        let fh = main.iter().map(|x| x.h).sum::<f32>() / main.len() as f32;
        if sd <= fh * 1.6 {
            (0.55 + 0.05 * (main.len() as f32 - 5.0)).min(0.9)
        } else {
            0.4
        }
    } else {
        0.0
    };
    // Beijo: os dois maiores rostos, parecidos em tamanho e quase a tocar-se.
    let kiss = if f.len() >= 2 && f[0].px >= 60.0 {
        let (a, b) = (f[0], f[1]);
        let ratio = b.w / a.w;
        let (ca, cb) = ((a.x + a.w / 2.0, a.y + a.h / 2.0), (b.x + b.w / 2.0, b.y + b.h / 2.0));
        let dist = ((ca.0 - cb.0).powi(2) + (ca.1 - cb.1).powi(2)).sqrt();
        let mw = (a.w + b.w) / 2.0;
        if ratio >= 0.6 && dist <= mw * 1.05 && (ca.1 - cb.1).abs() <= mw * 0.6 {
            0.7
        } else {
            0.0
        }
    } else {
        0.0
    };
    FaceCues {
        main: main.len(),
        all: f.len(),
        group,
        kiss,
        couple: main.len() == 2,
        close_up_no_faces: f.is_empty(),
        biggest_px: f.first().map(|x| x.px).unwrap_or(0.0),
    }
}

/// Uma cena: fotografias seguidas no tempo, sem grandes intervalos nem mudanças de local.
fn scenes(order: &[usize], photos: &[MomentPhoto]) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut last: Option<&MomentPhoto> = None;
    let mut last_gps: Option<(f64, f64)> = None;
    for &i in order {
        let p = &photos[i];
        let split = match (last.and_then(|l| l.time_ms), p.time_ms) {
            (Some(a), Some(b)) => b - a > SCENE_GAP_MS,
            _ => last.is_none(),
        } || matches!((last_gps, p.gps), (Some(a), Some(b)) if haversine_m(a, b) > SCENE_DISTANCE_M);
        if split || out.is_empty() {
            out.push(Vec::new());
        }
        out.last_mut().unwrap().push(i);
        last = Some(p);
        if p.gps.is_some() {
            last_gps = p.gps;
        }
    }
    out
}

#[derive(Default)]
struct Cand {
    scores: HashMap<MomentLabel, (f32, Vec<Reason>)>,
}

impl Cand {
    fn add(&mut self, l: MomentLabel, score: f32, reasons: Vec<Reason>) {
        let e = self.scores.entry(l).or_insert((0.0, Vec::new()));
        if score > e.0 {
            *e = (score, reasons);
        }
    }
}

const K: fn(&'static str) -> MomentLabel = MomentLabel::Kind;

/// Momento “pai” de um sub-momento: não competem entre si (o mais específico ganha
/// quando tem confiança suficiente).
pub fn parent(k: &str) -> Option<&'static str> {
    match k {
        "entrance" | "ring_exchange" | "kiss" => Some("ceremony"),
        "speeches" | "dinner" | "cake" | "first_dance" | "party" | "cocktail"
        | "couple_session" | "groups" | "exit" => Some("reception"),
        "dress" | "shoes" | "rings" | "invitations" | "decoration" => Some("details"),
        _ => None,
    }
}

fn family(a: MomentLabel, b: MomentLabel) -> bool {
    match (a, b) {
        (MomentLabel::Kind(x), MomentLabel::Kind(y)) => {
            parent(x) == Some(y) || parent(y) == Some(x)
        }
        _ => false,
    }
}

/// Classifica as fotografias sem decisão do utilizador. `user` = decisões do utilizador
/// (usadas como exemplos para fotografias parecidas); `None` no valor = “Por classificar”.
pub fn classify(
    photos: &[MomentPhoto],
    user: &HashMap<i64, Option<MomentLabel>>,
    cues: &Cues,
) -> Vec<MomentResult> {
    let n = photos.len();
    let ev: Vec<HashMap<&'static str, (f32, usize)>> =
        photos.iter().map(|p| cues.evidence(&p.scene)).collect();
    let fc: Vec<FaceCues> = photos.iter().map(face_cues).collect();
    let e = |i: usize, k: &str| ev[i].get(k).map(|x| x.0).unwrap_or(0.0);
    let cue_reason = |i: usize, k: &str| -> Reason {
        rv(&format!("scene:{}", label_name(ev[i].get(k).map(|x| x.1).unwrap_or(0))), e(i, k))
    };

    let mut order: Vec<usize> = (0..n).filter(|&i| photos[i].time_ms.is_some()).collect();
    order.sort_by_key(|&i| (photos[i].time_ms, photos[i].id));
    let sc = scenes(&order, photos);
    let mut scene_of = vec![usize::MAX; n];
    for (s, m) in sc.iter().enumerate() {
        for &i in m {
            scene_of[i] = s;
        }
    }

    // Cerimónia: a cena com mais evidência de igreja/altar (e várias fotografias).
    let scene_mean =
        |s: usize, k: &str| sc[s].iter().map(|&i| e(i, k)).sum::<f32>() / sc[s].len().max(1) as f32;
    let ceremony = (0..sc.len())
        .map(|s| (s, scene_mean(s, "ceremony"), sc[s].len()))
        .filter(|(_, m, len)| *m >= 0.18 && *len >= 3)
        .max_by(|a, b| (a.1 * (a.2 as f32).sqrt()).total_cmp(&(b.1 * (b.2 as f32).sqrt())));
    // Início e fim: primeira e última fotografia da cena com pistas de cerimónia (a
    // chegada pode estar na mesma cena, minutos antes).
    let cer_span = ceremony.map(|(s, _, _)| {
        let t: Vec<i64> = sc[s]
            .iter()
            .filter(|&&i| e(i, "ceremony") >= 0.15)
            .filter_map(|&i| photos[i].time_ms)
            .collect();
        (s, *t.first().unwrap(), *t.last().unwrap())
    });
    let day_end = order.last().and_then(|&i| photos[i].time_ms);

    let mut cands: Vec<Cand> = (0..n).map(|_| Cand::default()).collect();
    for i in 0..n {
        let p = &photos[i];
        let f = &fc[i];
        let c = &mut cands[i];
        let has = |role: &str| p.roles.iter().any(|r| r == role);
        let (bride, groom) = (has("bride"), has("groom"));
        let t = p.time_ms;
        let rel = match (t, cer_span) {
            (Some(t), Some((_, a, b))) => Some(if t < a {
                -1
            } else if t > b {
                1
            } else {
                0
            }),
            _ => None,
        };
        let in_ceremony = cer_span
            .is_some_and(|(s, a, b)| scene_of[i] == s && t.is_some_and(|t| t >= a && t <= b));

        // Detalhes e objetos: valem a qualquer hora, mas só em fotografias sem um rosto em
        // destaque (um retrato não é uma fotografia de detalhe).
        let no_face_bonus = if f.close_up_no_faces {
            0.15
        } else if f.all <= 1 {
            -0.05
        } else {
            -0.2
        };
        for k in ["dress", "shoes", "rings", "invitations", "decoration", "details"] {
            let v = e(i, k);
            if v > 0.0 && f.biggest_px < 80.0 {
                let mut s = 0.3 + 0.5 * v + no_face_bonus;
                if k == "rings" {
                    s -= 0.1; // sem classe “anel” no ImageNet: pista mais fraca
                }
                c.add(K(k), s, vec![cue_reason(i, k)]);
            }
        }
        // Momentos com objetos característicos.
        for (k, base) in [
            ("cake", 0.35),
            ("speeches", 0.35),
            ("dinner", 0.3),
            ("cocktail", 0.25),
            ("arrival", 0.3),
        ] {
            let v = e(i, k);
            if v > 0.0 {
                let mut s = base + 0.45 * v;
                let mut why = vec![cue_reason(i, k)];
                match (k, rel) {
                    ("arrival", Some(-1)) => {
                        s += 0.15;
                        why.push(r("time:beforeCeremony"));
                    }
                    ("arrival", Some(1)) => s -= 0.2,
                    ("cake" | "speeches" | "dinner" | "cocktail", Some(1)) => {
                        s += 0.1;
                        why.push(r("time:afterCeremony"));
                        // Cocktail nas primeiras horas depois da cerimónia; jantar mais tarde.
                        let since = t.zip(cer_span).map(|(t, (_, _, b))| t - b).unwrap_or(0);
                        let late =
                            since >= 3 * 3600 * 1000 || t.map(hour_of).is_some_and(|h| h >= 19.0);
                        match (k, late) {
                            ("cocktail", false) | ("dinner", true) => s += 0.15,
                            ("cocktail", true) | ("dinner", false) => s -= 0.15,
                            _ => {}
                        }
                    }
                    ("cake" | "speeches" | "dinner" | "cocktail", Some(-1)) => s -= 0.25,
                    _ => {}
                }
                c.add(K(k), s, why);
            }
        }
        // Cerimónia e as suas partes.
        if in_ceremony {
            let (_, a, b) = cer_span.unwrap();
            let frac = t.map(|t| (t - a) as f32 / (b - a).max(1) as f32).unwrap_or(0.5);
            c.add(
                K("ceremony"),
                0.55 + 0.15 * e(i, "ceremony"),
                vec![r("time:ceremonyScene"), cue_reason(i, "ceremony")],
            );
            if f.kiss > 0.0 {
                let s = if bride && groom { 0.85 } else { 0.72 };
                c.add(K("kiss"), s, vec![r("faces:kiss"), r("time:ceremonyScene")]);
            }
            if frac <= 0.15 && f.kiss == 0.0 {
                c.add(K("entrance"), 0.66, vec![r("time:ceremonyStart")]);
            }
            if e(i, "rings") > 0.15 && frac > 0.3 {
                c.add(
                    K("ring_exchange"),
                    0.6 + 0.3 * e(i, "rings"),
                    vec![cue_reason(i, "rings"), r("time:ceremonyScene")],
                );
            }
        } else if e(i, "ceremony") >= 0.5 {
            c.add(K("ceremony"), 0.4 + 0.3 * e(i, "ceremony"), vec![cue_reason(i, "ceremony")]);
        }
        // Antes da cerimónia: preparação.
        if rel == Some(-1) {
            let near_start =
                matches!((t, cer_span), (Some(t), Some((_, a, _))) if a - t <= 30 * 60 * 1000);
            if bride && !groom {
                c.add(K("bride_prep"), 0.72, vec![r("role:bride"), r("time:beforeCeremony")]);
            } else if groom && !bride {
                c.add(K("groom_prep"), 0.72, vec![r("role:groom"), r("time:beforeCeremony")]);
            }
            let (bp, gp) = (e(i, "bride_prep"), e(i, "groom_prep"));
            if bp > 0.2 && bp >= gp {
                c.add(
                    K("bride_prep"),
                    0.45 + 0.3 * bp,
                    vec![cue_reason(i, "bride_prep"), r("time:beforeCeremony")],
                );
            }
            if gp > 0.2 && gp > bp {
                c.add(
                    K("groom_prep"),
                    0.45 + 0.3 * gp,
                    vec![cue_reason(i, "groom_prep"), r("time:beforeCeremony")],
                );
            }
            if near_start && !in_ceremony && f.all >= 1 {
                c.add(K("arrival"), 0.5, vec![r("time:justBeforeCeremony")]);
            }
        }
        // Depois da cerimónia.
        if rel == Some(1) {
            let (_, _, b) = cer_span.unwrap();
            let since = t.unwrap() - b;
            if since <= 30 * 60 * 1000 && f.all >= 4 {
                let s = if since <= 20 * 60 * 1000 { 0.68 } else { 0.55 };
                c.add(
                    K("exit"),
                    s,
                    vec![r("time:justAfterCeremony"), rv("faces:count", f.all as f32)],
                );
            }
            if f.couple && (bride && groom || !p.roles.iter().any(|r| r != "bride" && r != "groom"))
            {
                let mut s = 0.5 + 0.25 * e(i, "scenic");
                if bride && groom {
                    s += 0.15;
                }
                c.add(K("couple_session"), s, vec![r("faces:couple"), cue_reason(i, "scenic")]);
            }
            let hour = t.map(hour_of).unwrap_or(12.0);
            let dark = p.brightness < 0.3 || p.iso.is_some_and(|i| i >= 3200);
            // Grupos posados fazem-se com luz e não colados à saída da cerimónia.
            if f.group > 0.0 && !dark {
                let s = 0.35 + 0.45 * f.group - if since <= 20 * 60 * 1000 { 0.1 } else { 0.0 };
                c.add(K("groups"), s, vec![rv("faces:group", f.main as f32)]);
            }
            if dark && !(5.0..20.0).contains(&hour) {
                if f.couple && f.kiss == 0.0 && e(i, "party") > 0.0 {
                    let s = 0.6 + 0.2 * e(i, "party") + if bride && groom { 0.1 } else { 0.0 };
                    c.add(K("first_dance"), s, vec![r("faces:couple"), cue_reason(i, "party")]);
                }
                if f.all >= 3 {
                    c.add(
                        K("party"),
                        0.52 + 0.25 * e(i, "party"),
                        vec![r("time:night"), rv("faces:count", f.all as f32)],
                    );
                } else if e(i, "party") > 0.2 {
                    c.add(
                        K("party"),
                        0.42 + 0.2 * e(i, "party"),
                        vec![r("time:night"), cue_reason(i, "party")],
                    );
                }
                if f.all == 0 || (f.main <= 2 && e(i, "scenic") > 0.2) {
                    c.add(K("night"), 0.55, vec![r("time:night"), r("light:dark")]);
                }
            }
            if !dark && f.all >= 3 && since <= 3 * 3600 * 1000 {
                c.add(
                    K("cocktail"),
                    0.45,
                    vec![r("time:afterCeremony"), rv("faces:count", f.all as f32)],
                );
            }
            if f.all >= 2 {
                c.add(K("reception"), 0.42, vec![r("time:afterCeremony")]);
            }
            if let Some(end) = day_end {
                let last_minutes = end - t.unwrap() <= 20 * 60 * 1000;
                if last_minutes && !(5.0..23.0).contains(&hour) {
                    // Encerramento: últimos minutos do dia; mais provável com os noivos (saída final).
                    let s = 0.5 + if f.couple || (bride && groom) { 0.12 } else { 0.0 };
                    c.add(K("closing"), s, vec![r("time:endOfDay")]);
                }
            }
        }
        // Noite sem cerimónia detetada: só a hora e a luz.
        if rel.is_none() {
            if let Some(t) = t {
                let h = hour_of(t);
                if !(5.0..21.0).contains(&h) && p.brightness < 0.25 && f.all == 0 {
                    c.add(K("night"), 0.52, vec![r("time:night"), r("light:dark")]);
                }
            }
        }
    }

    // Exemplos do utilizador: fotografias parecidas (mesma cena ou até 20 min) herdam a etiqueta.
    let examples: Vec<(usize, MomentLabel)> =
        (0..n).filter_map(|i| user.get(&photos[i].id).copied().flatten().map(|l| (i, l))).collect();
    if !examples.is_empty() {
        for i in 0..n {
            if user.contains_key(&photos[i].id) {
                continue;
            }
            let Some(ei) = photos[i].embedding.as_deref() else { continue };
            for &(j, l) in &examples {
                let Some(ej) = photos[j].embedding.as_deref() else { continue };
                let near = scene_of[i] == scene_of[j] && scene_of[i] != usize::MAX
                    || matches!((photos[i].time_ms, photos[j].time_ms), (Some(a), Some(b)) if (a - b).abs() <= 20 * 60 * 1000);
                if !near {
                    continue;
                }
                let s = cosine(ei, ej);
                if s >= 0.85 {
                    let score = 0.6 + 0.35 * ((s - 0.85) / 0.15).min(1.0);
                    cands[i].add(l, score, vec![rv("similarToUserChoice", s * 100.0)]);
                }
            }
        }
        // Maioria numa cena: ≥ 3 exemplos e ≥ 70 % iguais → as restantes da cena ganham essa etiqueta.
        for m in &sc {
            let mut votes: HashMap<MomentLabel, usize> = HashMap::new();
            let mut total = 0;
            for &i in m {
                if let Some(Some(l)) = user.get(&photos[i].id) {
                    *votes.entry(*l).or_default() += 1;
                    total += 1;
                }
            }
            if let Some((l, v)) = votes.into_iter().max_by_key(|x| x.1) {
                if total >= 3 && v * 10 >= total * 7 {
                    for &i in m {
                        if !user.contains_key(&photos[i].id) {
                            cands[i].add(l, 0.62, vec![rv("sceneMajorityUserChoice", v as f32)]);
                        }
                    }
                }
            }
        }
    }

    (0..n)
        .filter(|&i| !user.contains_key(&photos[i].id))
        .map(|i| {
            let mut v: Vec<(MomentLabel, f32, Vec<Reason>)> = cands[i].scores.drain().map(|(l, (s, why))| (l, s.min(0.99), why)).collect();
            v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
                        // Um sub-momento com confiança suficiente vence o seu “pai” (beijo > cerimónia).
            if let Some(top) = v.first().map(|x| (x.0, x.1)) {
                if let Some(pos) = v.iter().position(|x| family(x.0, top.0) && matches!((x.0, top.0), (MomentLabel::Kind(c), MomentLabel::Kind(p)) if parent(c) == Some(p)) && x.1 >= MIN_CONFIDENCE.max(top.1 - 0.12)) {
                    let child = v.remove(pos);
                    v.insert(0, child);
                }
            }
            let rival = v.first().and_then(|top| v.iter().skip(1).find(|x| !family(x.0, top.0)).map(|x| x.1));
            let (label, confidence, reasons) = match v.first() {
                Some((l, s, why)) if *s >= MIN_CONFIDENCE && rival.is_none_or(|x| s - x >= MIN_MARGIN) => {
                    (Some(*l), *s, why.iter().filter(|r| r.value.is_none_or(|x| x > 0.0)).cloned().collect())
                }
                Some((_, s, _)) => (None, *s, vec![r(if *s < MIN_CONFIDENCE { "lowConfidence" } else { "ambiguous" })]),
                None => (None, 0.0, vec![r("noEvidence")]),
            };
            MomentResult { id: photos[i].id, label, confidence, reasons }
        })
        .collect()
}

#[cfg(test)]
mod tests;
