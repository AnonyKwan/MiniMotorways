//! 3D renderer: a tilted perspective camera over a little diorama board.
//! Everything is built from flat-shaded low-poly meshes generated each frame, so no model
//! or image files are needed. Lighting is baked per face from a single sun direction.
use little_lanes::map::{idx, Pos, Terrain, DIRS, H, W};
use little_lanes::sim::{Car, CarState, Cell, Dest, Junction, Sim};
use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;

use crate::{PALETTE, WARN};

// ---- Palette -------------------------------------------------------------------

const GRASS: Color = Color::new(0.62, 0.80, 0.47, 1.0);
const SAND: Color = Color::new(0.93, 0.87, 0.68, 1.0);
const WATER: Color = Color::new(0.38, 0.68, 0.86, 1.0);
const WATER_DEEP: Color = Color::new(0.27, 0.53, 0.72, 1.0);
const FOAM: Color = Color::new(0.88, 0.95, 0.98, 1.0);
const HILL: Color = Color::new(0.80, 0.73, 0.57, 1.0);
const CLIFF: Color = Color::new(0.70, 0.60, 0.45, 1.0);
const ROCK: Color = Color::new(0.66, 0.65, 0.62, 1.0);
const CURB: Color = Color::new(0.90, 0.89, 0.85, 1.0);
const ASPHALT: Color = Color::new(0.34, 0.36, 0.41, 1.0);
const MARKING: Color = Color::new(0.98, 0.96, 0.86, 1.0);
const WALL: Color = Color::new(0.97, 0.95, 0.90, 1.0);
const WINDOW: Color = Color::new(0.62, 0.80, 0.90, 1.0);
const TRUNK: Color = Color::new(0.52, 0.39, 0.27, 1.0);
const LEAF_A: Color = Color::new(0.33, 0.60, 0.34, 1.0);
const LEAF_B: Color = Color::new(0.44, 0.69, 0.38, 1.0);
const PINE: Color = Color::new(0.25, 0.50, 0.36, 1.0);
const SHADOW: Color = Color::new(0.10, 0.14, 0.10, 0.22);
pub const SKY: Color = Color::new(0.84, 0.91, 0.92, 1.0);

const GROUND_Y: f32 = 0.0;
const WATER_Y: f32 = -0.26;
const HILL_Y: f32 = 0.62;
const BASE_Y: f32 = -1.1;

fn lighter(c: Color, f: f32) -> Color {
    Color::new(c.r + (1.0 - c.r) * f, c.g + (1.0 - c.g) * f, c.b + (1.0 - c.b) * f, c.a)
}
fn darker(c: Color, f: f32) -> Color {
    Color::new(c.r * f, c.g * f, c.b * f, c.a)
}
fn alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

fn hash(x: i32, y: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ salt.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 65536.0
}

fn noise(x: f32, y: f32, scale: f32, salt: u32) -> f32 {
    let (fx, fy) = (x / scale, y / scale);
    let (ix, iy) = (fx.floor() as i32, fy.floor() as i32);
    let (tx, ty) = (fx - ix as f32, fy - iy as f32);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let a = hash(ix, iy, salt) + (hash(ix + 1, iy, salt) - hash(ix, iy, salt)) * sx;
    let b = hash(ix, iy + 1, salt) + (hash(ix + 1, iy + 1, salt) - hash(ix, iy + 1, salt)) * sx;
    a + (b - a) * sy
}

// ---- Camera --------------------------------------------------------------------

pub struct Scene {
    pub cam: Camera3D,
    vp: Mat4,
}

