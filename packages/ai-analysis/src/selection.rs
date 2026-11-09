//! Seleção assistida: grupos, pontuações por critério, etiquetas e explicações.
//!
//! Princípios:
//! - **nunca** se apaga nem se altera nada: o resultado são recomendações;
//! - as decisões do utilizador (favorita/rejeitada) prevalecem e nunca são alteradas;
//! - cada fotografia guarda pontuações **individuais** (foco, olhos, expressão, exposição…),
//!   não só uma nota global;
//! - cada recomendação traz razões estruturadas (códigos + valores) que a interface traduz.

use crate::metrics::{exposure_score, noise_score, sharpness_score, TechnicalMetrics};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Rosto analisado (coordenadas normalizadas 0–1 na imagem orientada).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceInfo {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Largura do rosto em píxeis na imagem analisada (fiabilidade dos olhos).
    pub px: f32,
    pub blink_left: f32,
    pub blink_right: f32,
    pub smile: f32,
    pub jaw_open: f32,
    /// log10(variância do Laplaciano) do rosto normalizado.
    pub sharpness: f32,
    /// Idem, região dos olhos.
    pub eye_sharpness: f32,
    pub presence: f32,
    /// Rosto cortado pela margem da fotografia.
    pub cut: bool,
}

/// Abaixo desta largura (px), os olhos não são avaliados (resultado pouco fiável).
pub const MIN_FACE_PX_FOR_EYES: f32 = 48.0;

impl FaceInfo {
    pub fn eyes_reliable(&self) -> bool {
        self.px >= MIN_FACE_PX_FOR_EYES && self.presence >= 0.6 && !self.cut
    }
    /// “Fecho” dos olhos com compensação do semicerrar de quem ri (o modelo sobe o
    /// eyeBlink quando a pessoa ri com os olhos semicerrados).
    pub fn closure(&self) -> f32 {
        let raw = self.blink_left.max(self.blink_right);
        let laugh = ((self.smile - 0.5) / 0.5).clamp(0.0, 1.0);
        (raw - 0.25 * laugh).clamp(0.0, 1.0)
    }
    fn area(&self) -> f32 {
        self.w * self.h
    }
}

