//! Futebol: fases do jogo pela hora das fotografias.
//!
//! As fotografias formam blocos de atividade separados por pausas (≥ `PAUSE_MS`). Procura-se a
//! divisão em 1.ª parte e 2.ª parte (cada uma um ou mais blocos seguidos) que melhor explica um
//! jogo: partes de duração parecida (25–45 min + descontos, consoante o escalão), um intervalo
//! entre elas (a pausa maior a meio) e a maior parte das fotografias dentro das partes. O que
//! fica antes é "Antes do jogo" (aquecimento, entrada das equipas) e o que fica depois é
//! "Depois do jogo" (festejos, cumprimentos).
//!
//! Com a hora de início e a duração de cada parte (opcionais), as fases ficam definidas por
//! essas horas, com o intervalo ajustado à pausa real das fotografias.

/// Pausa mínima que separa blocos de atividade.
pub const PAUSE_MS: i64 = 4 * 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchPhase {
    PreGame,
    FirstHalf,
    HalfTime,
    SecondHalf,
    PostGame,
}

impl MatchPhase {
    pub const ALL: [MatchPhase; 5] = [
        MatchPhase::PreGame,
        MatchPhase::FirstHalf,
        MatchPhase::HalfTime,
        MatchPhase::SecondHalf,
        MatchPhase::PostGame,
    ];
    /// Identificador estável (catálogo, interface, pastas).
    pub fn id(self) -> &'static str {
        match self {
            MatchPhase::PreGame => "pre_game",
            MatchPhase::FirstHalf => "first_half",
            MatchPhase::HalfTime => "half_time",
            MatchPhase::SecondHalf => "second_half",
            MatchPhase::PostGame => "post_game",
        }
    }
}

/// Indicações opcionais do utilizador.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MatchHints {
    /// Hora do pontapé de saída (ms, mesma escala das fotografias).
    pub kickoff_ms: Option<i64>,
    /// Duração regulamentar de cada parte (minutos).
    pub half_minutes: Option<u32>,
}

/// Limites das partes (ms): [início, fim] de cada uma.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchTimes {
    pub first: (i64, i64),
    pub second: (i64, i64),
}

impl MatchTimes {
    pub fn phase_of(&self, t: i64) -> MatchPhase {
        if t < self.first.0 {
            MatchPhase::PreGame
        } else if t <= self.first.1 {
            MatchPhase::FirstHalf
        } else if t < self.second.0 {
            MatchPhase::HalfTime
        } else if t <= self.second.1 {
            MatchPhase::SecondHalf
        } else {
            MatchPhase::PostGame
        }
    }
}

/// Blocos de atividade: (início, fim, n.º de fotografias), por ordem.
fn blocks(sorted: &[i64]) -> Vec<(i64, i64, usize)> {
    let mut out: Vec<(i64, i64, usize)> = Vec::new();
    for &t in sorted {
        match out.last_mut() {
            Some(b) if t - b.1 < PAUSE_MS => {
                b.1 = t;
                b.2 += 1;
            }
            _ => out.push((t, t, 1)),
        }
    }
    out
}

