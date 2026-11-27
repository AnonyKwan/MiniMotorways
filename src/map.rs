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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapKind {
    /// A winding river down the east side and a few hills.
    Bayside,
    /// Two land masses split by a channel, with a lake. Bridges matter.
    TwinIslands,
    /// Lots of hills and a narrow stream. Tunnels matter.
    Highland,
}

impl MapKind {
    pub const ALL: [MapKind; 3] = [MapKind::Bayside, MapKind::TwinIslands, MapKind::Highland];

    pub fn name(self) -> &'static str {
        match self {
            MapKind::Bayside => "Bayside",
            MapKind::TwinIslands => "Twin Islands",
            MapKind::Highland => "Highland",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            MapKind::Bayside => "A calm town by a winding river",
            MapKind::TwinIslands => "Two shores, one channel. Bridges matter",
            MapKind::Highland => "Rocky hills everywhere. Tunnels matter",
        }
    }

    /// Multiplier on time between pins: below 1 makes a map busier and harder.
    pub fn difficulty(self) -> f32 {
        match self {
            MapKind::Bayside => 1.0,
            MapKind::TwinIslands => 0.9,
            MapKind::Highland => 0.95,
        }
    }

    pub fn index(self) -> usize {
        MapKind::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

fn blob(t: &mut [Terrain], cx: f32, cy: f32, r: f32, kind: Terrain, only_on: Terrain) {
    for y in 0..H {
        for x in 0..W {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if dx * dx + dy * dy < r * r && t[idx((x, y))] == only_on {
                t[idx((x, y))] = kind;
            }
        }
    }
}

/// Bayside terrain (kept for tests and callers that don't pick a map).
pub fn generate(seed: u64) -> Vec<Terrain> {
    generate_kind(MapKind::Bayside, seed)
}

pub fn generate_kind(kind: MapKind, seed: u64) -> Vec<Terrain> {
    let mut rng = Rng::new(seed);
    let mut t = vec![Terrain::Grass; (W * H) as usize];
    let phase = rng.range_f32(0.0, 6.28);
    match kind {
        MapKind::Bayside => {
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
                let (hx, hy) = (rng.range_f32(3.0, W as f32 - 3.0), rng.range_f32(3.0, H as f32 - 3.0));
                let r = rng.range_f32(1.2, 2.4);
                blob(&mut t, hx, hy, r, Terrain::Hill, Terrain::Grass);
            }
        }
        MapKind::TwinIslands => {
            let base = H as f32 * 0.5;
            for x in 0..W {
                let cy = base + ((x as f32) * 0.22 + phase).sin() * 2.5;
                for y in 0..H {
                    let d = y as f32 - cy;
                    if (-1.5..1.5).contains(&d) {
                        t[idx((x, y))] = Terrain::Water;
                    }
                }
            }
            // A lake on one shore
            let lx = rng.range_f32(8.0, W as f32 - 8.0);
            let ly = if rng.f32() < 0.5 { 4.5 } else { H as f32 - 4.5 };
            blob(&mut t, lx, ly, 2.2, Terrain::Water, Terrain::Grass);
            for _ in 0..3 {
                let (hx, hy) = (rng.range_f32(3.0, W as f32 - 3.0), rng.range_f32(3.0, H as f32 - 3.0));
                blob(&mut t, hx, hy, rng.range_f32(1.2, 2.0), Terrain::Hill, Terrain::Grass);
            }
        }
        MapKind::Highland => {
            let base = W as f32 * rng.range_f32(0.25, 0.35);
            for y in 0..H {
                let cx = base + ((y as f32) * 0.35 + phase).sin() * 2.0;
                for x in 0..W {
                    let d = x as f32 - cx;
                    if (-0.5..0.5).contains(&d) {
                        t[idx((x, y))] = Terrain::Water;
                    }
                }
            }
            for _ in 0..13 {
                let (hx, hy) = (rng.range_f32(2.0, W as f32 - 2.0), rng.range_f32(2.0, H as f32 - 2.0));
                blob(&mut t, hx, hy, rng.range_f32(1.4, 3.0), Terrain::Hill, Terrain::Grass);
            }
        }
    }
    t
}