impl Scene {
    /// Camera that fits the whole board between the top bar and the bottom inset, then zooms by `zoom` (1 = whole board)
    /// and shifts by `pan` tiles.
    pub fn new(hud_h: f32, bottom: f32, zoom: f32, pan: Vec2) -> Scene {
        let (sw, sh) = (screen_width(), screen_height());
        // A gentle tilt shows the sides of buildings; a narrow lens keeps the board close to
        // its true shape. Tall phone screens turn the board so its long side runs up the screen.
        let fovy: f32 = 24f32.to_radians();
        let pitch: f32 = 58f32.to_radians();
        let portrait = sh > sw * 1.05;
        let back = if portrait { vec3(1.0, 0.0, 0.0) } else { vec3(0.0, 0.0, 1.0) };
        let margin = 10.0;
        let mut corners = Vec::new();
        for y in [0.0, BASE_Y] {
            for (x, z) in [(0.0, 0.0), (W as f32, 0.0), (0.0, H as f32), (W as f32, H as f32)] {
                corners.push(vec3(x, y, z));
            }
        }
        let build = |target: Vec3, dist: f32| {
            let position = target + (Vec3::Y * pitch.sin() + back * pitch.cos()) * dist;
            let cam = Camera3D {
                position,
                target,
                up: Vec3::Y,
                fovy,
                z_near: 1.0,
                z_far: dist * 4.0,
                ..Default::default()
            };
            let vp = cam.matrix();
            Scene { cam, vp }
        };
        let bbox = |sc: &Scene| {
            let pts: Vec<Vec2> = corners.iter().map(|c| sc.to_screen(*c)).collect();
            let min = pts.iter().fold(vec2(f32::MAX, f32::MAX), |m, p| m.min(*p));
            let max = pts.iter().fold(vec2(f32::MIN, f32::MIN), |m, p| m.max(*p));
            (min, max)
        };
        // Find the closest camera distance at which the whole board fits below the top bar,
        // then nudge the target so the board is centred in that space.
        let mut target = vec3(W as f32 * 0.5, 0.0, H as f32 * 0.5);
        let avail_w = sw - 2.0 * margin;
        let avail_h = sh - hud_h - bottom - 2.0 * margin;
        let mut sc = build(target, 40.0);
        let mut fit = 40.0;
        for _ in 0..3 {
            let (mut lo, mut hi) = (5.0f32, 400.0f32);
            for _ in 0..30 {
                let mid = (lo + hi) * 0.5;
                let (min, max) = bbox(&build(target, mid));
                if max.x - min.x <= avail_w && max.y - min.y <= avail_h {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            sc = build(target, hi);
            fit = hi;
            let (min, max) = bbox(&sc);
            let want = vec2(sw * 0.5, hud_h + margin + avail_h * 0.5);
            let have = (min + max) * 0.5;
            // Convert the screen offset into a ground offset at the board centre.
            let g0 = sc.ground_at(have.x, have.y, 0.0);
            let g1 = sc.ground_at(want.x, want.y, 0.0);
            if let (Some(g0), Some(g1)) = (g0, g1) {
                target += vec3(g0.x - g1.x, 0.0, g0.y - g1.y);
            }
        }
        if zoom == 1.0 && pan == Vec2::ZERO {
            return sc;
        }
        build(target + vec3(pan.x, 0.0, pan.y), fit / zoom)
    }

    pub fn to_screen(&self, p: Vec3) -> Vec2 {
        let c = self.vp * p.extend(1.0);
        let n = c.truncate() / c.w;
        vec2((n.x + 1.0) * 0.5 * screen_width(), (1.0 - n.y) * 0.5 * screen_height())
    }

    /// Point on a horizontal plane at height `y` under the given screen position.
    pub fn ground_at(&self, sx: f32, sy: f32, y: f32) -> Option<Vec2> {
        let nx = sx / screen_width() * 2.0 - 1.0;
        let ny = 1.0 - sy / screen_height() * 2.0;
        let inv = self.vp.inverse();
        let a = inv.project_point3(vec3(nx, ny, -1.0));
        let b = inv.project_point3(vec3(nx, ny, 1.0));
        if (b.y - a.y).abs() < 1e-6 {
            return None;
        }
        let t = (y - a.y) / (b.y - a.y);
        let p = a + (b - a) * t;
        Some(vec2(p.x, p.z))
    }

    /// Tile under a screen point, taking raised hill tiles into account.
    pub fn pick_on(&self, s: &Sim, sx: f32, sy: f32) -> Option<Pos> {
        if let Some(g) = self.ground_at(sx, sy, HILL_Y) {
            let p = (g.x.floor() as i32, g.y.floor() as i32);
            if terrain_at(s, p.0, p.1) == Some(Terrain::Hill) {
                return Some(p);
            }
        }
        self.pick(sx, sy)
    }

    pub fn pick(&self, sx: f32, sy: f32) -> Option<Pos> {
        let g = self.ground_at(sx, sy, GROUND_Y)?;
        let p = (g.x.floor() as i32, g.y.floor() as i32);
        (p.0 >= 0 && p.1 >= 0 && p.0 < W && p.1 < H).then_some(p)
    }
}

// ---- Mesh building -----------------------------------------------------------

/// Sun direction (pointing toward the light).
fn sun() -> Vec3 {
    vec3(-0.45, 1.0, 0.35).normalize()
}

fn shade(c: Color, n: Vec3) -> Color {
    if c.a < 0.99 {
        return c;
    }
    let d = n.dot(sun()).max(0.0);
    let k = 0.42 + 0.58 * d;
    // A touch of cool sky light on faces turned away from the sun.
    let sky = (1.0 - d) * 0.05;
    Color::new((c.r * k + sky * 0.6).min(1.0), (c.g * k + sky * 0.8).min(1.0), (c.b * k + sky).min(1.0), 1.0)
}

pub struct Mb {
    verts: Vec<Vertex>,
    idx: Vec<u16>,
    /// When set, finished meshes are kept here instead of drawn (for the static-world cache).
    keep: Option<Vec<Mesh>>,
}

impl Mb {
    pub fn new() -> Self {
        Mb { verts: Vec::with_capacity(9000), idx: Vec::with_capacity(4400), keep: None }
    }

    pub fn flush(&mut self) {
        if self.idx.is_empty() {
            return;
        }
        let mesh = Mesh {
            vertices: std::mem::take(&mut self.verts),
            indices: std::mem::take(&mut self.idx),
            texture: None,
        };
        match &mut self.keep {
            Some(k) => k.push(mesh),
            None => draw_mesh(&mesh),
        }
        self.verts.reserve(9000);
        self.idx.reserve(4400);
    }

    /// Convex polygon with a known outward normal.
    fn poly_n(&mut self, pts: &[Vec3], n: Vec3, c: Color) {
        if self.idx.len() + (pts.len() - 2) * 3 > 4300 || self.verts.len() + pts.len() > 9000 {
            self.flush();
        }
        let col = shade(c, n);
        let cc: [u8; 4] = col.into();
        let base = self.verts.len() as u16;
        for p in pts {
            self.verts.push(Vertex { position: *p, uv: Vec2::ZERO, color: cc, normal: Vec4::ZERO });
        }
        for k in 1..pts.len() as u16 - 1 {
            self.idx.extend_from_slice(&[base, base + k, base + k + 1]);
        }
    }

    /// Convex polygon; the normal is computed and turned away from `inside`.
    fn poly(&mut self, pts: &[Vec3], inside: Vec3, c: Color) {
        let mut n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalize_or_zero();
        let centre = pts.iter().copied().sum::<Vec3>() / pts.len() as f32;
        if n.dot(centre - inside) < 0.0 {
            n = -n;
        }
        self.poly_n(pts, n, c);
    }

    fn flat(&mut self, pts: &[Vec3], c: Color) {
        self.poly_n(pts, Vec3::Y, c);
    }

    /// Axis-aligned or rotated box. `dir` is the unit heading of the local x axis in the x/z plane.
    fn boxx(&mut self, centre: Vec3, size: Vec3, dir: Vec2, c: Color) {
        let h = size * 0.5;
        let ax = vec3(dir.x, 0.0, dir.y);
        let az = vec3(-dir.y, 0.0, dir.x);
        let corner = |sx: f32, sy: f32, sz: f32| centre + ax * h.x * sx + Vec3::Y * h.y * sy + az * h.z * sz;
        let faces: [[(f32, f32, f32); 4]; 5] = [
            [(-1., 1., -1.), (1., 1., -1.), (1., 1., 1.), (-1., 1., 1.)],
            [(-1., -1., 1.), (1., -1., 1.), (1., 1., 1.), (-1., 1., 1.)],
            [(-1., -1., -1.), (1., -1., -1.), (1., 1., -1.), (-1., 1., -1.)],
            [(1., -1., -1.), (1., -1., 1.), (1., 1., 1.), (1., 1., -1.)],
            [(-1., -1., -1.), (-1., -1., 1.), (-1., 1., 1.), (-1., 1., -1.)],
        ];
        for f in faces {
            let pts: Vec<Vec3> = f.iter().map(|(a, b, d)| corner(*a, *b, *d)).collect();
            self.poly(&pts, centre, c);
        }
    }

    fn abox(&mut self, min: Vec3, max: Vec3, c: Color) {
        self.boxx((min + max) * 0.5, max - min, vec2(1.0, 0.0), c);
    }

    /// Cone (or a low-poly blob when `under` > 0, adding an inverted cone beneath).
    fn cone(&mut self, base: Vec3, r: f32, h: f32, under: f32, sides: usize, rot: f32, c: Color) {
        let apex = base + Vec3::Y * h;
        let low = base - Vec3::Y * under;
        let inside = base + Vec3::Y * h * 0.3;
        for k in 0..sides {
            let a0 = rot + k as f32 / sides as f32 * std::f32::consts::TAU;
            let a1 = rot + (k + 1) as f32 / sides as f32 * std::f32::consts::TAU;
            let p0 = base + vec3(a0.cos() * r, 0.0, a0.sin() * r);
            let p1 = base + vec3(a1.cos() * r, 0.0, a1.sin() * r);
            self.poly(&[p0, p1, apex], inside, c);
            if under > 0.0 {
                self.poly(&[p0, p1, low], inside, darker(c, 0.95));
            } else {
                self.flat(&[base, p1, p0], darker(c, 0.7));
            }
        }
    }

    fn disk(&mut self, centre: Vec3, r: f32, sides: usize, c: Color) {
        let pts: Vec<Vec3> = (0..sides)
            .map(|k| {
                let a = k as f32 / sides as f32 * std::f32::consts::TAU;
                centre + vec3(a.cos() * r, 0.0, a.sin() * r)
            })
            .collect();
        self.flat(&pts, c);
    }

    fn rect_y(&mut self, x0: f32, z0: f32, x1: f32, z1: f32, y: f32, c: Color) {
        self.flat(&[vec3(x0, y, z0), vec3(x1, y, z0), vec3(x1, y, z1), vec3(x0, y, z1)], c);
    }

    /// A wall quad between two points on the ground plan, from y0 up to y1.
    fn wall(&mut self, a: Vec2, b: Vec2, y0: f32, y1: f32, outward: Vec2, c: Color) {
        let n = vec3(outward.x, 0.0, outward.y).normalize_or_zero();
        self.poly_n(&[vec3(a.x, y0, a.y), vec3(b.x, y0, b.y), vec3(b.x, y1, b.y), vec3(a.x, y1, a.y)], n, c);
    }
}

// ---- Terrain -------------------------------------------------------------------

/// One tile of a raised, smooth-cornered slab. Convex corners (where neither neighbour beside
/// the corner belongs to the slab) are rounded; walls are drawn only on open edges.
#[allow(clippy::too_many_arguments)]
fn slab_tile(mb: &mut Mb, x: i32, z: i32, top: f32, bottom: f32, r: f32, same: &dyn Fn(i32, i32) -> bool, top_c: Color, side_c: Color) {
    let (fx, fz) = (x as f32, z as f32);
    // Corners in order: (-,-), (+,-), (+,+), (-,+)
    let signs = [(-1, -1), (1, -1), (1, 1), (-1, 1)];
    let round: Vec<bool> = signs.iter().map(|(sx, sz)| !same(x + sx, z) && !same(x, z + sz)).collect();
    // Top surface as a plus shape plus corner pieces.
    mb.rect_y(fx + r, fz, fx + 1.0 - r, fz + 1.0, top, top_c);
    mb.rect_y(fx, fz + r, fx + r, fz + 1.0 - r, top, top_c);
    mb.rect_y(fx + 1.0 - r, fz + r, fx + 1.0, fz + 1.0 - r, top, top_c);
    const SEG: usize = 5;
    for (k, (sx, sz)) in signs.iter().enumerate() {
        let cx = if *sx < 0 { fx + r } else { fx + 1.0 - r };
        let cz = if *sz < 0 { fz + r } else { fz + 1.0 - r };
        let ox = if *sx < 0 { fx } else { fx + 1.0 - r };
        let oz = if *sz < 0 { fz } else { fz + 1.0 - r };
        if round[k] {
            use std::f32::consts::{FRAC_PI_2, PI};
            let a_start = match (sx > &0, sz > &0) {
                (true, true) => 0.0,
                (false, true) => FRAC_PI_2,
                (false, false) => PI,
                (true, false) => 3.0 * FRAC_PI_2,
            };
            let a_end = a_start + FRAC_PI_2;
            let mut pts = vec![vec3(cx, top, cz)];
            let mut arc = Vec::new();
            for s in 0..=SEG {
                let a = a_start + (a_end - a_start) * s as f32 / SEG as f32;
                let p = vec2(cx + a.cos() * r, cz + a.sin() * r);
                arc.push(p);
                pts.push(vec3(p.x, top, p.y));
            }
            mb.flat(&pts, top_c);
            for w in arc.windows(2) {
                let mid = (w[0] + w[1]) * 0.5;
                mb.wall(w[0], w[1], bottom, top, mid - vec2(cx, cz), side_c);
            }
        } else {
            mb.rect_y(ox, oz, ox + r, oz + r, top, top_c);
        }
    }
    // Straight walls on open edges, shortened where a corner is rounded.
    let edges = [
        ((0, -1), 0, 1, vec2(fx, fz), vec2(fx + 1.0, fz)),
        ((1, 0), 1, 2, vec2(fx + 1.0, fz), vec2(fx + 1.0, fz + 1.0)),
        ((0, 1), 3, 2, vec2(fx, fz + 1.0), vec2(fx + 1.0, fz + 1.0)),
        ((-1, 0), 0, 3, vec2(fx, fz), vec2(fx, fz + 1.0)),
    ];
    for ((dx, dz), ca, cb, a, b) in edges {
        if same(x + dx, z + dz) {
            continue;
        }
        let along = (b - a).normalize();
        let a2 = if round[ca] { a + along * r } else { a };
        let b2 = if round[cb] { b - along * r } else { b };
        mb.wall(a2, b2, bottom, top, vec2(dx as f32, dz as f32), side_c);
    }
}

/// Fill the inside corner of tile (x, z) where slab `same` wraps around it, with a concave arc,
/// so staircase outlines read as smooth curves.
fn fillet_tile(mb: &mut Mb, x: i32, z: i32, top: f32, bottom: f32, r: f32, same: &dyn Fn(i32, i32) -> bool, top_c: Color, side_c: Color) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let (fx, fz) = (x as f32, z as f32);
    for (sx, sz) in [(-1i32, -1i32), (1, -1), (1, 1), (-1, 1)] {
        if !(same(x + sx, z) && same(x, z + sz) && same(x + sx, z + sz)) {
            continue;
        }
        let corner = vec2(if sx > 0 { fx + 1.0 } else { fx }, if sz > 0 { fz + 1.0 } else { fz });
        // Arc centre sits inside this tile, r away from both edges.
        let c = corner - vec2(sx as f32, sz as f32) * r;
        // The arc faces back toward the corner: angles pointing toward (sx, sz).
        let a_start = match (sx > 0, sz > 0) {
            (true, true) => 0.0,
            (false, true) => FRAC_PI_2,
            (false, false) => PI,
            (true, false) => 3.0 * FRAC_PI_2,
        };
        const SEG: usize = 6;
        let arc: Vec<Vec2> = (0..=SEG)
            .map(|k| {
                let a = a_start + FRAC_PI_2 * k as f32 / SEG as f32;
                c + vec2(a.cos(), a.sin()) * r
            })
            .collect();
        let mut pts = vec![vec3(corner.x, top, corner.y)];
        pts.extend(arc.iter().map(|p| vec3(p.x, top, p.y)));
        mb.flat(&pts, top_c);
        for w in arc.windows(2) {
            let mid = (w[0] + w[1]) * 0.5;
            mb.wall(w[0], w[1], bottom, top, c - mid, side_c);
        }
    }
}

fn terrain_at(s: &Sim, x: i32, z: i32) -> Option<Terrain> {
    (x >= 0 && z >= 0 && x < W && z < H).then(|| s.terrain[idx((x, z))])
}

/// How far the countryside reaches past the board, so the screen never shows an empty edge.
const APRON: i32 = 16;

/// Terrain around the board: rivers and channels that touch an edge carry straight on,
/// everything else is plain grass.
fn ext_terrain(s: &Sim, x: i32, z: i32) -> Terrain {
    if let Some(t) = terrain_at(s, x, z) {
        return t;
    }
    let inside_x = (0..W).contains(&x);
    let inside_z = (0..H).contains(&z);
    let edge = if inside_z && !inside_x {
        terrain_at(s, x.clamp(0, W - 1), z)
    } else if inside_x && !inside_z {
        terrain_at(s, x, z.clamp(0, H - 1))
    } else {
        None
    };
    if edge == Some(Terrain::Water) {
        Terrain::Water
    } else {
        Terrain::Grass
    }
}

/// Colour of the countryside outside the play area: the same grass, a little muted.
fn apron_grass() -> Color {
    Color::new(GRASS.r * 0.9, GRASS.g * 0.92, GRASS.b * 0.88, 1.0)
}

/// What the screen shows past the edge of everything drawn: the countryside's lit colour.
pub fn ground_clear() -> Color {
    shade(apron_grass(), Vec3::Y)
}

fn draw_apron(mb: &mut Mb, s: &Sim) {
    let grass = apron_grass();
    let outside = |x: i32, z: i32| !(0..W).contains(&x) || !(0..H).contains(&z);
    for z in -APRON..H + APRON {
        // Merge runs of outside grass into long strips to keep the mesh small.
        let mut x = -APRON;
        while x < W + APRON {
            if !outside(x, z) {
                x += 1;
                continue;
            }
            let t = ext_terrain(s, x, z);
            let start = x;
            while x < W + APRON && outside(x, z) && ext_terrain(s, x, z) == t {
                x += 1;
            }
            if t == Terrain::Grass {
                mb.rect_y(start as f32, z as f32, x as f32, z as f32 + 1.0, GROUND_Y - 0.001, grass);
            }
        }
        // Banks where outside water meets land.
        for x in -APRON..W + APRON {
            if !outside(x, z) || ext_terrain(s, x, z) != Terrain::Water {
                continue;
            }
            for d in DIRS {
                if ext_terrain(s, x + d.0, z + d.1) == Terrain::Water {
                    continue;
                }
                let (fx, fz) = (x as f32, z as f32);
                let (a, b) = match d {
                    (1, 0) => (vec2(fx + 1.0, fz), vec2(fx + 1.0, fz + 1.0)),
                    (-1, 0) => (vec2(fx, fz), vec2(fx, fz + 1.0)),
                    (0, 1) => (vec2(fx, fz + 1.0), vec2(fx + 1.0, fz + 1.0)),
                    _ => (vec2(fx, fz), vec2(fx + 1.0, fz)),
                };
                mb.wall(a, b, WATER_Y, GROUND_Y, vec2(-d.0 as f32, -d.1 as f32), SAND);
            }
        }
    }
    // Scattered groves so the countryside isn't bare.
    for z in -APRON..H + APRON {
        for x in -APRON..W + APRON {
            if !outside(x, z) || ext_terrain(s, x, z) != Terrain::Grass || hash(x, z, 60) > 0.16 {
                continue;
            }
            let c = vec3(x as f32 + 0.3 + hash(x, z, 61) * 0.4, 0.0, z as f32 + 0.3 + hash(x, z, 62) * 0.4);
            let sc = 0.8 + hash(x, z, 63) * 0.6;
            mb.disk(c + vec3(0.12, 0.014, -0.08) * sc, 0.2 * sc, 8, SHADOW);
            if hash(x, z, 64) < 0.4 {
                mb.cone(c, 0.2 * sc, 0.6 * sc, 0.0, 6, hash(x, z, 65), PINE);
            } else {
                mb.cone(c + Vec3::Y * 0.2 * sc, 0.25 * sc, 0.24 * sc, 0.2 * sc, 6, hash(x, z, 65), if hash(x, z, 66) < 0.5 { LEAF_A } else { LEAF_B });
            }
        }
    }
    // A faint line marks where the play area ends.
    let e = 0.03;
    let (w, h) = (W as f32, H as f32);
    let line = Color::new(1.0, 1.0, 1.0, 0.35);
    for (x0, z0, x1, z1) in [(-e, -e, w + e, e), (-e, h - e, w + e, h + e), (-e, -e, e, h + e), (w - e, -e, w + e, h + e)] {
        mb.rect_y(x0, z0, x1, z1, GROUND_Y + 0.003, line);
    }
}

fn draw_terrain(mb: &mut Mb, s: &Sim) {
    // Water surface under the board and the countryside around it; land sits on top.
    let a = APRON as f32;
    mb.rect_y(-a, -a, W as f32 + a, H as f32 + a, WATER_Y, WATER);
    draw_apron(mb, s);

    // Land (grass and hills) as one smooth slab; off-board counts as land so the edges stay square.
    let land = |x: i32, z: i32| ext_terrain(s, x, z) != Terrain::Water;
    for z in 0..H {
        for x in 0..W {
            if !land(x, z) {
                fillet_tile(mb, x, z, GROUND_Y, BASE_Y, 0.42, &land, GRASS, SAND);
                // Gentle depth tint and moving glints on the water.
                let near_land = DIRS.iter().any(|d| land(x + d.0, z + d.1) && terrain_at(s, x + d.0, z + d.1).is_some());
                if !near_land {
                    let t = noise(x as f32, z as f32, 3.0, 9) * 0.5;
                    mb.rect_y(x as f32, z as f32, x as f32 + 1.0, z as f32 + 1.0, WATER_Y + 0.001, alpha(WATER_DEEP, t * 0.6));
                }
                continue;
            }
            let g = GRASS;
            slab_tile(mb, x, z, GROUND_Y, BASE_Y, 0.42, &land, g, SAND);
            // Sandy rim along the water's edge.
            for d in DIRS {
                if terrain_at(s, x + d.0, z + d.1) == Some(Terrain::Water) {
                    let (fx, fz) = (x as f32, z as f32);
                    let w = 0.12;
                    let (x0, z0, x1, z1) = match d {
                        (1, 0) => (fx + 1.0 - w, fz, fx + 1.0, fz + 1.0),
                        (-1, 0) => (fx, fz, fx + w, fz + 1.0),
                        (0, 1) => (fx, fz + 1.0 - w, fx + 1.0, fz + 1.0),
                        _ => (fx, fz, fx + 1.0, fz + w),
                    };
                    // Keep the rim off rounded corners.
                    let inset = 0.42;
                    let (x0, x1) = if d.0 == 0 {
                        (x0 + if !land(x - 1, z) { inset } else { 0.0 }, x1 - if !land(x + 1, z) { inset } else { 0.0 })
                    } else {
                        (x0, x1)
                    };
                    let (z0, z1) = if d.1 == 0 {
                        (z0 + if !land(x, z - 1) { inset } else { 0.0 }, z1 - if !land(x, z + 1) { inset } else { 0.0 })
                    } else {
                        (z0, z1)
                    };
                    if x1 > x0 && z1 > z0 {
                        mb.rect_y(x0, z0, x1, z1, GROUND_Y + 0.002, SAND);
                    }
                }
            }
        }
    }

    // Hills: rounded plateaus with cliffs, a few rocks and pines.
    let hill = |x: i32, z: i32| terrain_at(s, x, z) == Some(Terrain::Hill);
    for z in 0..H {
        for x in 0..W {
            if !hill(x, z) {
                if terrain_at(s, x, z) == Some(Terrain::Grass) {
                    fillet_tile(mb, x, z, HILL_Y, GROUND_Y, 0.4, &hill, HILL, CLIFF);
                }
                continue;
            }
            slab_tile(mb, x, z, HILL_Y, GROUND_Y, 0.4, &hill, HILL, CLIFF);
            if s.cells[idx((x, z))] != Cell::Empty {
                continue;
            }
            let (fx, fz) = (x as f32, z as f32);
            if hash(x, z, 7) < 0.35 {
                let c = vec3(fx + 0.3 + hash(x, z, 8) * 0.4, HILL_Y, fz + 0.3 + hash(x, z, 9) * 0.4);
                let r = 0.08 + hash(x, z, 11) * 0.06;
                mb.cone(c, r * 1.3, r * 1.2, 0.0, 5, hash(x, z, 12) * 3.0, ROCK);
            } else if hash(x, z, 13) < 0.3 {
                let c = vec3(fx + 0.5, HILL_Y, fz + 0.5);
                mb.boxx(c + Vec3::Y * 0.06, vec3(0.06, 0.12, 0.06), vec2(1.0, 0.0), TRUNK);
                mb.cone(c + Vec3::Y * 0.1, 0.2, 0.5, 0.0, 6, 0.3, PINE);
            }
        }
    }
}

// ---- Roads --------------------------------------------------------------------

fn connects(s: &Sim, from: Cell, from_pos: Pos, to: Pos) -> bool {
    match s.cell(to) {
        Cell::Road => true,
        // Houses join the road only through their driveway.
        Cell::Home(h) => from == Cell::Road && s.driveway(h) == from_pos,
        Cell::Door(_) => from == Cell::Road,
        Cell::Dest(id) => matches!(from, Cell::Door(d) if d == id),
        Cell::Park(_) => false,
        Cell::Empty => false,
    }
}

/// Height of the road surface on a tile: on top of hills, at ground level elsewhere.
fn road_h(s: &Sim, p: Pos) -> f32 {
    match terrain_at(s, p.0, p.1) {
        // Roads through hills run in a tunnel at ground level, hidden under the hill.
        Some(Terrain::Water) => BRIDGE_Y,
        _ => GROUND_Y,
    }
}

/// Bridge decks stand clear of the water; the road ramps up to them on the bank tile.
const BRIDGE_Y: f32 = 0.24;

const HWY_Y: f32 = 0.7;
const RAMP: f32 = 0.9;

/// Height along a highway at distance `d` from its start, with ramps at both ends.
fn hwy_height(d: f32, len: f32) -> f32 {
    let up = (d / RAMP).clamp(0.0, 1.0).min(((len - d) / RAMP).clamp(0.0, 1.0));
    up * up * (3.0 - 2.0 * up) * HWY_Y
}

fn draw_roads(mb: &mut Mb, s: &Sim) {
    for (layer, (w, y, c)) in [(0.38, 0.006, CURB), (0.31, 0.012, ASPHALT)].into_iter().enumerate() {
        for z in 0..H {
            for x in 0..W {
                let cell = s.cells[idx((x, z))];
                if !matches!(cell, Cell::Road | Cell::Door(_)) {
                    continue;
                }
                let hy = road_h(s, (x, z));
                let on_water = terrain_at(s, x, z) == Some(Terrain::Water);
                let ctr = vec3(x as f32 + 0.5, y + hy, z as f32 + 0.5);
                let links: Vec<(i32, i32)> = DIRS.iter().copied().filter(|d| connects(s, cell, (x, z), (x + d.0, z + d.1))).collect();
                if layer == 0 && on_water {
                    // Bridge deck, pillar and railings
                    let deck_c = Color::new(0.86, 0.85, 0.81, 1.0);
                    let top = BRIDGE_Y;
                    // Shadow on the water, deck slab, and a pair of piers.
                    mb.rect_y(ctr.x - 0.35, ctr.z - 0.3, ctr.x + 0.55, ctr.z + 0.5, WATER_Y + 0.003, alpha(SHADOW, 0.22));
                    mb.boxx(vec3(ctr.x, top - 0.07, ctr.z), vec3(0.9, 0.14, 0.9), vec2(1.0, 0.0), deck_c);
                    let along_x = links.iter().any(|d| d.1 == 0);
                    for off in [-0.22f32, 0.22] {
                        let (px, pz) = if along_x { (ctr.x, ctr.z + off) } else { (ctr.x + off, ctr.z) };
                        let h = top - 0.14 - WATER_Y;
                        mb.boxx(vec3(px, WATER_Y + h / 2.0, pz), vec3(0.14, h, 0.14), vec2(1.0, 0.0), Color::new(0.7, 0.69, 0.66, 1.0));
                    }
                    for d in DIRS {
                        if links.contains(&d) {
                            continue;
                        }
                        let (dx, dz) = (d.0 as f32, d.1 as f32);
                        let rc = vec3(ctr.x + dx * 0.42, BRIDGE_Y + 0.07, ctr.z + dz * 0.42);
                        let size = if d.0 == 0 { vec3(0.9, 0.14, 0.06) } else { vec3(0.06, 0.14, 0.9) };
                        mb.boxx(rc, size, vec2(1.0, 0.0), Color::new(0.93, 0.92, 0.88, 1.0));
                    }
                }
                let junction_here = s.junctions[idx((x, z))];
                if junction_here == Junction::Roundabout && cell == Cell::Road {
                    // A roundabout widens into a ring around a planted island.
                    mb.disk(ctr, w + 0.1, 28, c);
                } else {
                    mb.disk(ctr, w, 16, c);
                }
                for d in &links {
                    let (dx, dz) = (d.0 as f32, d.1 as f32);
                    let nh = road_h(s, (x + d.0, z + d.1));
                    // Bridges keep their deck height right to the bank; the ramp is on the land tile.
                    let edge_y = y + if hy == BRIDGE_Y || nh == BRIDGE_Y { hy.max(nh) } else { (hy + nh) * 0.5 };
                    let end = vec3(ctr.x + dx * 0.5, edge_y, ctr.z + dz * 0.5);
                    let side = vec3(-dz, 0.0, dx) * w;
                    mb.flat(&[ctr + side, end + side, end - side, ctr - side], c);
                }
                if layer == 1 {
                    let my = ctr.y + 0.004;
                    let junction = s.junctions[idx((x, z))];
                    if let Cell::Door(_) = cell {
                        for i in 0..3 {
                            let lx = ctr.x - 0.2 + i as f32 * 0.2;
                            mb.rect_y(lx - 0.015, ctr.z - 0.05, lx + 0.015, ctr.z + 0.25, my, MARKING);
                        }
                    } else if junction == Junction::Roundabout {
                        // Dashed ring line, then the kerbed island with a small tree.
                        for k in 0..12 {
                            let a0 = k as f32 / 12.0 * std::f32::consts::TAU;
                            let a1 = a0 + 0.26;
                            let (r0, r1) = (0.305, 0.335);
                            let pt = |a: f32, r: f32| vec3(ctr.x + a.cos() * r, my, ctr.z + a.sin() * r);
                            mb.flat(&[pt(a0, r0), pt(a1, r0), pt(a1, r1), pt(a0, r1)], MARKING);
                        }
                        mb.disk(vec3(ctr.x, my + 0.002, ctr.z), 0.22, 24, MARKING);
                        mb.disk(vec3(ctr.x, my + 0.004, ctr.z), 0.185, 24, GRASS);
                        mb.cone(vec3(ctr.x, my, ctr.z), 0.1, 0.22, 0.06, 6, 0.0, LEAF_B);
                    } else if junction == Junction::Light {
                        // Drawn every frame by draw_lights, because the colours change.
                    } else if links.len() == 2 && links[0].0 == -links[1].0 && links[0].1 == -links[1].1 && !on_water {
                        let horizontal = links[0].1 == 0;
                        for off in [-0.22, 0.22] {
                            if horizontal {
                                mb.rect_y(ctr.x + off - 0.1, ctr.z - 0.02, ctr.x + off + 0.1, ctr.z + 0.02, my, MARKING);
                            } else {
                                mb.rect_y(ctr.x - 0.02, ctr.z + off - 0.1, ctr.x + 0.02, ctr.z + off + 0.1, my, MARKING);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Moving glints on open water.
fn draw_water_glints(mb: &mut Mb, s: &Sim, time: f32) {
    let land = |x: i32, z: i32| terrain_at(s, x, z).map_or(true, |t| t != Terrain::Water);
    for z in 0..H {
        for x in 0..W {
            if land(x, z) || hash(x, z, 3) >= 0.35 {
                continue;
            }
            let ph = hash(x, z, 4) * 6.28;
            let ox = (time * 0.6 + ph).sin() * 0.1;
            let (px, pz) = (x as f32 + 0.25 + hash(x, z, 5) * 0.3 + ox, z as f32 + 0.3 + hash(x, z, 6) * 0.4);
            mb.rect_y(px, pz, px + 0.26, pz + 0.04, WATER_Y + 0.004, alpha(FOAM, 0.55));
        }
    }
}

/// Traffic lights: pole, lamp and stop bars coloured by the current phase.
fn draw_lights(mb: &mut Mb, s: &Sim) {
    for z in 0..H {
        for x in 0..W {
            if s.cells[idx((x, z))] != Cell::Road || s.junctions[idx((x, z))] != Junction::Light {
                continue;
            }
            let cell = Cell::Road;
            let hy = road_h(s, (x, z));
            let ctr = vec3(x as f32 + 0.5, 0.012 + hy, z as f32 + 0.5);
            let my = ctr.y + 0.004;
            let links: Vec<(i32, i32)> = DIRS.iter().copied().filter(|d| connects(s, cell, (x, z), (x + d.0, z + d.1))).collect();
            let pole = vec3(ctr.x + 0.36, hy, ctr.z - 0.36);
            mb.boxx(pole + Vec3::Y * 0.2, vec3(0.04, 0.4, 0.04), vec2(1.0, 0.0), Color::new(0.3, 0.3, 0.33, 1.0));
            let green_x = s.light_green((x, z), 0);
            let (on, off) = (Color::new(0.3, 0.85, 0.4, 1.0), Color::new(0.9, 0.3, 0.25, 1.0));
            mb.boxx(pole + vec3(-0.05, 0.42, 0.0), vec3(0.07, 0.07, 0.07), vec2(1.0, 0.0), if green_x { on } else { off });
            mb.boxx(pole + vec3(0.0, 0.42, 0.05), vec3(0.07, 0.07, 0.07), vec2(1.0, 0.0), if green_x { off } else { on });
            // Stop bars: green arms may go, red arms wait.
            for d in &links {
                let (dx, dz) = (d.0 as f32, d.1 as f32);
                let go = (d.1 == 0) == green_x;
                let c2 = vec3(ctr.x + dx * 0.34, my, ctr.z + dz * 0.34);
                let side = vec3(-dz, 0.0, dx) * 0.29;
                let along = vec3(dx, 0.0, dz) * 0.07;
                mb.flat(&[c2 + side + along, c2 - side + along, c2 - side, c2 + side], if go { on } else { off });
            }
            mb.disk(pole + vec3(0.0, 0.46, 0.0), 0.09, 12, Color::new(0.22, 0.22, 0.25, 1.0));
            mb.disk(pole + vec3(0.0, 0.47, 0.0), 0.055, 12, if green_x { on } else { off });
        }
    }
}

/// Tunnels: a stone portal where a road enters a hill, and a faint trace over the hilltop
/// so the player can see (and erase) the route.
fn draw_tunnels(mb: &mut Mb, s: &Sim) {
    let hill = |x: i32, z: i32| terrain_at(s, x, z) == Some(Terrain::Hill);
    for z in 0..H {
        for x in 0..W {
            if !hill(x, z) || s.cells[idx((x, z))] != Cell::Road {
                continue;
            }
            let c = vec2(x as f32 + 0.5, z as f32 + 0.5);
            let links: Vec<(i32, i32)> = DIRS.iter().copied().filter(|d| connects(s, Cell::Road, (x, z), (x + d.0, z + d.1))).collect();
            mb.disk(vec3(c.x, HILL_Y + 0.004, c.y), 0.12, 10, alpha(SHADOW, 0.16));
            for d in &links {
                let dir = vec2(d.0 as f32, d.1 as f32);
                let side = vec2(-dir.y, dir.x) * 0.12;
                let (a, b) = (c, c + dir * 0.5);
                let q = |p: Vec2| vec3(p.x, HILL_Y + 0.004, p.y);
                mb.flat(&[q(a + side), q(b + side), q(b - side), q(a - side)], alpha(SHADOW, 0.16));
                if hill(x + d.0, z + d.1) {
                    continue;
                }
                // Portal on the cliff face: stone frame and a dark arched mouth.
                let n = vec3(dir.x, 0.0, dir.y);
                let t = vec3(-dir.y, 0.0, dir.x);
                let face = vec3(c.x, 0.0, c.y) + n * 0.5;
                let stone = Color::new(0.74, 0.72, 0.68, 1.0);
                let arch = |w: f32, h: f32, off: f32| -> Vec<Vec3> {
                    let mut pts = vec![face + n * off - t * w, face + n * off + t * w];
                    for k in 0..=8 {
                        let ang = k as f32 / 8.0 * std::f32::consts::PI;
                        pts.push(face + n * off + t * (w * ang.cos()) + Vec3::Y * (h + w * 0.8 * ang.sin()));
                    }
                    pts
                };
                mb.poly_n(&arch(0.42, 0.2, 0.012), n, stone);
                mb.poly_n(&arch(0.3, 0.14, 0.02), n, Color::new(0.16, 0.16, 0.19, 1.0));
                // Seen from above: a stone lintel along the hilltop edge and shade over the
                // road just outside, so the entrance reads clearly at any angle.
                mb.boxx(face - n * 0.06 + Vec3::Y * (HILL_Y + 0.04), vec3(0.9, 0.08, 0.12), vec2(-dir.y, dir.x), stone);
                let (p0, p1) = (vec2(face.x, face.z), vec2(face.x, face.z) + dir * 0.28);
                let q = |p: Vec2| vec3(p.x, 0.02, p.y);
                let w = side * 2.6;
                mb.flat(&[q(p0 + w), q(p1 + w), q(p1 - w), q(p0 - w)], alpha(SHADOW, 0.35));
            }
        }
    }
}

/// Elevated highways with ramps, pillars and side barriers.
fn draw_highways(mb: &mut Mb, s: &Sim) {
    for h in &s.highways {
        let a = vec2(h.a.0 as f32 + 0.5, h.a.1 as f32 + 0.5);
        let b = vec2(h.b.0 as f32 + 0.5, h.b.1 as f32 + 0.5);
        let len = (b - a).length();
        let dir = (b - a) / len;
        let side = vec2(-dir.y, dir.x);
        let steps = (len * 4.0).ceil() as usize;
        for k in 0..steps {
            let d0 = len * k as f32 / steps as f32;
            let d1 = len * (k + 1) as f32 / steps as f32;
            let (p0, p1) = (a + dir * d0, a + dir * d1);
            let (y0, y1) = (hwy_height(d0, len) + 0.02, hwy_height(d1, len) + 0.02);
            let q = |p: Vec2, y: f32, sd: f32| vec3(p.x + side.x * sd, y, p.y + side.y * sd);
            mb.flat(&[q(p0, y0, 0.36), q(p1, y1, 0.36), q(p1, y1, -0.36), q(p0, y0, -0.36)], Color::new(0.88, 0.87, 0.83, 1.0));
            mb.flat(&[q(p0, y0 + 0.005, 0.3), q(p1, y1 + 0.005, 0.3), q(p1, y1 + 0.005, -0.3), q(p0, y0 + 0.005, -0.3)], darker(ASPHALT, 0.9));
            if k % 2 == 0 {
                mb.flat(&[q(p0, y0 + 0.01, 0.02), q(p1, y1 + 0.01, 0.02), q(p1, y1 + 0.01, -0.02), q(p0, y0 + 0.01, -0.02)], Color::new(1.0, 0.85, 0.35, 1.0));
            }
            if y0 > 0.2 {
                // Side barriers and an underside
                for sd in [0.36f32, -0.36] {
                    mb.poly(&[q(p0, y0, sd), q(p1, y1, sd), q(p1, y1 + 0.08, sd), q(p0, y0 + 0.08, sd)], vec3((p0.x + p1.x) / 2.0, y0, (p0.y + p1.y) / 2.0), Color::new(0.93, 0.92, 0.89, 1.0));
                    mb.poly(&[q(p0, y0 - 0.1, sd), q(p1, y1 - 0.1, sd), q(p1, y1, sd), q(p0, y0, sd)], vec3((p0.x + p1.x) / 2.0, y0, (p0.y + p1.y) / 2.0), Color::new(0.75, 0.74, 0.7, 1.0));
                }
            }
        }
        // Pillars
        let mut d = RAMP + 0.4;
        while d < len - RAMP {
            let p = a + dir * d;
            mb.boxx(vec3(p.x, HWY_Y / 2.0 - 0.15, p.y), vec3(0.14, HWY_Y + 0.3, 0.14), dir, Color::new(0.78, 0.77, 0.73, 1.0));
            d += 1.6;
        }
        // Shadow
        let o = vec2(0.3, -0.2);
        let (sa, sb) = (a + dir * RAMP + o, b - dir * RAMP + o);
        mb.flat(&[vec3(sa.x + side.x * 0.36, 0.015, sa.y + side.y * 0.36), vec3(sb.x + side.x * 0.36, 0.015, sb.y + side.y * 0.36), vec3(sb.x - side.x * 0.36, 0.015, sb.y - side.y * 0.36), vec3(sa.x - side.x * 0.36, 0.015, sa.y - side.y * 0.36)], SHADOW);
    }
}

// ---- Buildings, trees, cars ---------------------------------------------------

fn shadow_rect(mb: &mut Mb, x0: f32, z0: f32, x1: f32, z1: f32, h: f32) {
    // Cast away from the sun (toward +x, -z) in proportion to height.
    let off = vec2(0.45, -0.35) * h;
    mb.flat(
        &[
            vec3(x0, 0.016, z1),
            vec3(x0 + off.x, 0.016, z0 + off.y),
            vec3(x1 + off.x, 0.016, z0 + off.y),
            vec3(x1 + off.x, 0.016, z1 + off.y),
            vec3(x1, 0.016, z1),
        ],
        SHADOW,
    );
}

fn drop_height(age: f32) -> f32 {
    let t = (age / 0.6).clamp(0.0, 1.0);
    if t < 0.7 {
        (1.0 - (t / 0.7).powi(2)) * 2.0
    } else if t < 1.0 {
        ((t - 0.7) / 0.3 * std::f32::consts::PI).sin() * 0.12
    } else {
        0.0
    }
}

/// A house that faces its driveway, with a pitched roof, chimney and a paved drive.
fn draw_home(mb: &mut Mb, p: Pos, facing: usize, col: Color, lift: f32) {
    let c = vec3(p.0 as f32 + 0.5, 0.0, p.1 as f32 + 0.5);
    let f = vec2(DIRS[facing].0 as f32, DIRS[facing].1 as f32);
    let r = vec2(-f.y, f.x);
    // Local frame: lx to the house's right, lz toward the driveway.
    let tf = |lx: f32, ly: f32, lz: f32| c + vec3(r.x * lx + f.x * lz, ly, r.y * lx + f.y * lz);
    let y = lift;
    let (w, d, h) = (0.74, 0.54, 0.36);
    let zc = -0.08; // house sits toward the back so the drive fits in front
    // Driveway: the same asphalt and kerb as the roads, at road width, from the front wall
    // to the tile edge, so it joins a road seamlessly.
    let (cw, aw) = (0.38, 0.31);
    let front = zc + d / 2.0;
    mb.flat(&[tf(-cw, 0.006, front - 0.02), tf(cw, 0.006, front - 0.02), tf(cw, 0.006, 0.5), tf(-cw, 0.006, 0.5)], CURB);
    mb.flat(&[tf(-aw, 0.012, front), tf(aw, 0.012, front), tf(aw, 0.012, 0.5), tf(-aw, 0.012, 0.5)], ASPHALT);
    if lift == 0.0 {
        let a = tf(-w / 2.0, 0.0, zc - d / 2.0);
        let b = tf(w / 2.0, 0.0, zc + d / 2.0);
        shadow_rect(mb, a.x.min(b.x), a.z.min(b.z), a.x.max(b.x), a.z.max(b.z), h + 0.32);
    }
    // Walls
    let wc = tf(0.0, y + h / 2.0, zc);
    mb.boxx(wc, vec3(w, h, d), r, WALL);
    // Front door and windows
    let fz = zc + d / 2.0 + 0.006;
    mb.poly_n(&[tf(-0.06, y, fz), tf(0.06, y, fz), tf(0.06, y + 0.2, fz), tf(-0.06, y + 0.2, fz)], vec3(f.x, 0.0, f.y), darker(col, 0.55));
    for wx in [-0.2, 0.2] {
        mb.poly_n(&[tf(wx - 0.06, y + 0.12, fz), tf(wx + 0.06, y + 0.12, fz), tf(wx + 0.06, y + 0.24, fz), tf(wx - 0.06, y + 0.24, fz)], vec3(f.x, 0.0, f.y), WINDOW);
    }
    // Pitched roof: ridge runs parallel to the front, eaves overhang the walls.
    let (rw, rd, rh) = (w / 2.0 + 0.06, d / 2.0 + 0.08, 0.3);
    let y0 = y + h;
    let inside = tf(0.0, y0 + 0.05, zc);
    let ridge_a = tf(-rw, y0 + rh, zc);
    let ridge_b = tf(rw, y0 + rh, zc);
    mb.poly(&[tf(-rw, y0, zc + rd), tf(rw, y0, zc + rd), ridge_b, ridge_a], inside, col);
    mb.poly(&[tf(-rw, y0, zc - rd), tf(rw, y0, zc - rd), ridge_b, ridge_a], inside, col);
    mb.poly(&[tf(-rw, y0, zc - rd), tf(-rw, y0, zc + rd), ridge_a], inside, darker(col, 0.8));
    mb.poly(&[tf(rw, y0, zc - rd), tf(rw, y0, zc + rd), ridge_b], inside, darker(col, 0.8));
    // Ridge cap
    mb.boxx(tf(0.0, y0 + rh + 0.01, zc), vec3(rw * 2.0 + 0.02, 0.03, 0.05), r, lighter(col, 0.35));
    // Chimney
    mb.boxx(tf(0.18, y0 + 0.24, zc - 0.12), vec3(0.09, 0.3, 0.09), r, darker(col, 0.55));
}

/// A shop: flat roof with rooftop units and a sign, striped awning over the front,
/// and an entrance facing each of its fixed driveways.
fn draw_shop(mb: &mut Mb, d: &Dest, col: Color, lift: f32) {
    let o = d.origin;
    let (x0, z0) = (o.0 as f32 + 0.1, o.1 as f32 + 0.08);
    let (x1, z1) = (o.0 as f32 + d.size.0 as f32 - 0.1, o.1 as f32 + d.size.1 as f32 - 0.22);
    let y = lift;
    let h = if d.big() { 0.95 } else { 0.72 };
    if lift == 0.0 {
        shadow_rect(mb, x0, z0, x1, z1, h + 0.3);
    }
    let grey = Color::new(0.82, 0.83, 0.85, 1.0);
    // Car park across the shop's front row: kerb, asphalt, and painted bays between driveways.
    let front = (o.1 + d.size.1) as f32;
    let (lx0, lx1) = (o.0 as f32 + 0.04, (o.0 + d.size.0) as f32 - 0.04);
    // The lot runs right up to the shop front, like a house's driveway.
    let shop_front = front - 0.22;
    mb.rect_y(lx0, shop_front - 0.02, lx1, front + 0.92, y + 0.006, CURB);
    mb.rect_y(lx0 + 0.06, shop_front, lx1 - 0.06, front + 0.86, y + 0.012, ASPHALT);
    for k in 0..d.size.0 {
        let tx = (o.0 + k) as f32;
        if d.doors.iter().any(|p| p.0 == o.0 + k) {
            continue;
        }
        // Three bays per car-park tile, nose-in toward the shop.
        for b in 0..=3 {
            let bx = tx + 0.08 + b as f32 * 0.28;
            mb.rect_y(bx - 0.012, front + 0.04, bx + 0.012, front + 0.42, y + 0.016, MARKING);
        }
        // A couple of parked cars in the shop's colour, varied per tile.
        for b in 0..3 {
            if hash(o.0 + k, o.1 + b, 91) < 0.45 {
                let cx = tx + 0.22 + b as f32 * 0.28;
                let base = vec3(cx, y + 0.012, front + 0.23);
                mb.boxx(base + Vec3::Y * 0.07, vec3(0.17, 0.1, 0.3), vec2(0.0, 1.0), lighter(col, 0.15));
                mb.boxx(base + Vec3::Y * 0.15 + vec3(0.0, 0.0, 0.02), vec3(0.14, 0.06, 0.16), vec2(0.0, 1.0), Color::new(0.3, 0.35, 0.42, 1.0));
            }
        }
    }
    // Body and flat roof with a parapet
    mb.abox(vec3(x0, y, z0), vec3(x1, y + h, z1), lighter(col, 0.55));
    mb.abox(vec3(x0 - 0.04, y + h, z0 - 0.04), vec3(x1 + 0.04, y + h + 0.08, z1 + 0.04), darker(col, 0.8));
    mb.rect_y(x0 + 0.07, z0 + 0.07, x1 - 0.07, z1 - 0.07, y + h + 0.081, lighter(col, 0.25));
    // Rooftop units
    mb.abox(vec3(x0 + 0.25, y + h + 0.08, z0 + 0.2), vec3(x0 + 0.7, y + h + 0.3, z0 + 0.55), grey);
    mb.abox(vec3(x0 + 0.3, y + h + 0.3, z0 + 0.25), vec3(x0 + 0.65, y + h + 0.32, z0 + 0.5), darker(grey, 0.85));
    mb.cone(vec3(x1 - 0.45, y + h + 0.08, z0 + 0.42), 0.17, 0.14, 0.0, 10, 0.0, grey);
    if d.big() {
        // A skylight strip across the larger roof
        mb.abox(vec3(x0 + 1.0, y + h + 0.08, z0 + 0.3), vec3(x1 - 1.0, y + h + 0.18, z0 + 0.6), WINDOW);
    }
    // Sign along the roof's front edge
    let mid = (x0 + x1) / 2.0;
    mb.abox(vec3(mid - 0.55, y + h + 0.08, z1 - 0.14), vec3(mid + 0.55, y + h + 0.4, z1 - 0.05), col);
    mb.abox(vec3(mid - 0.45, y + h + 0.16, z1 - 0.045), vec3(mid + 0.45, y + h + 0.32, z1 - 0.035), WHITE);
    // Shop front: windows, an entrance per driveway, striped awning
    let f = z1 + 0.005;
    mb.poly_n(&[vec3(x0 + 0.12, y + 0.1, f), vec3(x1 - 0.12, y + 0.1, f), vec3(x1 - 0.12, y + 0.45, f), vec3(x0 + 0.12, y + 0.45, f)], Vec3::Z, WINDOW);
    for door in &d.doors {
        let dx = door.0 as f32 + 0.5;
        mb.poly_n(&[vec3(dx - 0.16, y, f + 0.001), vec3(dx + 0.16, y, f + 0.001), vec3(dx + 0.16, y + 0.42, f + 0.001), vec3(dx - 0.16, y + 0.42, f + 0.001)], Vec3::Z, darker(col, 0.5));
    }
    let stripes = (d.size.0 * 4) as usize;
    let (ay0, ay1, out) = (y + 0.62, y + 0.5, 0.24);
    for k in 0..stripes {
        let a = x0 + (x1 - x0) * k as f32 / stripes as f32;
        let b = x0 + (x1 - x0) * (k + 1) as f32 / stripes as f32;
        let c = if k % 2 == 0 { col } else { WHITE };
        mb.poly(&[vec3(a, ay0, z1), vec3(b, ay0, z1), vec3(b, ay1, z1 + out), vec3(a, ay1, z1 + out)], vec3(mid, y, z1 - 1.0), c);
    }
}

fn draw_trees(mb: &mut Mb, s: &Sim) {
    for z in 0..H {
        for x in 0..W {
            if s.terrain[idx((x, z))] != Terrain::Grass || s.cells[idx((x, z))] != Cell::Empty {
                continue;
            }
            let grove = noise(x as f32, z as f32, 5.0, 77);
            if grove < 0.6 || hash(x, z, 50) > (grove - 0.6) * 3.2 {
                continue;
            }
            let c = vec3(x as f32 + 0.3 + hash(x, z, 51) * 0.4, 0.0, z as f32 + 0.3 + hash(x, z, 52) * 0.4);
            let sc = 0.8 + hash(x, z, 53) * 0.5;
            mb.disk(c + vec3(0.12, 0.014, -0.08) * sc, 0.2 * sc, 8, SHADOW);
            mb.boxx(c + Vec3::Y * 0.08 * sc, vec3(0.06, 0.16, 0.06) * sc, vec2(1.0, 0.0), TRUNK);
            if hash(x, z, 54) < 0.35 {
                mb.cone(c + Vec3::Y * 0.12 * sc, 0.2 * sc, 0.55 * sc, 0.0, 6, hash(x, z, 55), PINE);
            } else {
                let leaf = if hash(x, z, 56) < 0.5 { LEAF_A } else { LEAF_B };
                mb.cone(c + Vec3::Y * 0.32 * sc, 0.24 * sc, 0.22 * sc, 0.2 * sc, 6, hash(x, z, 55), leaf);
            }
        }
    }
}

fn centre(p: Pos) -> Vec2 {
    vec2(p.0 as f32 + 0.5, p.1 as f32 + 0.5)
}

fn bezier(a: Vec2, b: Vec2, c: Vec2, u: f32) -> (Vec2, Vec2) {
    let p = a * (1.0 - u) * (1.0 - u) + b * 2.0 * u * (1.0 - u) + c * u * u;
    let d = (b - a) * 2.0 * (1.0 - u) + (c - b) * 2.0 * u;
    (p, d.normalize_or_zero())
}

/// Where a car is (in tile units on the ground plan) and which way it faces.
/// Cars follow a curve through turns instead of snapping at tile centres.
pub fn car_pose(c: &Car) -> (Vec2, Vec2) {
    let a = centre(c.prev);
    let b = centre(c.cur);
    let t = c.progress;
    let next = c.path.get(c.i + 1).map(|p| centre(*p));
    let before = if c.i >= 2 && c.path.get(c.i - 1) == Some(&c.prev) { Some(centre(c.path[c.i - 2])) } else { None };
    let turns = |p: Vec2, q: Vec2, r: Vec2| {
        let (d1, d2) = (q - p, r - q);
        d1.length_squared() > 0.1 && d2.length_squared() > 0.1 && d1.dot(d2).abs() < 0.01
    };
    if t >= 0.5 {
        if let Some(n) = next {
            if turns(a, b, n) {
                return bezier((a + b) * 0.5, b, (b + n) * 0.5, t - 0.5);
            }
        }
    } else if let Some(z) = before {
        if turns(z, a, b) {
            return bezier((z + a) * 0.5, a, (a + b) * 0.5, t + 0.5);
        }
    }
    let mut dir = (b - a).normalize_or_zero();
    if dir.length_squared() < 0.01 {
        dir = next.map(|n| (n - b).normalize_or_zero()).unwrap_or(vec2(1.0, 0.0));
    }
    (a + (b - a) * t, dir)
}

fn draw_cars(mb: &mut Mb, s: &Sim, poses: &[Option<(Vec2, Vec2)>]) {
    for (i, c) in s.cars.iter().enumerate() {
        if c.state == CarState::Idle {
            continue;
        }
        let col = PALETTE[s.homes[i].color];
        let (pos, dir) = poses.get(i).copied().flatten().unwrap_or_else(|| car_pose(c));
        let normal = vec2(-dir.y, dir.x);
        let p = pos + normal * 0.15;
        let seg = (centre(c.cur) - centre(c.prev)).length();
        let ground = if seg > 1.5 {
            hwy_height(seg * c.progress, seg) + 0.03
        } else {
            let hp = |q: Pos| road_h(s, q);
            hp(c.prev) + (hp(c.cur) - hp(c.prev)) * c.progress
        };
        let base = vec3(p.x, ground, p.y);
        mb.flat(
            &[
                base + vec3(-0.22, 0.017, -0.13) + vec3(0.06, 0.0, -0.04),
                base + vec3(0.22, 0.017, -0.13) + vec3(0.06, 0.0, -0.04),
                base + vec3(0.22, 0.017, 0.13) + vec3(0.06, 0.0, -0.04),
                base + vec3(-0.22, 0.017, 0.13) + vec3(0.06, 0.0, -0.04),
            ],
            alpha(SHADOW, 0.18),
        );
        mb.boxx(base + Vec3::Y * 0.1, vec3(0.44, 0.12, 0.24), dir, col);
        mb.boxx(base + Vec3::Y * 0.2 - vec3(dir.x, 0.0, dir.y) * 0.03, vec3(0.24, 0.09, 0.2), dir, lighter(Color::new(0.25, 0.3, 0.38, 1.0), 0.25));
        mb.boxx(base + Vec3::Y * 0.25 - vec3(dir.x, 0.0, dir.y) * 0.03, vec3(0.2, 0.02, 0.2), dir, lighter(col, 0.3));
    }
}

// ---- Public entry points ---------------------------------------------------------

pub struct Highlight {
    pub tile: Option<Pos>,
    /// Colour of the hover marker.
    pub tint: Color,
    pub flash: Option<(Pos, f32)>,
    /// Highway being placed: its start tile and whether the hovered end is valid.
    pub highway_from: Option<(Pos, bool)>,
}

/// The parts of the world that only change when the player builds or something spawns.
/// Rebuilding every mesh each frame was the main cost on phones, so they are kept between frames.
#[derive(Default)]
pub struct WorldCache {
    key: u64,
    meshes: Vec<Mesh>,
}

fn settled(s: &Sim, born: f32) -> bool {
    s.time - born >= 0.6
}

fn world_key(s: &Sim) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for t in &s.terrain {
        (*t as u8).hash(&mut h);
    }
    for c in &s.cells {
        match c {
            Cell::Empty => 0u64,
            Cell::Road => 1,
            Cell::Home(i) => 2 + 8 * *i as u64,
            Cell::Dest(i) => 3 + 8 * *i as u64,
            Cell::Door(i) => 4 + 8 * *i as u64,
            Cell::Park(i) => 5 + 8 * *i as u64,
        }
        .hash(&mut h);
    }
    for j in &s.junctions {
        (*j as u8).hash(&mut h);
    }
    for hw in &s.highways {
        (hw.a, hw.b).hash(&mut h);
    }
    for home in &s.homes {
        (home.pos, home.facing, home.color, settled(s, home.born)).hash(&mut h);
    }
    for d in &s.dests {
        (d.origin, d.color, settled(s, d.born)).hash(&mut h);
    }
    h.finish()
}

pub fn draw_world(s: &Sim, time: f32, hl: &Highlight, cache: &mut WorldCache, car_poses: &[Option<(Vec2, Vec2)>]) {
    let key = world_key(s);
    if key != cache.key || cache.meshes.is_empty() {
        let mut mb = Mb::new();
        mb.keep = Some(Vec::new());
        draw_terrain(&mut mb, s);
        draw_roads(&mut mb, s);
        draw_highways(&mut mb, s);
        draw_tunnels(&mut mb, s);
        for d in s.dests.iter().filter(|d| settled(s, d.born)) {
            draw_shop(&mut mb, d, PALETTE[d.color], 0.0);
        }
        for h in s.homes.iter().filter(|h| settled(s, h.born)) {
            draw_home(&mut mb, h.pos, h.facing, PALETTE[h.color], 0.0);
        }
        draw_trees(&mut mb, s);
        mb.flush();
        cache.meshes = mb.keep.take().unwrap_or_default();
        cache.key = key;
    }
    for m in &cache.meshes {
        draw_mesh(m);
    }

    let mut mb = Mb::new();
    draw_water_glints(&mut mb, s, time);
    draw_lights(&mut mb, s);
    for d in s.dests.iter().filter(|d| !settled(s, d.born)) {
        draw_shop(&mut mb, d, PALETTE[d.color], drop_height(s.time - d.born));
    }
    for h in s.homes.iter().filter(|h| !settled(s, h.born)) {
        draw_home(&mut mb, h.pos, h.facing, PALETTE[h.color], drop_height(s.time - h.born));
    }
    draw_cars(&mut mb, s, car_poses);
    if let Some(t) = hl.tile {
        let y = if terrain_at(s, t.0, t.1) == Some(Terrain::Hill) { HILL_Y } else { road_h(s, t) } + 0.03;
        mb.rect_y(t.0 as f32 + 0.04, t.1 as f32 + 0.04, t.0 as f32 + 0.96, t.1 as f32 + 0.96, y, hl.tint);
        if let Some((from, ok)) = hl.highway_from {
            let a = vec2(from.0 as f32 + 0.5, from.1 as f32 + 0.5);
            let b = vec2(t.0 as f32 + 0.5, t.1 as f32 + 0.5);
            let len = (b - a).length();
            if len > 0.1 {
                let dir = (b - a) / len;
                let side = vec2(-dir.y, dir.x) * 0.3;
                let c = if ok { Color::new(1.0, 0.85, 0.35, 0.6) } else { alpha(WARN, 0.5) };
                let q = |p: Vec2| vec3(p.x, HWY_Y, p.y);
                mb.flat(&[q(a + side), q(b + side), q(b - side), q(a - side)], c);
            }
            mb.rect_y(from.0 as f32 + 0.04, from.1 as f32 + 0.04, from.0 as f32 + 0.96, from.1 as f32 + 0.96, 0.035, Color::new(1.0, 0.85, 0.35, 0.6));
        }
    }
    if let Some((p, a)) = hl.flash {
        mb.rect_y(p.0 as f32, p.1 as f32, p.0 as f32 + 1.0, p.1 as f32 + 1.0, 0.035, alpha(WARN, a.min(0.6)));
    }
    mb.flush();
}

/// 2D overlays drawn after the 3D pass: demand pins and overflow rings stay crisp and readable.
pub fn draw_overlays(s: &Sim, sc: &Scene, time: f32) {
    for (i, d) in s.dests.iter().enumerate() {
        let col = PALETTE[d.color];
        let top = vec3(d.origin.0 as f32 + d.size.0 as f32 / 2.0, 1.4 + drop_height(s.time - d.born), d.origin.1 as f32 + 0.3);
        let c = sc.to_screen(top);
        let unit = (sc.to_screen(top + vec3(1.0, 0.0, 0.0)) - c).length();
        let n = d.pins.min(12) as usize;
        if n > 0 {
            let r = (unit * 0.15).max(5.0);
            let gap = r * 2.5;
            let per_row = n.min(6);
            let rows = n.div_ceil(6);
            let w = gap * (per_row as f32 - 1.0);
            // Bubble behind the pins
            let bw = w + r * 3.2;
            let bh = gap * (rows as f32 - 1.0) + r * 3.2;
            let (bx, by) = (c.x - bw / 2.0, c.y - bh);
            draw_rectangle(bx + r * 0.8, by, bw - r * 1.6, bh, alpha(WHITE, 0.92));
            draw_rectangle(bx, by + r * 0.8, bw, bh - r * 1.6, alpha(WHITE, 0.92));
            for (ex, ey) in [(bx + r * 0.8, by + r * 0.8), (bx + bw - r * 0.8, by + r * 0.8), (bx + r * 0.8, by + bh - r * 0.8), (bx + bw - r * 0.8, by + bh - r * 0.8)] {
                draw_circle(ex, ey, r * 0.8, alpha(WHITE, 0.92));
            }
            draw_triangle(vec2(c.x - r * 0.7, c.y - 0.5), vec2(c.x + r * 0.7, c.y - 0.5), vec2(c.x, c.y + r * 0.9), alpha(WHITE, 0.92));
            for k in 0..n {
                let (cx, cy) = (k % 6, k / 6);
                let px = c.x - w / 2.0 + cx as f32 * gap;
                let py = by + r * 1.6 + cy as f32 * gap;
                if k < d.assigned as usize {
                    draw_circle_lines(px, py, r, 2.0, col);
                } else {
                    draw_circle(px, py, r, col);
                }
            }
        }
        if d.overflow > 0.0 || d.pins > d.pin_limit() {
            let g = vec3(d.origin.0 as f32 + d.size.0 as f32 / 2.0, 0.05, d.origin.1 as f32 + d.size.1 as f32 / 2.0);
            let gc = sc.to_screen(g);
            let rad = (sc.to_screen(g + vec3(2.0, 0.0, 0.0)) - gc).length();
            draw_poly_lines(gc.x, gc.y, 64, rad, 0.0, 8.0, alpha(Color::new(0.2, 0.1, 0.1, 1.0), 0.18));
            let pulse = if s.failed_dest == Some(i) { 1.0 } else { 0.75 + 0.25 * (time * 6.0).sin() };
            draw_arc(gc.x, gc.y, 64, rad - 4.0, -90.0, 8.0, 360.0 * d.overflow.min(1.0), alpha(WARN, pulse));
        }
    }
}
