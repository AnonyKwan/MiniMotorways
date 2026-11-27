use crate::rng::Rng;

pub const W: i32 = 40;
pub const H: i32 = 24;

pub type Pos = (i32, i32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Terrain {
    Grass,
    Water,
    Hill,
}

pub fn in_bounds(p: Pos) -> bool {
    p.0 >= 0 && p.1 >= 0 && p.0 < W && p.1 < H
}

pub fn idx(p: Pos) -> usize {
    (p.1 * W + p.0) as usize
}

pub const DIRS: [Pos; 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// Index into DIRS for the step from `a` to the neighbouring tile `b`.
pub fn dir_index(a: Pos, b: Pos) -> usize {
    match (b.0 - a.0, b.1 - a.1) {
        (1, 0) => 0,
        (0, 1) => 1,
        (-1, 0) => 2,
        _ => 3,
    }
}

/// "Bayside": a winding river down the east side and a few hills.
pub fn generate(seed: u64) -> Vec<Terrain> {
    let mut rng = Rng::new(seed);
    let mut t = vec![Terrain::Grass; (W * H) as usize];

    let phase = rng.range_f32(0.0, 6.28);
    let base = W as f32 * rng.range_f32(0.62, 0.72);
    for y in 0..H {
        let cx = base + ((y as f32) * 0.28 + phase).sin() * 3.0;
        for x in 0..W {
            let d = x as f32 - cx;
            if (-1.0..1.0).contains(&d) {
                t[idx((x, y))] = Terrain::Water;
            }
        }
    }

    for _ in 0..5 {
        let hx = rng.range_f32(3.0, W as f32 - 3.0);
        let hy = rng.range_f32(3.0, H as f32 - 3.0);
        let r = rng.range_f32(1.2, 2.4);
        for y in 0..H {
            for x in 0..W {
                let (dx, dy) = (x as f32 + 0.5 - hx, y as f32 + 0.5 - hy);
                if dx * dx + dy * dy < r * r && t[idx((x, y))] == Terrain::Grass {
                    t[idx((x, y))] = Terrain::Hill;
                }
            }
        }
    }
    t
}
