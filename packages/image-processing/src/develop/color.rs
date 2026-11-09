//! Gestão de cor do pipeline: espaço de trabalho **linear Rec. 2020** (D-003), saída sRGB.
//!
//! - Entradas sRGB (JPEG, PNG, …) são linearizadas e convertidas para Rec. 2020; os RAW
//!   chegam da LibRaw já em Rec. 2020 linear.
//! - As operações “de aspeto” (contraste, curvas, HSL) trabalham com a curva de
//!   transferência sRGB aplicada às componentes Rec. 2020 (“domínio de visualização”).
//! - A saída converte de volta para sRGB (8 bits), o espaço que o WebView e os ficheiros
//!   de pré-visualização assumem. Sem ajustes, o resultado é o original (±1 nível).

/// sRGB linear → Rec. 2020 linear (D65), ITU-R BT.2087.
pub const SRGB_TO_REC2020: [[f32; 3]; 3] = [
    [0.627_404, 0.329_283, 0.043_313],
    [0.069_097, 0.919_540, 0.011_362],
    [0.016_391, 0.088_013, 0.895_595],
];

/// Rec. 2020 linear → sRGB linear.
pub const REC2020_TO_SRGB: [[f32; 3]; 3] = [
    [1.660_491, -0.587_641, -0.072_850],
    [-0.124_550, 1.132_9, -0.008_349],
    [-0.018_151, -0.100_579, 1.118_73],
];

/// Luminância Rec. 2020.
pub const LUMA: [f32; 3] = [0.2627, 0.6780, 0.0593];