/// Dados de entrada por fotografia.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoInput {
    pub id: i64,
    /// Instante de captura em milissegundos (relógio da câmara).
    pub capture_ms: Option<i64>,
    pub camera: Option<String>,
    /// Pasta + nome sem extensão, em minúsculas (deteção de pares RAW+JPEG).
    pub stem_key: String,
    /// Número de sequência do ficheiro (ex.: IMG_0123 → 123).
    pub file_seq: Option<u64>,
    pub is_raw: bool,
    pub metrics: TechnicalMetrics,
    pub faces: Vec<FaceInfo>,
    pub phash: u64,
    pub embedding: Option<Vec<f32>>,
    /// Decisão do utilizador: −1 rejeitada, 0 nenhuma, 1 favorita.
    pub user_flag: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Conservative,
    #[default]
    Balanced,
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub mode: Mode,
    /// Número aproximado de fotografias a entregar (None = sem objetivo).
    pub target: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Label {
    Best,
    Alternative,
    Review,
    Reject,
    Duplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupKind {
    /// Mesmo instante/momento (rajada, sequência, várias câmaras).
    Sequence,
    /// Fotografias praticamente idênticas.
    Duplicate,
    /// O mesmo disparo em RAW e JPEG.
    RawJpeg,
}

/// Pontuações individuais 0–1 (None = não aplicável, ex.: sem rostos).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scores {
    pub focus: f32,
    pub motion: f32,
    pub exposure: f32,
    pub noise: f32,
    pub eyes: Option<f32>,
    pub expression: Option<f32>,
    pub composition: f32,
    pub technical: f32,
    pub people: Option<f32>,
    pub overall: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reason {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f32>,
}

fn r(code: &str) -> Reason {
    Reason { code: code.into(), value: None }
}
fn rv(code: &str, v: f32) -> Reason {
    Reason { code: code.into(), value: Some(v) }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub id: i64,
    pub label: Label,
    pub group: Option<usize>,
    /// Posição no grupo (0 = melhor).
    pub rank: usize,
    pub group_size: usize,
    pub scores: Scores,
    /// Frase-resumo (chave de tradução), ex.: `bestInSequence`.
    pub summary: String,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub kind: GroupKind,
    /// Membros ordenados do melhor para o pior.
    pub members: Vec<i64>,
}

// --------------------------------------------------------------------------- pontuação

fn face_sharpness_score(log_var: f32) -> f32 {
    // Rosto normalizado a 128 px: ≤ 1,2 desfocado; ≥ 2,4 nítido.
    ((log_var - 1.2) / 1.2).clamp(0.0, 1.0)
}

/// Rostos principais: os 4 maiores, desde que tenham pelo menos 30 % da área do maior.
fn main_faces(faces: &[FaceInfo]) -> Vec<&FaceInfo> {
    let mut v: Vec<&FaceInfo> = faces.iter().collect();
    v.sort_by(|a, b| b.area().total_cmp(&a.area()));
    let biggest = v.first().map(|f| f.area()).unwrap_or(0.0);
    v.into_iter().take(4).filter(|f| f.area() >= biggest * 0.3).collect()
}

pub fn score(p: &PhotoInput) -> Scores {
    let m = &p.metrics;
    let main = main_faces(&p.faces);
    let reliable: Vec<&&FaceInfo> = main.iter().filter(|f| f.eyes_reliable()).collect();
    let focus = if main.is_empty() {
        sharpness_score(m.focus_peak)
    } else {
        // Foco no rosto principal mais nítido, mas com peso para o pior dos principais.
        let s: Vec<f32> =
            main.iter().map(|f| face_sharpness_score(f.sharpness.max(f.eye_sharpness))).collect();
        let best = s.iter().copied().fold(0.0, f32::max);
        let worst = s.iter().copied().fold(1.0, f32::min);
        0.7 * best + 0.3 * worst
    };
    let motion = 1.0 - m.motion_blur;
    let exposure = exposure_score(m);
    let noise = noise_score(m.noise);
    let eyes = (!reliable.is_empty()).then(|| {
        reliable
            .iter()
            .map(|f| (1.0 - (f.closure() - 0.3) / 0.4).clamp(0.0, 1.0))
            .fold(1.0, f32::min)
    });
    let expression = (!main.is_empty()).then(|| {
        let v: f32 = main
            .iter()
            .map(|f| {
                let talking = (f.jaw_open - 0.35).max(0.0) * (1.0 - f.smile);
                (0.55 + 0.45 * f.smile - 0.6 * talking).clamp(0.0, 1.0)
            })
            .sum::<f32>()
            / main.len() as f32;
        v
    });
    let tilt_pen = reliable_tilt(p).map(|t| ((t.abs() - 1.0) / 4.0).clamp(0.0, 1.0)).unwrap_or(0.0);
    let cut_pen = if main.iter().any(|f| f.cut) { 0.4 } else { 0.0 };
    let composition = (1.0 - 0.5 * tilt_pen - cut_pen).clamp(0.0, 1.0);
    let mut s = Scores {
        focus,
        motion,
        exposure,
        noise,
        eyes,
        expression,
        composition,
        ..Default::default()
    };
    combine(&mut s);
    s
}

/// Inclinação só é fiável com arestas nítidas (numa imagem muito desfocada é ruído).
fn reliable_tilt(p: &PhotoInput) -> Option<f32> {
    p.metrics.tilt_degrees.filter(|_| sharpness_score(p.metrics.focus_peak) >= 0.3)
}

/// Recalcula as pontuações agregadas a partir dos critérios individuais.
fn combine(s: &mut Scores) {
    s.technical = 0.45 * s.focus + 0.2 * s.motion + 0.25 * s.exposure + 0.1 * s.noise;
    s.people = match (s.eyes, s.expression) {
        (Some(e), Some(x)) => Some(0.65 * e + 0.35 * x),
        (None, Some(x)) => Some(x),
        _ => None,
    };
    s.overall = match s.people {
        Some(pp) => 0.5 * s.technical + 0.35 * pp + 0.15 * s.composition,
        None => 0.85 * s.technical + 0.15 * s.composition,
    };
}

/// Medida bruta de foco (log10 da variância do Laplaciano): rosto principal ou imagem.
fn raw_focus(p: &PhotoInput, use_faces: bool) -> f32 {
    if use_faces {
        main_faces(&p.faces).iter().map(|f| f.sharpness.max(f.eye_sharpness)).fold(0.0, f32::max)
    } else {
        p.metrics.focus_peak
    }
}

/// Dentro de um grupo, o foco é comparado **entre as fotografias do grupo**: a escala
/// absoluta satura em imagens nítidas e não distingue “nítida” de “a mais nítida da
/// rajada”. O ruído medido baixa com a desfocagem (o detalhe fino confunde-se com ruído),
/// por isso no grupo usa-se a mediana — numa rajada o ISO é o mesmo.
fn relative_adjust(photos: &[PhotoInput], scores: &mut [Scores], members: &[usize]) {
    if members.len() < 2 {
        return;
    }
    let use_faces = members.iter().all(|&m| !main_faces(&photos[m].faces).is_empty());
    let max = members.iter().map(|&m| raw_focus(&photos[m], use_faces)).fold(f32::MIN, f32::max);
    let mut noise: Vec<f32> = members.iter().map(|&m| scores[m].noise).collect();
    noise.sort_by(f32::total_cmp);
    let median_noise = noise[noise.len() / 2];
    for &m in members {
        // 0,6 em log10 ≈ 4× menos variância do Laplaciano → relativo 0.
        let rel = (1.0 - (max - raw_focus(&photos[m], use_faces)) / 0.6).clamp(0.0, 1.0);
        let s = &mut scores[m];
        s.focus = 0.4 * s.focus + 0.6 * rel;
        s.noise = median_noise;
        combine(s);
    }
}

/// Falhas técnicas graves que justificam sugerir rejeição.
fn severe_faults(p: &PhotoInput, s: &Scores) -> Vec<Reason> {
    let mut v = Vec::new();
    if s.focus < 0.2 {
        v.push(r("outOfFocus"));
    }
    if p.metrics.motion_blur > 0.6 {
        v.push(r("motionBlur"));
    }
    if p.metrics.highlights_clipped > 0.15 {
        v.push(rv("overexposed", p.metrics.highlights_clipped * 100.0));
    }
    if p.metrics.brightness < 0.07 {
        v.push(r("underexposed"));
    }
    v
}

// --------------------------------------------------------------------------- grupos

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn similarity(a: &PhotoInput, b: &PhotoInput) -> f32 {
    match (&a.embedding, &b.embedding) {
        (Some(x), Some(y)) => cosine(x, y),
        // Sem embeddings: aproximação pelo pHash (0 bits diferentes → 1; 32 → 0).
        _ => 1.0 - (crate::hash::hamming(a.phash, b.phash) as f32 / 32.0).min(1.0),
    }
}

fn face_layout_similar(a: &PhotoInput, b: &PhotoInput) -> bool {
    let (fa, fb) = (main_faces(&a.faces), main_faces(&b.faces));
    if fa.is_empty() || fb.is_empty() {
        return fa.len() == fb.len();
    }
    (fa.len() as i32 - fb.len() as i32).abs() <= 1
}

struct Dsu(Vec<usize>);
impl Dsu {
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.0[r] != r {
            r = self.0[r];
        }
        let mut c = x;
        while self.0[c] != r {
            let n = self.0[c];
            self.0[c] = r;
            c = n;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.0[rb] = ra;
        }
    }
}

