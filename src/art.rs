//! All world artwork, drawn procedurally from simple shapes so no image files are needed.
use little_lanes::map::{idx, Pos, Terrain, DIRS, H, W};
use little_lanes::sim::{CarState, Cell, Sim, DEST_H, DEST_W, PIN_LIMIT};
use macroquad::prelude::*;

use crate::{darker, View, PALETTE, WARN};

// ---- Palette -------------------------------------------------------------

const GRASS: Color = Color::new(0.655, 0.812, 0.478, 1.0);
const GRASS_DARK: Color = Color::new(0.537, 0.729, 0.373, 1.0);
const SAND: Color = Color::new(0.914, 0.867, 0.690, 1.0);
const WATER: Color = Color::new(0.357, 0.655, 0.835, 1.0);
const WATER_LIGHT: Color = Color::new(0.584, 0.800, 0.910, 1.0);
const FOAM: Color = Color::new(0.855, 0.937, 0.969, 1.0);
const HILL: Color = Color::new(0.800, 0.722, 0.553, 1.0);
const HILL_TOP: Color = Color::new(0.878, 0.812, 0.659, 1.0);
const CLIFF: Color = Color::new(0.639, 0.549, 0.388, 1.0);
const ROCK: Color = Color::new(0.604, 0.584, 0.541, 1.0);
const CURB: Color = Color::new(0.886, 0.867, 0.820, 1.0);
const ASPHALT: Color = Color::new(0.333, 0.353, 0.400, 1.0);
const MARKING: Color = Color::new(0.97, 0.95, 0.85, 1.0);
const WALL: Color = Color::new(0.969, 0.949, 0.906, 1.0);
const WINDOW: Color = Color::new(0.651, 0.820, 0.902, 1.0);
const SHADOW: Color = Color::new(0.0, 0.0, 0.0, 0.16);
const TREE_A: Color = Color::new(0.333, 0.604, 0.341, 1.0);
const TREE_B: Color = Color::new(0.427, 0.690, 0.400, 1.0);
const TRUNK: Color = Color::new(0.510, 0.380, 0.259, 1.0);

/// Off-map backdrop colour.
pub const BACKDROP: Color = Color::new(0.553, 0.718, 0.400, 1.0);

fn lighter(c: Color, f: f32) -> Color {
    Color::new(c.r + (1.0 - c.r) * f, c.g + (1.0 - c.g) * f, c.b + (1.0 - c.b) * f, c.a)
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t)
}

fn alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

/// Stable pseudo-random value in [0, 1) for a tile and a salt.
fn hash(x: i32, y: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ salt.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 65536.0
}

// ---- Drawing helpers in tile units ------------------------------------------

fn rect(v: &View, x: f32, y: f32, w: f32, h: f32, c: Color) {
    let p = v.pt(x, y);
    draw_rectangle(p.x, p.y, w * v.ts, h * v.ts, c);
}

fn circle(v: &View, x: f32, y: f32, r: f32, c: Color) {
    let p = v.pt(x, y);
    draw_circle(p.x, p.y, r * v.ts, c);
}

fn line(v: &View, x1: f32, y1: f32, x2: f32, y2: f32, w: f32, c: Color) {
    let (a, b) = (v.pt(x1, y1), v.pt(x2, y2));
    draw_line(a.x, a.y, b.x, b.y, w * v.ts, c);
}

/// Rectangle centred at (x, y) in tile units, rotated by `rot` radians.
fn rot_rect(v: &View, x: f32, y: f32, w: f32, h: f32, rot: f32, c: Color) {
    let p = v.pt(x, y);
    draw_rectangle_ex(
        p.x,
        p.y,
        w * v.ts,
        h * v.ts,
        DrawRectangleParams { offset: vec2(0.5, 0.5), rotation: rot, color: c },
    );
}

/// Rounded rectangle (corner radius r) in tile units.
fn rrect(v: &View, x: f32, y: f32, w: f32, h: f32, r: f32, c: Color) {
    rect(v, x + r, y, w - 2.0 * r, h, c);
    rect(v, x, y + r, w, h - 2.0 * r, c);
    circle(v, x + r, y + r, r, c);
    circle(v, x + w - r, y + r, r, c);
    circle(v, x + r, y + h - r, r, c);
    circle(v, x + w - r, y + h - r, r, c);
}

