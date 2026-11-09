//! Agrupamento de rostos por identidade (vetores SFace normalizados).
//!
//! - Rostos confirmados pelo utilizador definem pessoas fixas (nunca mudam).
//! - Rostos automáticos juntam-se a uma pessoa fixa ou a um grupo novo por semelhança
//!   (coseno) ao centroide; grupos novos muito parecidos fundem-se.
//! - Só rostos de boa qualidade criam grupos; os pequenos/fracos apenas se juntam.
//! - Rostos excluídos pelo utilizador (“não é esta pessoa”) são ignorados.
//! - Dois rostos da **mesma fotografia** nunca são a mesma pessoa.

use std::collections::{HashMap, HashSet};

/// Limiar oficial do SFace para “mesma pessoa” é 0,363; usa-se um pouco acima para
/// evitar fundir pessoas diferentes (é mais fácil juntar dois grupos do que separá-los).
pub const JOIN: f32 = 0.42;
pub const MERGE: f32 = 0.50;
/// Rostos fracos só se juntam com uma semelhança maior.
pub const WEAK_JOIN: f32 = 0.48;

#[derive(Debug, Clone, PartialEq)]
pub struct FaceInput {
    pub id: i64,
    /// Fotografia do rosto.
    pub asset: i64,
    pub identity: Vec<f32>,
    pub px: f32,
    pub presence: f32,
    pub sharpness: f32,
    pub person: Option<i64>,
    /// 0 automático, 1 confirmado, 2 excluído.
    pub source: i64,
}