/// Liga duas fotografias consecutivas (por tempo) do mesmo instante.
fn same_moment(a: &PhotoInput, b: &PhotoInput) -> bool {
    let sim = similarity(a, b);
    let ph = crate::hash::hamming(a.phash, b.phash);
    let same_cam = a.camera.is_some() && a.camera == b.camera;
    match (a.capture_ms, b.capture_ms) {
        (Some(ta), Some(tb)) => {
            let dt = (tb - ta).abs();
            (dt <= 3_000 && (sim >= 0.80 || ph <= 16) && face_layout_similar(a, b))
                || (dt <= 15_000 && sim >= 0.90)
                // Câmaras diferentes no mesmo instante (segundo fotógrafo).
                || (!same_cam && dt <= 2_000 && sim >= 0.72 && face_layout_similar(a, b))
        }
        _ => {
            let consecutive = same_cam
                && matches!((a.file_seq, b.file_seq), (Some(x), Some(y)) if x.abs_diff(y) <= 2);
            consecutive && sim >= 0.85
        }
    }
}

/// Quase-duplicado = **cópia** do mesmo ficheiro/imagem (exportação repetida, cópia com
/// outro nome). As fotografias de uma rajada também são visualmente quase iguais, mas são
/// instantes diferentes (olhos, expressão) e por isso são ordenadas, não descartadas:
/// só contam como duplicado se tiverem o mesmo instante (< 50 ms), não tiverem data, ou
/// estiverem a mais de 15 s de distância.
fn near_duplicate(a: &PhotoInput, b: &PhotoInput) -> bool {
    let visually_same = crate::hash::hamming(a.phash, b.phash) <= 4 && similarity(a, b) >= 0.97;
    let copy_like = match (a.capture_ms, b.capture_ms) {
        (Some(x), Some(y)) => (x - y).abs() < 50 || (x - y).abs() > 15_000,
        _ => true,
    };
    visually_same && copy_like
}

