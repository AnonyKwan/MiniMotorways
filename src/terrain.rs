//! Procedural terrain: water, mountains and trees, shaped by each city's design numbers.
use crate::cities::City;
use crate::rng::Rng;

pub const LAND: u8 = 0;
pub const WATER: u8 = 1;
pub const MOUNTAIN: u8 = 2;

#[derive(Clone, Debug)]
pub struct Terrain {
    pub w: i32,
    pub h: i32,
    pub kind: Vec<u8>,
    pub tree: Vec<bool>,
}

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(2_246_822_519));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 16_777_216.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = smooth(x - x0);
    let fy = smooth(y - y0);
    let (ix, iy) = (x0 as i32, y0 as i32);
    let a = hash(ix, iy, seed);
    let b = hash(ix + 1, iy, seed);
    let c = hash(ix, iy + 1, seed);
    let d = hash(ix + 1, iy + 1, seed);
    let top = a + (b - a) * fx;
    let bot = c + (d - c) * fx;
    top + (bot - top) * fy
}

fn field(x: f32, y: f32, seed: u32) -> f32 {
    0.6 * value_noise(x / 7.0, y / 7.0, seed)
        + 0.3 * value_noise(x / 3.5, y / 3.5, seed.wrapping_add(17))
        + 0.1 * value_noise(x / 1.7, y / 1.7, seed.wrapping_add(31))
}

/// Share of the map covered, from a raw tile count in the design tables.
fn fraction(tiles: u32, cap: f32) -> f32 {
    if tiles == 0 {
        return 0.0;
    }
    let r = (tiles as f32 / 6000.0).min(1.0);
    0.04 + (cap - 0.04) * r.powf(0.6)
}

/// Marks the `frac` lowest-valued land cells as `mark`.
fn carve(kind: &mut [u8], values: &[f32], frac: f32, mark: u8) {
    let mut cand: Vec<usize> = (0..kind.len()).filter(|&i| kind[i] == LAND).collect();
    cand.sort_by(|&a, &b| values[a].partial_cmp(&values[b]).unwrap_or(std::cmp::Ordering::Equal));
    let n = (cand.len() as f32 * frac) as usize;
    for &i in cand.iter().take(n) {
        kind[i] = mark;
    }
}

impl Terrain {
    pub fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.w + x) as usize
    }

    pub fn generate(city: &City, seed: u32, rng: &mut Rng) -> Terrain {
        let (w, h) = (city.w, city.h);
        let n = (w * h) as usize;
        let mut kind = vec![LAND; n];
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        // Keeps the middle of the map dry so the first buildings always have room.
        let centre_bias = |x: i32, y: i32| -> f32 {
            let dx = (x as f32 + 0.5 - cx) / cx;
            let dy = (y as f32 + 0.5 - cy) / cy;
            let d = (dx * dx + dy * dy).sqrt().min(1.4);
            0.35 * (1.0 - d / 1.4)
        };
        let mut wv = vec![0.0f32; n];
        let mut mv = vec![0.0f32; n];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                let bias = centre_bias(x, y);
                wv[i] = field(x as f32, y as f32, seed) + bias;
                mv[i] = field(x as f32 + 91.0, y as f32 - 47.0, seed.wrapping_add(7)) + bias;
            }
        }
        carve(&mut kind, &wv, fraction(city.water, 0.32), WATER);
        carve(&mut kind, &mv, fraction(city.mountain, 0.22), MOUNTAIN);
        let mut tree = vec![false; n];
        let want = city.trees.clamp(12, 90);
        let mut placed = 0;
        let mut tries = 0;
        while placed < want && tries < 4000 {
            tries += 1;
            let i = rng.below(n as u32) as usize;
            if kind[i] == LAND && !tree[i] {
                tree[i] = true;
                placed += 1;
            }
        }
        Terrain { w, h, kind, tree }
    }
}