impl FaceInput {
    fn strong(&self) -> bool {
        self.px >= 56.0 && self.presence >= 0.8
    }
    fn quality(&self) -> f32 {
        self.px.min(300.0) * self.presence * (0.5 + self.sharpness.clamp(0.0, 3.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Existing(i64),
    New(usize),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Clustering {
    /// Apenas rostos automáticos (confirmados/excluídos não aparecem).
    pub assignments: Vec<(i64, Target)>,
    pub new_clusters: usize,
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

struct Centroid {
    sum: Vec<f32>,
    unit: Vec<f32>,
    n: usize,
    assets: HashSet<i64>,
}

impl Centroid {
    fn new(v: &[f32], asset: i64) -> Self {
        Centroid { sum: v.to_vec(), unit: v.to_vec(), n: 1, assets: HashSet::from([asset]) }
    }
    fn add(&mut self, v: &[f32], asset: i64) {
        self.assets.insert(asset);
        for (s, x) in self.sum.iter_mut().zip(v) {
            *s += x;
        }
        let n = self.sum.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
        self.unit = self.sum.iter().map(|x| x / n).collect();
        self.n += 1;
    }
    fn merge(&mut self, o: &Centroid) {
        self.assets.extend(o.assets.iter().copied());
        for (s, x) in self.sum.iter_mut().zip(&o.sum) {
            *s += x;
        }
        let n = self.sum.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
        self.unit = self.sum.iter().map(|x| x / n).collect();
        self.n += o.n;
    }
}

/// Melhor centroide que ainda não tem um rosto desta fotografia.
fn best(cs: &[Centroid], v: &[f32], asset: i64) -> Option<(usize, f32)> {
    cs.iter()
        .enumerate()
        .filter(|(_, c)| !c.assets.contains(&asset))
        .map(|(i, c)| (i, dot(&c.unit, v)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

pub fn cluster(faces: &[FaceInput]) -> Clustering {
    // Pessoas fixas: centroides dos rostos confirmados.
    let mut fixed_ids: Vec<i64> = Vec::new();
    let mut fixed: Vec<Centroid> = Vec::new();
    let mut fixed_index: HashMap<i64, usize> = HashMap::new();
    for f in faces.iter().filter(|f| f.source == 1 && !f.identity.is_empty()) {
        let Some(p) = f.person else { continue };
        match fixed_index.get(&p) {
            Some(&i) => fixed[i].add(&f.identity, f.asset),
            None => {
                fixed_index.insert(p, fixed.len());
                fixed_ids.push(p);
                fixed.push(Centroid::new(&f.identity, f.asset));
            }
        }
    }
    let mut auto: Vec<&FaceInput> =
        faces.iter().filter(|f| f.source == 0 && !f.identity.is_empty()).collect();
    // Os melhores rostos primeiro: definem centroides estáveis.
    auto.sort_by(|a, b| {
        b.strong().cmp(&a.strong()).then(b.quality().total_cmp(&a.quality())).then(a.id.cmp(&b.id))
    });
    let mut clusters: Vec<Centroid> = Vec::new();
    let mut members: Vec<Vec<i64>> = Vec::new();
    let mut out: Vec<(i64, Target)> = Vec::new();
    for f in auto {
        let need = if f.strong() { JOIN } else { WEAK_JOIN };
        let to_fixed = best(&fixed, &f.identity, f.asset).filter(|b| b.1 >= need);
        let to_new = best(&clusters, &f.identity, f.asset).filter(|b| b.1 >= need);
        match (to_fixed, to_new) {
            (Some(a), b) if b.is_none_or(|b| a.1 >= b.1) => {
                fixed[a.0].assets.insert(f.asset);
                out.push((f.id, Target::Existing(fixed_ids[a.0])));
            }
            (_, Some(b)) => {
                clusters[b.0].add(&f.identity, f.asset);
                members[b.0].push(f.id);
            }
            _ if f.strong() => {
                clusters.push(Centroid::new(&f.identity, f.asset));
                members.push(vec![f.id]);
            }
            _ => {}
        }
    }
    // Fusão de grupos novos muito parecidos (união simples, maior absorve menor).
    let mut alive = vec![true; clusters.len()];
    loop {
        let mut pair: Option<(usize, usize, f32)> = None;
        for i in 0..clusters.len() {
            if !alive[i] {
                continue;
            }
            for j in (i + 1)..clusters.len() {
                if !alive[j] {
                    continue;
                }
                if !clusters[i].assets.is_disjoint(&clusters[j].assets) {
                    continue;
                }
                let s = dot(&clusters[i].unit, &clusters[j].unit);
                if s >= MERGE && pair.is_none_or(|p| s > p.2) {
                    pair = Some((i, j, s));
                }
            }
        }
        let Some((i, j, _)) = pair else { break };
        let (keep, gone) = if clusters[i].n >= clusters[j].n { (i, j) } else { (j, i) };
        let g = std::mem::take(&mut members[gone]);
        members[keep].extend(g);
        let other = Centroid {
            sum: clusters[gone].sum.clone(),
            unit: clusters[gone].unit.clone(),
            n: clusters[gone].n,
            assets: clusters[gone].assets.clone(),
        };
        clusters[keep].merge(&other);
        alive[gone] = false;
    }
    // Grupos com menos de 2 rostos não viram pessoas (convidados vistos uma vez).
    let mut new_clusters = 0;
    for (i, m) in members.iter().enumerate() {
        if !alive[i] || m.len() < 2 {
            continue;
        }
        for &face in m {
            out.push((face, Target::New(new_clusters)));
        }
        new_clusters += 1;
    }
    Clustering { assignments: out, new_clusters }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Identidades sintéticas: base aleatória por pessoa + ruído (cos ≈ 0,6–0,8 com a base).
    fn vec_for(person: u32, noise_seed: u32, noise: f32) -> Vec<f32> {
        let mut s = person.wrapping_mul(2_654_435_761).wrapping_add(17);
        let mut n =
            noise_seed.wrapping_mul(40_503).wrapping_add(person.wrapping_mul(7919)).wrapping_add(3);
        let rnd = |st: &mut u32| {
            *st ^= *st << 13;
            *st ^= *st >> 17;
            *st ^= *st << 5;
            (*st % 2001) as f32 / 1000.0 - 1.0
        };
        let v: Vec<f32> = (0..128).map(|_| rnd(&mut s) + noise * rnd(&mut n)).collect();
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.into_iter().map(|x| x / norm).collect()
    }

    fn face(id: i64, person: u32, k: u32, px: f32) -> FaceInput {
        FaceInput {
            id,
            asset: id,
            identity: vec_for(person, k, 0.8),
            px,
            presence: 0.95,
            sharpness: 2.0,
            person: None,
            source: 0,
        }
    }

    #[test]
    fn groups_same_people_and_keeps_strangers_apart() {
        let mut faces = Vec::new();
        let mut id = 0;
        // 3 pessoas com 20 rostos cada + 30 convidados vistos uma vez.
        for p in 0..3 {
            for k in 0..20 {
                id += 1;
                faces.push(face(id, p, k, 120.0));
            }
        }
        for g in 0..30 {
            id += 1;
            faces.push(face(id, 100 + g, 0, 90.0));
        }
        let c = cluster(&faces);
        assert_eq!(c.new_clusters, 3, "três pessoas; convidados isolados não viram grupo");
        let mut by_cluster: HashMap<usize, Vec<i64>> = HashMap::new();
        for (f, t) in &c.assignments {
            if let Target::New(i) = t {
                by_cluster.entry(*i).or_default().push(*f);
            }
        }
        for m in by_cluster.values() {
            assert_eq!(m.len(), 20);
            let p = (m[0] - 1) / 20;
            assert!(m.iter().all(|f| (f - 1) / 20 == p), "grupo puro");
        }
    }

    #[test]
    fn confirmed_people_attract_new_faces_and_excluded_are_ignored() {
        let mut faces: Vec<FaceInput> = (0..5)
            .map(|k| FaceInput { person: Some(77), source: 1, ..face(k + 1, 0, k as u32, 150.0) })
            .collect();
        faces.push(face(10, 0, 50, 140.0)); // a mesma pessoa, automática
        faces.push(face(11, 0, 51, 44.0)); // pequena, mas parecida: junta-se
        faces.push(FaceInput { source: 2, ..face(12, 0, 52, 150.0) }); // excluída
        faces.push(face(13, 1, 0, 150.0)); // outra pessoa, sozinha
        let c = cluster(&faces);
        let t: HashMap<i64, Target> = c.assignments.iter().copied().collect();
        assert_eq!(t.get(&10), Some(&Target::Existing(77)));
        assert_eq!(t.get(&11), Some(&Target::Existing(77)));
        assert!(!t.contains_key(&12) && !t.contains_key(&13));
        assert!(t.keys().all(|k| *k >= 10), "confirmados não são reatribuídos");
    }

    #[test]
    fn two_faces_in_the_same_photo_are_never_the_same_person() {
        // Gémeos (identidades quase iguais) lado a lado em 4 fotografias.
        let mut faces = Vec::new();
        for k in 0..4 {
            let a = face(k * 2 + 1, 5, k as u32, 120.0);
            let mut b = face(k * 2 + 2, 5, 100 + k as u32, 120.0);
            b.asset = a.asset;
            faces.push(a);
            faces.push(b);
        }
        let c = cluster(&faces);
        let mut by: HashMap<usize, Vec<i64>> = HashMap::new();
        for (f, t) in &c.assignments {
            if let Target::New(i) = t {
                by.entry(*i).or_default().push(*f);
            }
        }
        assert!(!by.is_empty());
        for m in by.values() {
            let assets: HashSet<i64> =
                m.iter().map(|f| faces.iter().find(|x| x.id == *f).unwrap().asset).collect();
            assert_eq!(
                assets.len(),
                m.len(),
                "um grupo nunca tem dois rostos da mesma fotografia: {m:?}"
            );
        }
    }

    #[test]
    fn performance_30000_faces() {
        // Em debug (sem otimizações) usa-se uma amostra menor; o limite de tempo só vale em release.
        let n: u32 = if cfg!(debug_assertions) { 6_000 } else { 30_000 };
        let mut faces = Vec::new();
        for i in 0..n {
            // 250 pessoas recorrentes (80 %) + convidados únicos (20 %).
            let p = if i % 5 == 0 { 10_000 + i } else { i % 250 };
            faces.push(face(i as i64, p, i, if i % 3 == 0 { 45.0 } else { 110.0 }));
        }
        let t = std::time::Instant::now();
        let c = cluster(&faces);
        let e = t.elapsed();
        eprintln!("{n} rostos: {} grupos em {e:.2?}", c.new_clusters);
        assert!((200..=300).contains(&c.new_clusters), "{}", c.new_clusters);
        if !cfg!(debug_assertions) {
            assert!(e.as_secs_f32() < 30.0);
        }
    }
}