fn terrain(s: &Sim, p: Pos) -> Option<Terrain> {
    (p.0 >= 0 && p.1 >= 0 && p.0 < W && p.1 < H).then(|| s.terrain[idx(p)])
}

// ---- World ------------------------------------------------------------------

pub fn draw_world(s: &Sim, v: &View, time: f32) {
    draw_ground(s, v, time);
    draw_roads(s, v);
    for (i, d) in s.dests.iter().enumerate() {
        draw_dest(s, v, i, d.origin, PALETTE[d.color]);
    }
    for h in s.homes.iter() {
        draw_home(v, h.pos, PALETTE[h.color]);
    }
    draw_trees(s, v);
    draw_cars(s, v);
    for (i, d) in s.dests.iter().enumerate() {
        draw_demand(s, v, i, d.origin, time);
    }
}

fn draw_ground(s: &Sim, v: &View, time: f32) {
    // Base layer
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32, y as f32);
            match s.terrain[idx((x, y))] {
                Terrain::Grass => {
                    let g = mix(GRASS, GRASS_DARK, hash(x, y, 1) * 0.25);
                    rect(v, fx, fy, 1.02, 1.02, g);
                    // Grass tufts and the odd flower
                    for k in 0..3 {
                        let (hx, hy) = (hash(x, y, 10 + k), hash(x, y, 20 + k));
                        if hash(x, y, 30 + k) < 0.55 {
                            let (px, py) = (fx + 0.15 + hx * 0.7, fy + 0.15 + hy * 0.7);
                            line(v, px - 0.04, py, px - 0.07, py - 0.08, 0.025, GRASS_DARK);
                            line(v, px + 0.02, py, px + 0.04, py - 0.09, 0.025, GRASS_DARK);
                        }
                    }
                    if hash(x, y, 40) < 0.06 {
                        let fc = if hash(x, y, 41) < 0.5 { WHITE } else { Color::new(1.0, 0.85, 0.35, 1.0) };
                        circle(v, fx + 0.3 + hash(x, y, 42) * 0.4, fy + 0.3 + hash(x, y, 43) * 0.4, 0.05, fc);
                    }
                }
                Terrain::Water => {
                    let c = mix(WATER, WATER_LIGHT, hash(x, y, 2) * 0.15);
                    rect(v, fx, fy, 1.02, 1.02, c);
                }
                Terrain::Hill => {
                    rect(v, fx, fy, 1.02, 1.02, HILL);
                }
            }
        }
    }

    // Edge details: beaches, foam, ripples, hill tops and cliffs.
    for y in 0..H {
        for x in 0..W {
            let (fx, fy) = (x as f32, y as f32);
            let t = s.terrain[idx((x, y))];
            for (k, d) in DIRS.iter().enumerate() {
                let n = terrain(s, (x + d.0, y + d.1));
                let strip = |w: f32, c: Color| match k {
                    0 => rect(v, fx + 1.0 - w, fy, w, 1.0, c),
                    1 => rect(v, fx, fy + 1.0 - w, 1.0, w, c),
                    2 => rect(v, fx, fy, w, 1.0, c),
                    _ => rect(v, fx, fy, 1.0, w, c),
                };
                match t {
                    Terrain::Grass if n == Some(Terrain::Water) => strip(0.16, SAND),
                    Terrain::Water if n.is_some() && n != Some(Terrain::Water) => strip(0.08, FOAM),
                    Terrain::Hill if n.is_some() && n != Some(Terrain::Hill) => match k {
                        1 => strip(0.24, CLIFF),
                        3 => strip(0.10, HILL_TOP),
                        _ => strip(0.06, darker(HILL, 0.92)),
                    },
                    _ => {}
                }
            }
            match t {
                Terrain::Water => {
                    if hash(x, y, 3) < 0.35 {
                        let phase = hash(x, y, 4) * 6.28;
                        let ox = (time * 0.8 + phase).sin() * 0.08;
                        let (px, py) = (fx + 0.3 + hash(x, y, 5) * 0.3 + ox, fy + 0.3 + hash(x, y, 6) * 0.4);
                        line(v, px, py, px + 0.22, py, 0.04, alpha(FOAM, 0.55));
                    }
                }
                Terrain::Hill => {
                    if hash(x, y, 7) < 0.45 {
                        let (px, py) = (fx + 0.25 + hash(x, y, 8) * 0.5, fy + 0.25 + hash(x, y, 9) * 0.4);
                        let r = 0.07 + hash(x, y, 11) * 0.06;
                        circle(v, px + 0.02, py + 0.03, r, alpha(CLIFF, 0.6));
                        circle(v, px, py, r, ROCK);
                        circle(v, px - r * 0.3, py - r * 0.3, r * 0.4, lighter(ROCK, 0.35));
                    }
                }
                _ => {}
            }
        }
    }
}

