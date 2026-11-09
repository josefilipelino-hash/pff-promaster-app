//! Hashes percetuais: pHash (DCT 32×32 → 64 bits) e dHash (gradientes 9×8 → 64 bits).

use image::{imageops, RgbImage};

pub fn phash(img: &RgbImage) -> u64 {
    let g = imageops::grayscale(&imageops::resize(img, 32, 32, imageops::FilterType::Triangle));
    let px: Vec<f64> = g.as_raw().iter().map(|&v| v as f64).collect();
    // DCT-II 2D, apenas os coeficientes 8×8 de baixa frequência.
    let cos: Vec<Vec<f64>> = (0..8)
        .map(|u| {
            (0..32)
                .map(|x| ((2 * x + 1) as f64 * u as f64 * std::f64::consts::PI / 64.0).cos())
                .collect()
        })
        .collect();
    let mut coeffs = [0f64; 64];
    for v in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for y in 0..32 {
                for x in 0..32 {
                    s += px[y * 32 + x] * cos[u][x] * cos[v][y];
                }
            }
            coeffs[v * 8 + u] = s;
        }
    }
    let mut sorted: Vec<f64> = coeffs[1..].to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let median = sorted[sorted.len() / 2];
    coeffs
        .iter()
        .enumerate()
        .fold(0u64, |h, (i, &c)| if i > 0 && c > median { h | (1 << i) } else { h })
}

pub fn dhash(img: &RgbImage) -> u64 {
    let g = imageops::grayscale(&imageops::resize(img, 9, 8, imageops::FilterType::Triangle));
    let mut h = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            if g.get_pixel(x, y).0[0] > g.get_pixel(x + 1, y).0[0] {
                h |= 1 << (y * 8 + x);
            }
        }
    }
    h
}

pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;
    use momento_test_fixtures::test_image;

    #[test]
    fn similar_images_have_close_hashes() {
        let a = test_image(800, 600, 3, 1);
        let shifted = imageops::crop_imm(&a, 10, 8, 780, 585).to_image();
        let darker = RgbImage::from_fn(a.width(), a.height(), |x, y| {
            let p = a.get_pixel(x, y).0;
            image::Rgb([p[0] / 2 + 20, p[1] / 2 + 20, p[2] / 2 + 20])
        });
        let other = test_image(800, 600, 40, 3);
        assert!(
            hamming(phash(&a), phash(&shifted)) <= 8,
            "{}",
            hamming(phash(&a), phash(&shifted))
        );
        assert!(hamming(phash(&a), phash(&darker)) <= 8);
        assert!(hamming(phash(&a), phash(&other)) > 14, "{}", hamming(phash(&a), phash(&other)));
        assert!(hamming(dhash(&a), dhash(&shifted)) <= 10);
    }
}