/// Agrupa por pares RAW+JPEG, quase-duplicados e momentos. Devolve (grupos, membro→grupo).
pub fn build_groups(photos: &[PhotoInput]) -> Vec<(GroupKind, Vec<usize>)> {
    let n = photos.len();
    let mut groups: Vec<(GroupKind, Vec<usize>)> = Vec::new();
    // 1) Pares RAW+JPEG: mesmo nome base na mesma pasta.
    let mut by_stem: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, p) in photos.iter().enumerate() {
        by_stem.entry(p.stem_key.as_str()).or_default().push(i);
    }
    let mut companion = vec![false; n]; // JPEG de um par (não participa nos outros grupos)
    for idx in by_stem.values() {
        let raws: Vec<usize> = idx.iter().copied().filter(|&i| photos[i].is_raw).collect();
        let others: Vec<usize> = idx.iter().copied().filter(|&i| !photos[i].is_raw).collect();
        if raws.len() == 1 && !others.is_empty() {
            let mut members = raws.clone();
            members.extend(&others);
            for &o in &others {
                companion[o] = true;
            }
            groups.push((GroupKind::RawJpeg, members));
        }
    }
    // 2) Ordem temporal (sem data: pela ordem do ficheiro).
    let mut order: Vec<usize> = (0..n).filter(|&i| !companion[i]).collect();
    order.sort_by_key(|&i| {
        (
            photos[i].capture_ms.unwrap_or(i64::MAX),
            photos[i].file_seq.unwrap_or(u64::MAX),
            photos[i].id,
        )
    });
    let mut dsu = Dsu((0..n).collect());
    let mut dup = Dsu((0..n).collect());
    for (k, &i) in order.iter().enumerate() {
        // Compara com as fotografias seguintes dentro de uma janela (várias câmaras
        // intercaladas, rajadas longas).
        for &j in order.iter().skip(k + 1).take(12) {
            let (a, b) = (&photos[i], &photos[j]);
            if let (Some(ta), Some(tb)) = (a.capture_ms, b.capture_ms) {
                if tb - ta > 15_000 {
                    break;
                }
            }
            if near_duplicate(a, b) {
                dup.union(i, j);
                dsu.union(i, j);
            } else if same_moment(a, b) {
                dsu.union(i, j);
            }
        }
    }
    // 3) Quase-duplicados fora da janela temporal (cópias exportadas, etc.): pHash igual.
    let mut by_hash: HashMap<u64, usize> = HashMap::new();
    for &i in &order {
        if let Some(&j) = by_hash.get(&photos[i].phash) {
            if near_duplicate(&photos[i], &photos[j]) {
                dup.union(j, i);
                dsu.union(j, i);
            }
        } else {
            by_hash.insert(photos[i].phash, i);
        }
    }
    let mut seq: HashMap<usize, Vec<usize>> = HashMap::new();
    for &i in &order {
        seq.entry(dsu.find(i)).or_default().push(i);
    }
    let mut seq: Vec<Vec<usize>> = seq.into_values().filter(|v| v.len() > 1).collect();
    seq.sort_by_key(|v| v[0]);
    for members in seq {
        let mut dmap: HashMap<usize, usize> = HashMap::new();
        for &m in &members {
            *dmap.entry(dup.find(m)).or_default() += 1;
        }
        let all_dup = dmap.len() == 1;
        groups.push((if all_dup { GroupKind::Duplicate } else { GroupKind::Sequence }, members));
    }
    groups
}