fn connects(s: &Sim, from: Cell, to: Pos) -> bool {
    match s.cell(to) {
        Cell::Road => true,
        Cell::Home(_) | Cell::Door(_) => from == Cell::Road,
        Cell::Dest(id) => matches!(from, Cell::Door(d) if d == id),
        Cell::Empty => false,
    }
}

fn draw_roads(s: &Sim, v: &View) {
    // Three passes so curbs never cover neighbouring asphalt.
    for pass in 0..3 {
        for y in 0..H {
            for x in 0..W {
                let c = s.cells[idx((x, y))];
                if !matches!(c, Cell::Road | Cell::Door(_)) {
                    continue;
                }
                let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                let links: Vec<usize> = (0..4).filter(|k| connects(s, c, (x + DIRS[*k].0, y + DIRS[*k].1))).collect();
                match pass {
                    0 => {
                        circle(v, cx, cy, 0.37, CURB);
                        for k in &links {
                            let d = DIRS[*k];
                            line(v, cx, cy, cx + d.0 as f32 * 0.5, cy + d.1 as f32 * 0.5, 0.74, CURB);
                        }
                    }
                    1 => {
                        circle(v, cx, cy, 0.3, ASPHALT);
                        for k in &links {
                            let d = DIRS[*k];
                            line(v, cx, cy, cx + d.0 as f32 * 0.5, cy + d.1 as f32 * 0.5, 0.6, ASPHALT);
                        }
                    }
                    _ => {
                        if let Cell::Door(_) = c {
                            // Parking bays
                            for i in 0..3 {
                                let lx = cx - 0.2 + i as f32 * 0.2;
                                line(v, lx, cy - 0.05, lx, cy + 0.25, 0.03, alpha(MARKING, 0.8));
                            }
                            continue;
                        }
                        let straight_h = links.len() == 2 && links.contains(&0) && links.contains(&2);
                        let straight_v = links.len() == 2 && links.contains(&1) && links.contains(&3);
                        if straight_h {
                            line(v, cx - 0.32, cy, cx - 0.12, cy, 0.04, MARKING);
                            line(v, cx + 0.12, cy, cx + 0.32, cy, 0.04, MARKING);
                        } else if straight_v {
                            line(v, cx, cy - 0.32, cx, cy - 0.12, 0.04, MARKING);
                            line(v, cx, cy + 0.12, cx, cy + 0.32, 0.04, MARKING);
                        }
                    }
                }
            }
        }
    }
}