#[inline]
pub fn mul3(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

pub fn matmul(a: &[[f32; 3]; 3], b: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    o
}

#[inline]
pub fn luma(v: [f32; 3]) -> f32 {
    LUMA[0] * v[0] + LUMA[1] * v[1] + LUMA[2] * v[2]
}

/// Curva sRGB: linear → codificado (0–1).
#[inline]
pub fn oetf(x: f32) -> f32 {
    if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

/// Curva sRGB inversa: codificado → linear.
#[inline]
pub fn eotf(x: f32) -> f32 {
    if x <= 0.040_45 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

/// Tabela para acelerar a curva sRGB no intervalo [0, 1] (4096 entradas + interpolação).
pub struct Lut {
    t: Vec<f32>,
}

impl Lut {
    pub const N: usize = 4096;
    pub fn new(f: impl Fn(f32) -> f32) -> Self {
        Lut { t: (0..=Self::N).map(|i| f(i as f32 / Self::N as f32)).collect() }
    }
    /// Fora de [0, 1]: extrapolação pela função limitada aos extremos.
    #[inline]
    pub fn get(&self, x: f32) -> f32 {
        let p = (x.clamp(0.0, 1.0)) * Self::N as f32;
        let i = (p as usize).min(Self::N - 1);
        let f = p - i as f32;
        self.t[i] + (self.t[i + 1] - self.t[i]) * f
    }
}

/// Tabelas partilhadas da curva sRGB.
pub struct Transfer {
    pub to_display: Lut,
    pub to_linear: Lut,
    /// Linear → sRGB 8 bits (com arredondamento).
    pub to_u8: Vec<u8>,
}

impl Transfer {
    pub fn new() -> Self {
        Transfer {
            to_display: Lut::new(oetf),
            to_linear: Lut::new(eotf),
            to_u8: (0..=8192)
                .map(|i| (oetf(i as f32 / 8192.0) * 255.0).round().clamp(0.0, 255.0) as u8)
                .collect(),
        }
    }
    #[inline]
    pub fn encode_u8(&self, linear: f32) -> u8 {
        self.to_u8[(linear.clamp(0.0, 1.0) * 8192.0 + 0.5) as usize]
    }
}

impl Default for Transfer {
    fn default() -> Self {
        Self::new()
    }
}

/// Cromaticidade (x, y) do corpo negro (aproximação de Kim et al., 1667–25000 K).
pub fn planck_xy(k: f32) -> (f32, f32) {
    let t = k.clamp(1667.0, 25000.0) as f64;
    let x = if t <= 4000.0 {
        -0.266_123_9e9 / t.powi(3) - 0.234_358_9e6 / t.powi(2) + 0.877_695_6e3 / t + 0.179_910
    } else {
        -3.025_846_9e9 / t.powi(3) + 2.107_037_9e6 / t.powi(2) + 0.222_634_7e3 / t + 0.240_390
    };
    let y = if t <= 2222.0 {
        -1.106_381_4 * x.powi(3) - 1.348_110_20 * x.powi(2) + 2.185_558_32 * x - 0.202_196_83
    } else if t <= 4000.0 {
        -0.954_947_6 * x.powi(3) - 1.374_185_93 * x.powi(2) + 2.091_370_15 * x - 0.167_488_67
    } else {
        3.081_758_0 * x.powi(3) - 5.873_386_7 * x.powi(2) + 3.751_129_9 * x - 0.370_014_83
    };
    (x as f32, y as f32)
}

/// Branco de uma temperatura em sRGB linear (luminância 1).
fn white_srgb(k: f32) -> [f32; 3] {
    let (x, y) = planck_xy(k);
    let xyz = [x / y, 1.0, (1.0 - x - y) / y];
    const XYZ_TO_SRGB: [[f32; 3]; 3] = [
        [3.240_97, -1.537_383, -0.498_611],
        [-0.969_244, 1.875_968, 0.041_555],
        [0.055_630, -0.203_977, 1.056_972],
    ];
    mul3(&XYZ_TO_SRGB, xyz)
}

/// Temperatura de referência para o balanço relativo (o “neutro” da câmara).
pub const WB_REFERENCE_K: f32 = 5500.0;

/// Matriz de balanço de brancos (em Rec. 2020 linear) para temperatura/matiz relativos.
/// Temperatura +100 → mais quente (como se a luz fosse ~2× mais fria e se compensasse).
pub fn wb_matrix(temperature: f32, tint: f32) -> [[f32; 3]; 3] {
    if temperature == 0.0 && tint == 0.0 {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    let k_scene = WB_REFERENCE_K * 2f32.powf(temperature / 100.0);
    let a = white_srgb(WB_REFERENCE_K);
    let b = white_srgb(k_scene);
    let mut m = [a[0] / b[0], a[1] / b[1], a[2] / b[2]];
    m[1] *= 2f32.powf(-tint / 100.0 * 0.4);
    // Mantém a luminância (só muda a cor).
    let y = 0.2126 * m[0] + 0.7152 * m[1] + 0.0722 * m[2];
    let d = [[m[0] / y, 0.0, 0.0], [0.0, m[1] / y, 0.0], [0.0, 0.0, m[2] / y]];
    matmul(&SRGB_TO_REC2020, &matmul(&d, &REC2020_TO_SRGB))
}

/// Seletor de balanço de brancos: temperatura/matiz que tornam `sample` (Rec. 2020 linear,
/// antes do balanço) neutro. Procura em grelha e refina.
/// (temperatura, matiz) que fazem `sample` (linear Rec. 2020) ficar com a mesma cor que
/// `target` (normalizado). Com `target` neutro é o mesmo que [`neutralize`].
pub fn match_white(sample: [f32; 3], target: [f32; 3]) -> (f32, f32) {
    let ts = mul3(&REC2020_TO_SRGB, target);
    let tm = (ts[0] + ts[1] + ts[2]) / 3.0;
    if tm <= 0.0 {
        return neutralize(sample);
    }
    let tn = ts.map(|v| v / tm);
    let err = |t: f32, n: f32| {
        let v = mul3(&wb_matrix(t, n), sample);
        let s = mul3(&REC2020_TO_SRGB, v);
        let m = (s[0] + s[1] + s[2]) / 3.0;
        if m <= 0.0 {
            return f32::MAX;
        }
        (0..3).map(|i| (s[i] / m - tn[i]).powi(2)).sum::<f32>()
    };
    let mut best = (0.0, 0.0, f32::MAX);
    for t in (-100..=100).step_by(5) {
        for n in (-100..=100).step_by(5) {
            let e = err(t as f32, n as f32);
            if e < best.2 {
                best = (t as f32, n as f32, e);
            }
        }
    }
    let (t0, n0) = (best.0, best.1);
    for t in -10..=10 {
        for n in -10..=10 {
            let (tt, nn) = (
                (t0 + t as f32 * 0.5).clamp(-100.0, 100.0),
                (n0 + n as f32 * 0.5).clamp(-100.0, 100.0),
            );
            let e = err(tt, nn);
            if e < best.2 {
                best = (tt, nn, e);
            }
        }
    }
    (best.0, best.1)
}

pub fn neutralize(sample: [f32; 3]) -> (f32, f32) {
    let err = |t: f32, n: f32| {
        let v = mul3(&wb_matrix(t, n), sample);
        let s = mul3(&REC2020_TO_SRGB, v);
        let m = (s[0] + s[1] + s[2]) / 3.0;
        if m <= 0.0 {
            return f32::MAX;
        }
        ((s[0] - m).powi(2) + (s[1] - m).powi(2) + (s[2] - m).powi(2)) / (m * m)
    };
    let mut best = (0.0, 0.0, f32::MAX);
    for t in (-100..=100).step_by(5) {
        for n in (-100..=100).step_by(5) {
            let e = err(t as f32, n as f32);
            if e < best.2 {
                best = (t as f32, n as f32, e);
            }
        }
    }
    let (t0, n0) = (best.0, best.1);
    for t in -10..=10 {
        for n in -10..=10 {
            let (tt, nn) = (
                (t0 + t as f32 * 0.5).clamp(-100.0, 100.0),
                (n0 + n as f32 * 0.5).clamp(-100.0, 100.0),
            );
            let e = err(tt, nn);
            if e < best.2 {
                best = (tt, nn, e);
            }
        }
    }
    (best.0, best.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrices_are_inverse_and_keep_white() {
        let m = matmul(&SRGB_TO_REC2020, &REC2020_TO_SRGB);
        for (i, row) in m.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                assert!((v - if i == j { 1.0 } else { 0.0 }).abs() < 1e-3, "{m:?}");
            }
        }
        let w = mul3(&SRGB_TO_REC2020, [1.0, 1.0, 1.0]);
        assert!(w.iter().all(|v| (v - 1.0).abs() < 1e-3));
    }

    #[test]
    fn transfer_roundtrip_is_exact_in_8_bits() {
        let t = Transfer::new();
        for v in 0..=255u8 {
            let lin = eotf(v as f32 / 255.0);
            assert_eq!(t.encode_u8(lin), v);
        }
    }

    #[test]
    fn warmer_means_more_red_less_blue_and_neutralize_inverts() {
        let m = wb_matrix(50.0, 0.0);
        let g = mul3(&REC2020_TO_SRGB, mul3(&m, [0.18, 0.18, 0.18]));
        assert!(g[0] > g[2], "{g:?}");
        // Uma superfície cinzenta fotografada com luz azulada é neutralizada.
        let bluish = mul3(&wb_matrix(-40.0, 15.0), [0.2, 0.2, 0.2]);
        let (t, n) = neutralize(bluish);
        assert!(t > 20.0, "compensa para o quente: {t} {n}");
        let fixed = mul3(&REC2020_TO_SRGB, mul3(&wb_matrix(t, n), bluish));
        let m = (fixed[0] + fixed[1] + fixed[2]) / 3.0;
        assert!(fixed.iter().all(|c| (c - m).abs() / m < 0.02), "neutro: {fixed:?}");
    }
}