// --------------------------------------------------------------------------- piscadelas relativas

/// Dentro de um grupo, compara cada pessoa consigo própria: se o fecho dos olhos de um
/// rosto sobe muito acima da mediana *dessa pessoa* no grupo, é uma piscadela.
fn relative_blinks(photos: &[PhotoInput], members: &[usize]) -> HashMap<usize, u32> {
    // (cx, cy, [(foto, fecho)]) — uma “pista” por pessoa, seguida pela posição do rosto.
    type Track = (f32, f32, Vec<(usize, f32)>);
    let mut tracks: Vec<Track> = Vec::new();
    for &m in members {
        for f in photos[m].faces.iter().filter(|f| f.eyes_reliable()) {
            let (cx, cy) = (f.x + f.w / 2.0, f.y + f.h / 2.0);
            let tol = (f.w.max(f.h) * 0.8).max(0.04);
            match tracks.iter_mut().find(|t| (t.0 - cx).abs() < tol && (t.1 - cy).abs() < tol) {
                Some(t) => t.2.push((m, f.closure())),
                None => tracks.push((cx, cy, vec![(m, f.closure())])),
            }
        }
    }
    let mut out: HashMap<usize, u32> = HashMap::new();
    for (_, _, obs) in tracks.iter().filter(|t| t.2.len() >= 3) {
        let mut v: Vec<f32> = obs.iter().map(|o| o.1).collect();
        v.sort_by(|a, b| a.total_cmp(b));
        let median = v[v.len() / 2];
        for &(m, c) in obs {
            if c >= 0.45 && c - median >= 0.3 {
                *out.entry(m).or_default() += 1;
            }
        }
    }
    out
}

// --------------------------------------------------------------------------- etiquetas

struct ModeParams {
    max_best: usize,
    alt_window: f32,
    max_alt: usize,
    reject_gap: f32,
    single_best: f32,
}

fn params(mode: Mode) -> ModeParams {
    match mode {
        Mode::Conservative => ModeParams {
            max_best: 3,
            alt_window: 0.12,
            max_alt: 3,
            reject_gap: 0.35,
            single_best: 0.45,
        },
        Mode::Balanced => ModeParams {
            max_best: 3,
            alt_window: 0.08,
            max_alt: 2,
            reject_gap: 0.25,
            single_best: 0.55,
        },
        Mode::Strict => ModeParams {
            max_best: 1,
            alt_window: 0.04,
            max_alt: 1,
            reject_gap: 0.15,
            single_best: 0.65,
        },
    }
}