fn draw_home(v: &View, p: Pos, col: Color) {
    let (x, y) = (p.0 as f32, p.1 as f32);
    // Shadow
    rrect(v, x + 0.14, y + 0.14, 0.84, 0.82, 0.08, SHADOW);
    // Front wall with door and windows (3/4 view)
    rect(v, x + 0.12, y + 0.54, 0.76, 0.36, WALL);
    rect(v, x + 0.12, y + 0.86, 0.76, 0.04, darker(WALL, 0.82));
    rect(v, x + 0.43, y + 0.64, 0.14, 0.24, darker(col, 0.55));
    rect(v, x + 0.2, y + 0.63, 0.15, 0.12, WINDOW);
    rect(v, x + 0.65, y + 0.63, 0.15, 0.12, WINDOW);
    // Pitched roof: light back slope, darker front slope, ridge and eaves
    rect(v, x + 0.06, y + 0.08, 0.88, 0.24, lighter(col, 0.12));
    rect(v, x + 0.06, y + 0.32, 0.88, 0.24, darker(col, 0.84));
    rect(v, x + 0.06, y + 0.53, 0.88, 0.04, darker(col, 0.62));
    line(v, x + 0.06, y + 0.32, x + 0.94, y + 0.32, 0.035, lighter(col, 0.45));
    // Chimney
    rect(v, x + 0.7, y + 0.05, 0.1, 0.16, darker(col, 0.6));
    rect(v, x + 0.68, y + 0.04, 0.14, 0.04, darker(col, 0.5));
}

fn draw_dest(s: &Sim, v: &View, _i: usize, o: Pos, col: Color) {
    let (x, y) = (o.0 as f32 + 0.06, o.1 as f32 + 0.06);
    let (w, h) = (DEST_W as f32 - 0.12, DEST_H as f32 - 0.12);
    let roof_h = h * 0.62;
    // Shadow
    rrect(v, x + 0.1, y + 0.12, w, h, 0.08, SHADOW);
    // Flat roof with rim and rooftop units
    rrect(v, x, y, w, roof_h, 0.08, darker(col, 0.8));
    rrect(v, x + 0.06, y + 0.06, w - 0.12, roof_h - 0.12, 0.06, lighter(col, 0.12));
    rect(v, x + 0.2, y + 0.18, 0.32, 0.22, Color::new(0.82, 0.82, 0.84, 1.0));
    rect(v, x + 0.24, y + 0.22, 0.24, 0.04, Color::new(0.66, 0.66, 0.7, 1.0));
    rect(v, x + 0.24, y + 0.3, 0.24, 0.04, Color::new(0.66, 0.66, 0.7, 1.0));
    circle(v, x + w - 0.35, y + 0.32, 0.13, Color::new(0.82, 0.82, 0.84, 1.0));
    circle(v, x + w - 0.35, y + 0.32, 0.07, Color::new(0.66, 0.66, 0.7, 1.0));
    // Facade
    let fy = y + roof_h;
    let fh = h - roof_h;
    rect(v, x, fy, w, fh, WALL);
    // Striped awning
    let stripes = 10;
    for k in 0..stripes {
        let c = if k % 2 == 0 { col } else { WHITE };
        rect(v, x + k as f32 * w / stripes as f32, fy, w / stripes as f32 + 0.01, 0.16, c);
    }
    rect(v, x, fy + 0.16, w, 0.03, alpha(darker(col, 0.6), 0.5));
    // Shop windows either side of the doorway
    rect(v, x + 0.15, fy + 0.27, 0.75, fh - 0.36, WINDOW);
    rect(v, x + w - 0.9, fy + 0.27, 0.75, fh - 0.36, WINDOW);
    rect(v, x + w / 2.0 - 0.2, fy + 0.25, 0.4, fh - 0.25, darker(col, 0.55));
    // Base line
    rect(v, x, y + h - 0.04, w, 0.04, darker(WALL, 0.8));
    let _ = s;
}

fn draw_demand(s: &Sim, v: &View, i: usize, o: Pos, time: f32) {
    let d = &s.dests[i];
    let col = PALETTE[d.color];
    // Demand pins sit on the roof as white badges; hollow once a car is on its way.
    let n = d.pins.min(12);
    for k in 0..n {
        let (cx, cy) = (k % 6, k / 6);
        let px = o.0 as f32 + 0.42 + cx as f32 * 0.43;
        let py = o.1 as f32 + 0.34 + cy as f32 * 0.4;
        circle(v, px + 0.03, py + 0.04, 0.13, SHADOW);
        if k < d.assigned {
            circle(v, px, py, 0.13, WHITE);
            circle(v, px, py, 0.08, lighter(col, 0.5));
        } else {
            circle(v, px, py, 0.13, WHITE);
            circle(v, px, py, 0.07, darker(col, 0.85));
        }
    }
    if d.overflow > 0.0 || d.pins > PIN_LIMIT {
        let c = v.pt(o.0 as f32 + DEST_W as f32 / 2.0, o.1 as f32 + DEST_H as f32 / 2.0);
        let rad = v.ts * 2.1;
        draw_circle_lines(c.x, c.y, rad, 7.0, alpha(darker(WARN, 0.5), 0.25));
        let pulse = if s.failed_dest == Some(i) { 1.0 } else { 0.75 + 0.25 * (time * 6.0).sin() };
        draw_arc(c.x, c.y, 48, rad - 3.5, -90.0, 7.0, 360.0 * d.overflow.min(1.0), alpha(WARN, pulse));
    }
}