/// Fases do jogo; `None` se as fotografias não parecerem um jogo (poucas, sem duas partes).
pub fn detect(times: &[i64], hints: &MatchHints) -> Option<MatchTimes> {
    let mut t: Vec<i64> = times.to_vec();
    t.sort_unstable();
    if t.len() < 20 {
        return None;
    }
    let b = blocks(&t);
    let total = t.len() as f64;
    let min = 60_000f64;
    let half_target = hints.half_minutes.map(|m| m as f64 * min);
    let mut best: Option<(f64, MatchTimes)> = None;
    let n = b.len();
    for a in 0..n {
        // Com hora de início: a 1.ª parte começa perto dela (até 10 min de atraso; 2 min antes).
        if let Some(k) = hints.kickoff_ms {
            if b[a].0 < k - 2 * 60_000 && (a + 1 >= n || b[a + 1].0 < k - 2 * 60_000) && b[a].1 < k {
                continue;
            }
            if b[a].0 > k + 10 * 60_000 {
                continue;
            }
        }
        for e1 in a..n {
            for s2 in e1 + 1..n {
                for e2 in s2..n {
                    let first = (b[a].0, b[e1].1);
                    let second = (b[s2].0, b[e2].1);
                    let d1 = (first.1 - first.0) as f64;
                    let d2 = (second.1 - second.0) as f64;
                    let gap = (second.0 - first.1) as f64;
                    // Partes entre 15 e 60 min, intervalo entre 4 e 30 min.
                    if !(15.0 * min..=60.0 * min).contains(&d1)
                        || !(15.0 * min..=60.0 * min).contains(&d2)
                        || !(4.0 * min..=30.0 * min).contains(&gap)
                    {
                        continue;
                    }
                    let inside: usize = b[a..=e1].iter().chain(&b[s2..=e2]).map(|x| x.2).sum();
                    let balance = (d1 - d2).abs() / d1.max(d2);
                    // A maior pausa entre as partes deve ser o intervalo (não uma paragem).
                    let inner_gap = b[a..=e1]
                        .windows(2)
                        .chain(b[s2..=e2].windows(2))
                        .map(|w| (w[1].0 - w[0].1) as f64)
                        .fold(0.0, f64::max);
                    let mut score = 1.5 * inside as f64 / total - 2.0 * balance + (gap / (15.0 * min)).min(1.0);
                    if inner_gap > gap {
                        score -= 1.0;
                    }
                    if let Some(h) = half_target {
                        // Cada parte: duração regulamentar + até 10 min de descontos.
                        let err = |d: f64| ((d - h).max(0.0) - 10.0 * min).max(0.0) + (h - 5.0 * min - d).max(0.0);
                        score -= (err(d1) + err(d2)) / (10.0 * min);
                    }
                    if let Some(k) = hints.kickoff_ms {
                        score -= ((first.0 - k).abs() as f64 / (10.0 * min)).min(1.0);
                    }
                    if best.as_ref().is_none_or(|x| score > x.0) {
                        best = Some((score, MatchTimes { first, second }));
                    }
                }
            }
        }
    }
    let (_, mut m) = best?;
    if let Some(k) = hints.kickoff_ms {
        // O pontapé de saída é a fronteira certa (as fotografias podem começar um pouco depois).
        if (m.first.0 - k).abs() <= 10 * 60_000 {
            m.first.0 = m.first.0.min(k);
        }
    }
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    /// Fotografias sintéticas de um jogo: rajadas irregulares durante cada fase.
    fn game(pre: i64, half: i64, interval: i64, post: i64, seed: u64) -> (Vec<i64>, MatchTimes) {
        let mut s = seed | 1;
        let mut rnd = move |n: i64| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s % n.max(1) as u64) as i64
        };
        let mut t = Vec::new();
        let mut shoot = |from: i64, to: i64, t: &mut Vec<i64>| {
            let mut x = from;
            while x < to {
                // Rajada de 3–12 fotografias a ~10/s, depois 5–150 s sem disparar.
                for k in 0..3 + rnd(10) {
                    t.push(x + k * 100);
                }
                x += 5_000 + rnd(145_000);
            }
        };
        let k0 = 15 * 3600 * 1000;
        // Aquecimento até 8 min antes do início; equipas entram aos −3 min.
        shoot(k0 - pre, k0 - 8 * MIN, &mut t);
        shoot(k0 - 3 * MIN, k0 - MIN, &mut t);
        let first = (k0, k0 + half + 2 * MIN);
        shoot(first.0, first.1, &mut t);
        let second = (first.1 + interval, first.1 + interval + half + 4 * MIN);
        shoot(second.0, second.1, &mut t);
        shoot(second.1 + 5 * MIN, second.1 + 5 * MIN + post, &mut t);
        (t, MatchTimes { first, second })
    }

    #[test]
    fn finds_the_halves_of_games_of_every_age_group() {
        for (half, interval) in [(45, 15), (40, 12), (35, 10), (30, 10), (25, 8)] {
            for seed in [1u64, 7, 42] {
                let (t, truth) = game(25 * MIN, half * MIN, interval * MIN, 10 * MIN, seed);
                let m = detect(&t, &MatchHints::default()).expect("jogo");
                // A primeira/última fotografia de cada parte pode estar até 2,5 min dentro.
                let near = |a: i64, b: i64| (a - b).abs() <= 150_000;
                assert!(near(m.first.0, truth.first.0) && near(m.first.1, truth.first.1), "{half} min, {seed}: 1.ª {m:?} vs {truth:?}");
                assert!(near(m.second.0, truth.second.0) && near(m.second.1, truth.second.1), "{half} min, {seed}: 2.ª {m:?} vs {truth:?}");
                // Classificação: aquecimento antes, festejos depois.
                assert_eq!(m.phase_of(truth.first.0 - 20 * MIN), MatchPhase::PreGame);
                assert_eq!(m.phase_of(truth.second.1 + 7 * MIN), MatchPhase::PostGame);
                assert_eq!(m.phase_of((truth.first.1 + truth.second.0) / 2), MatchPhase::HalfTime);
            }
        }
    }

    #[test]
    fn kickoff_and_half_length_fix_the_boundaries() {
        let (t, truth) = game(25 * MIN, 30 * MIN, 10 * MIN, 10 * MIN, 3);
        let hints = MatchHints { kickoff_ms: Some(truth.first.0), half_minutes: Some(30) };
        let m = detect(&t, &hints).unwrap();
        assert_eq!(m.first.0, truth.first.0);
        assert!((m.second.0 - truth.second.0).abs() <= 150_000, "{m:?}");
    }

    #[test]
    fn not_a_game() {
        // Poucas fotografias, ou um só bloco contínuo (ex.: sessão de fotografias).
        assert!(detect(&[0, 1000, 2000], &MatchHints::default()).is_none());
        let one_block: Vec<i64> = (0..400).map(|i| i * 20_000).collect();
        assert!(detect(&one_block, &MatchHints::default()).is_none());
    }
}