fn best_count(mode: Mode, n: usize) -> usize {
    match mode {
        Mode::Conservative => n.div_ceil(4).clamp(1, 3),
        Mode::Balanced => ((n as f32 / 6.0).round() as usize).clamp(1, 3),
        Mode::Strict => 1,
    }
}

/// Razões pelas quais `i` se destaca no grupo (só critérios com variação real no grupo).
fn strengths(photos: &[PhotoInput], scores: &[Scores], members: &[usize], i: usize) -> Vec<Reason> {
    let mut out = Vec::new();
    let max_of = |f: &dyn Fn(usize) -> Option<f32>| {
        members.iter().filter_map(|&m| f(m)).fold(f32::MIN, f32::max)
    };
    let min_of = |f: &dyn Fn(usize) -> Option<f32>| {
        members.iter().filter_map(|&m| f(m)).fold(f32::MAX, f32::min)
    };
    let mut check = |code: &str, f: &dyn Fn(usize) -> Option<f32>, spread: f32| {
        if let Some(v) = f(i) {
            let (hi, lo) = (max_of(f), min_of(f));
            if hi - lo >= spread && v >= hi - 0.02 {
                out.push(r(code));
            }
        }
    };
    let has_faces = !photos[i].faces.is_empty();
    check(if has_faces { "sharpestFace" } else { "sharpest" }, &|m| Some(scores[m].focus), 0.08);
    check("eyesOpen", &|m| scores[m].eyes, 0.25);
    check("bestExpression", &|m| scores[m].expression, 0.12);
    check("bestExposure", &|m| Some(scores[m].exposure), 0.1);
    check("leastMotion", &|m| Some(scores[m].motion), 0.15);
    // Todos os rostos visíveis quando noutras fotografias há menos.
    let faces_here = main_faces(&photos[i].faces).iter().filter(|f| !f.cut).count();
    let max_faces = members
        .iter()
        .map(|&m| main_faces(&photos[m].faces).iter().filter(|f| !f.cut).count())
        .max()
        .unwrap_or(0);
    let min_faces = members
        .iter()
        .map(|&m| main_faces(&photos[m].faces).iter().filter(|f| !f.cut).count())
        .min()
        .unwrap_or(0);
    if faces_here == max_faces && max_faces > min_faces && max_faces >= 2 {
        out.push(rv("allFacesVisible", max_faces as f32));
    }
    out.truncate(3);
    out
}

fn weaknesses(p: &PhotoInput, s: &Scores, blinks: u32) -> Vec<Reason> {
    let mut v = severe_faults(p, s);
    let main = main_faces(&p.faces);
    let closed = main.iter().filter(|f| f.eyes_reliable() && f.closure() >= 0.6).count();
    if closed > 0 {
        v.push(rv("eyesClosed", closed as f32));
    } else if blinks > 0 {
        v.push(rv("blink", blinks as f32));
    }
    if main.iter().any(|f| f.cut) {
        v.push(r("faceCut"));
    }
    // Rosto principal desfocado enquanto outra zona da imagem está nítida (foco falhado).
    if let Some(f) = main.first() {
        if face_sharpness_score(f.sharpness) < 0.3 && sharpness_score(p.metrics.focus_peak) > 0.7 {
            v.push(r("missedFocus"));
        }
    }
    if s.noise < 0.4 {
        v.push(r("noisy"));
    }
    if let Some(t) = reliable_tilt(p) {
        if t.abs() >= 2.0 {
            v.push(rv("tilted", t));
        }
    }
    if p.metrics.highlights_clipped > 0.04 && !v.iter().any(|r| r.code == "overexposed") {
        v.push(rv("highlightsClipped", p.metrics.highlights_clipped * 100.0));
    }
    if p.metrics.shadows_crushed > 0.25 {
        v.push(rv("shadowsCrushed", p.metrics.shadows_crushed * 100.0));
    }
    v
}