fn draw_trees(s: &Sim, v: &View) {
    for y in 0..H {
        for x in 0..W {
            if s.terrain[idx((x, y))] != Terrain::Grass || s.cells[idx((x, y))] != Cell::Empty {
                continue;
            }
            if hash(x, y, 50) > 0.09 {
                continue;
            }
            let (cx, cy) = (x as f32 + 0.3 + hash(x, y, 51) * 0.4, y as f32 + 0.3 + hash(x, y, 52) * 0.35);
            let r = 0.2 + hash(x, y, 53) * 0.08;
            circle(v, cx + 0.08, cy + 0.12, r, SHADOW);
            rect(v, cx - 0.03, cy, 0.06, 0.16, TRUNK);
            circle(v, cx, cy - 0.02, r, TREE_A);
            circle(v, cx - r * 0.3, cy - r * 0.35, r * 0.6, TREE_B);
            circle(v, cx - r * 0.45, cy - r * 0.5, r * 0.18, lighter(TREE_B, 0.3));
        }
    }
}

fn draw_cars(s: &Sim, v: &View) {
    for (i, c) in s.cars.iter().enumerate() {
        if c.state == CarState::Idle {
            continue;
        }
        let col = PALETTE[s.homes[i].color];
        let (ax, ay) = (c.prev.0 as f32, c.prev.1 as f32);
        let (bx, by) = (c.cur.0 as f32, c.cur.1 as f32);
        let t = c.progress;
        let (x, y) = (ax + (bx - ax) * t, ay + (by - ay) * t);
        let mut dir = vec2(bx - ax, by - ay);
        if dir.length_squared() < 0.01 {
            dir = c
                .path
                .get(c.i + 1)
                .map(|n| vec2((n.0 - c.cur.0) as f32, (n.1 - c.cur.1) as f32))
                .unwrap_or(vec2(1.0, 0.0));
        }
        let dir = dir.normalize_or_zero();
        let normal = vec2(-dir.y, dir.x);
        let ctr = vec2(x + 0.5, y + 0.5) + normal * 0.15;
        let _ = i;
        let rot = dir.y.atan2(dir.x);
        let (len, wid) = (0.5, 0.28);
        // Shadow, body, cabin, windscreen, rear window
        rot_rect(v, ctr.x + 0.03, ctr.y + 0.05, len, wid, rot, SHADOW);
        rot_rect(v, ctr.x, ctr.y, len, wid, rot, darker(col, 0.92));
        rot_rect(v, ctr.x - dir.x * 0.02, ctr.y - dir.y * 0.02, len * 0.5, wid * 0.78, rot, lighter(col, 0.25));
        let f = ctr + dir * 0.1;
        rot_rect(v, f.x, f.y, 0.05, wid * 0.7, rot, Color::new(0.2, 0.25, 0.32, 1.0));
        let r = ctr - dir * 0.12;
        rot_rect(v, r.x, r.y, 0.04, wid * 0.7, rot, Color::new(0.2, 0.25, 0.32, 0.8));
        // Headlights
        let hl = ctr + dir * 0.2;
        for side in [-1.0, 1.0] {
            let p = hl + normal * 0.08 * side;
            circle(v, p.x, p.y, 0.025, Color::new(1.0, 0.97, 0.8, 1.0));
        }
    }
}