pub fn select(photos: &[PhotoInput], settings: Settings) -> (Vec<Group>, Vec<Recommendation>) {
    let pm = params(settings.mode);
    let mut scores: Vec<Scores> = photos.iter().map(score).collect();
    let raw_groups = build_groups(photos);
    for (kind, members) in &raw_groups {
        if *kind != GroupKind::RawJpeg {
            relative_adjust(photos, &mut scores, members);
        }
    }
    let scores = scores;
    let mut recs: Vec<Option<Recommendation>> = vec![None; photos.len()];
    let mut groups_out = Vec::new();
    let mut in_group = vec![false; photos.len()];
    let mk = |i: usize, label, group, rank, size, summary: &str, reasons| Recommendation {
        id: photos[i].id,
        label,
        group,
        rank,
        group_size: size,
        scores: scores[i],
        summary: summary.into(),
        reasons,
    };

    // Pares RAW+JPEG primeiro: o JPEG é marcado como duplicado do RAW.
    let mut pair_rep: HashMap<usize, usize> = HashMap::new();
    for (kind, members) in raw_groups.iter().filter(|g| g.0 == GroupKind::RawJpeg) {
        let gi = groups_out.len();
        let raw = members[0];
        for &j in &members[1..] {
            recs[j] = Some(mk(
                j,
                Label::Duplicate,
                Some(gi),
                1,
                members.len(),
                "rawJpegPair",
                vec![r("rawJpegPair")],
            ));
            pair_rep.insert(j, raw);
            in_group[j] = true;
        }
        groups_out
            .push(Group { kind: *kind, members: members.iter().map(|&m| photos[m].id).collect() });
    }

    for (kind, members) in raw_groups.iter().filter(|g| g.0 != GroupKind::RawJpeg) {
        let gi = groups_out.len();
        let blinks = relative_blinks(photos, members);
        let adj = |m: usize| -> f32 {
            // Piscadela relativa penaliza a pontuação de olhos.
            let s = scores[m].overall;
            if blinks.get(&m).copied().unwrap_or(0) > 0 {
                s - 0.2
            } else {
                s
            }
        };
        let mut ranked = members.clone();
        ranked.sort_by(|&a, &b| adj(b).total_cmp(&adj(a)).then(photos[a].id.cmp(&photos[b].id)));
        let n = ranked.len();
        let top = adj(ranked[0]);
        let nb = best_count(settings.mode, n).min(pm.max_best);
        let mut n_alt = 0;
        for (rank, &m) in ranked.iter().enumerate() {
            in_group[m] = true;
            let s = adj(m);
            let faults = weaknesses(&photos[m], &scores[m], blinks.get(&m).copied().unwrap_or(0));
            let severe = !severe_faults(&photos[m], &scores[m]).is_empty()
                || faults.iter().any(|r| r.code == "eyesClosed" || r.code == "blink");
            let (label, summary, reasons) = if *kind == GroupKind::Duplicate && rank > 0 {
                (Label::Duplicate, "nearDuplicate", vec![r("nearDuplicate")])
            } else if rank < nb && (rank == 0 || (top - s <= pm.alt_window && !severe)) {
                let mut rs = strengths(photos, &scores, &ranked, m);
                if rank > 0 {
                    rs.insert(0, rv("closeToBest", (top - s) * 100.0));
                }
                if top < 0.35 {
                    rs.insert(0, r("bestAvailableLowQuality"));
                }
                (
                    Label::Best,
                    if *kind == GroupKind::Duplicate {
                        "bestOfDuplicates"
                    } else {
                        "bestInSequence"
                    },
                    rs,
                )
            } else if !severe && top - s <= pm.alt_window && n_alt < pm.max_alt {
                n_alt += 1;
                let mut rs = vec![rv("closeToBest", (top - s) * 100.0)];
                rs.extend(strengths(photos, &scores, &ranked, m));
                (Label::Alternative, "alternative", rs)
            } else if severe || top - s >= pm.reject_gap {
                let mut rs = faults.clone();
                if rs.is_empty() {
                    rs.push(r("muchWorseThanBest"));
                }
                (Label::Reject, "rejectSuggested", rs)
            } else {
                let mut rs = faults.clone();
                rs.insert(0, r("weakerThanBest"));
                (Label::Review, "review", rs)
            };
            recs[m] = Some(mk(m, label, Some(gi), rank, n, summary, reasons));
        }
        groups_out
            .push(Group { kind: *kind, members: ranked.iter().map(|&m| photos[m].id).collect() });
    }

    // Fotografias isoladas.
    for i in 0..photos.len() {
        if in_group[i] {
            continue;
        }
        let s = &scores[i];
        let faults = weaknesses(&photos[i], s, 0);
        let severe = !severe_faults(&photos[i], s).is_empty()
            || faults.iter().any(|r| r.code == "eyesClosed");
        let (label, summary, reasons) = if severe {
            (Label::Reject, "rejectSuggested", faults)
        } else if s.overall >= pm.single_best {
            let mut rs = Vec::new();
            if s.focus >= 0.7 {
                rs.push(r(if photos[i].faces.is_empty() { "sharp" } else { "sharpFace" }));
            }
            if s.eyes.is_some_and(|e| e >= 0.9) {
                rs.push(r("eyesOpen"));
            }
            if s.exposure >= 0.85 {
                rs.push(r("goodExposure"));
            }
            (Label::Best, "goodSingle", rs)
        } else {
            (Label::Review, "reviewSingle", faults)
        };
        recs[i] = Some(mk(i, label, None, 0, 1, summary, reasons));
    }

    let mut recs: Vec<Recommendation> =
        recs.into_iter().map(|r| r.expect("todas as fotografias recebem recomendação")).collect();
    if let Some(target) = settings.target {
        apply_target(photos, &mut recs, target);
    }
    // O par JPEG herda a etiqueta “duplicada”; nada mais a fazer.
    let _ = pair_rep;
    (groups_out, recs)
}

/// Ajusta as etiquetas para que Melhor + Alternativa ≈ `target`, privilegiando a
/// diversidade: primeiro a melhor de cada momento, depois as segundas, etc. Fotografias
/// marcadas como favoritas pelo utilizador contam sempre.
fn apply_target(photos: &[PhotoInput], recs: &mut [Recommendation], target: usize) {
    let user_picks = photos.iter().filter(|p| p.user_flag == 1).count();
    let budget = target.saturating_sub(user_picks);
    let eligible = |r: &Recommendation, p: &PhotoInput| {
        p.user_flag == 0 && matches!(r.label, Label::Best | Label::Alternative | Label::Review)
    };
    // Candidatos por (posição no grupo, −pontuação).
    let mut cands: Vec<usize> =
        (0..recs.len()).filter(|&i| eligible(&recs[i], &photos[i])).collect();
    cands.sort_by(|&a, &b| {
        recs[a]
            .rank
            .cmp(&recs[b].rank)
            .then(recs[b].scores.overall.total_cmp(&recs[a].scores.overall))
    });
    let chosen: std::collections::HashSet<usize> = cands.iter().copied().take(budget).collect();
    for &i in &cands {
        let r = &mut recs[i];
        if chosen.contains(&i) {
            if r.label == Label::Review {
                r.label = if r.rank == 0 { Label::Best } else { Label::Alternative };
                r.summary = "includedForTarget".into();
                r.reasons.insert(
                    0,
                    Reason { code: "includedForTarget".into(), value: Some(target as f32) },
                );
            }
        } else if matches!(r.label, Label::Best | Label::Alternative) {
            r.label = Label::Review;
            r.summary = "outsideTarget".into();
            r.reasons
                .insert(0, Reason { code: "outsideTarget".into(), value: Some(target as f32) });
        }
    }
}

#[cfg(test)]
mod tests;
