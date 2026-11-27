use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::map::{self, idx, in_bounds, MapKind, Pos, Terrain, DIRS, H, W};
use crate::rng::Rng;

/// Simulation step length in seconds.
pub const DT: f32 = 1.0 / 60.0;
/// Car speed in tiles per second.
pub const CAR_SPEED: f32 = 2.6;
/// Seconds a car spends at a destination before heading home.
pub const UNLOAD_TIME: f32 = 0.6;
/// Pins a destination can hold before its overflow timer starts.
pub const PIN_LIMIT: u32 = 4;
/// Seconds of continuous overflow before the game ends.
pub const OVERFLOW_TIME: f32 = 40.0;
pub const WEEK_LENGTH: f32 = 60.0;
pub const START_ROADS: u32 = 30;
pub const ROADS_PER_WEEK: u32 = 15;
pub const MAX_COLORS: usize = 6;

/// Footprint of a small shop (one driveway) and a big shop (two driveways).
pub const SMALL_SHOP: (i32, i32) = (3, 2);
pub const BIG_SHOP: (i32, i32) = (4, 2);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    Empty,
    Road,
    Home(usize),
    Dest(usize),
    Door(usize),
    /// Car park in front of a shop, between or beside its driveways. On a big shop cars can
    /// drive through it from one driveway to the other.
    Park(usize),
}

#[derive(Clone, Debug)]
pub struct Home {
    pub pos: Pos,
    pub color: usize,
    /// Sim time when it appeared, for the arrival animation.
    pub born: f32,
    /// Index into DIRS of the side the driveway is on. Cars can only use that side.
    pub facing: usize,
}

#[derive(Clone, Debug)]
pub struct Dest {
    /// Top-left tile of the footprint.
    pub origin: Pos,
    /// Footprint in tiles (width, height).
    pub size: (i32, i32),
    /// Driveway tiles along the front edge: one for a small shop, two for a big one.
    pub doors: Vec<Pos>,
    /// Sim time when it appeared, for the arrival animation.
    pub born: f32,
    pub color: usize,
    /// Trips wanted, including ones a car is already on its way to serve.
    pub pins: u32,
    /// Pins that already have a car assigned.
    pub assigned: u32,
    /// 0..1, game over at 1.
    pub overflow: f32,
    pub pin_timer: f32,
}

/// DIRS index for the main direction of travel from a to b (works for highway hops too).
pub fn dir_between(a: Pos, b: Pos) -> usize {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if dx.abs() >= dy.abs() {
        if dx >= 0 { 0 } else { 2 }
    } else if dy >= 0 {
        1
    } else {
        3
    }
}

impl Dest {
    pub fn big(&self) -> bool {
        self.doors.len() > 1
    }
    /// Pins a destination can hold before its overflow timer starts.
    pub fn pin_limit(&self) -> u32 {
        if self.big() {
            PIN_LIMIT + 2
        } else {
            PIN_LIMIT
        }
    }
}

/// Things the player can pick at the end of each week.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Upgrade {
    Bridge,
    Tunnel,
    Roundabout,
    TrafficLight,
    Highway,
    ExtraRoads,
}

impl Upgrade {
    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Bridge => "Bridge",
            Upgrade::Tunnel => "Tunnel",
            Upgrade::Roundabout => "Roundabout",
            Upgrade::TrafficLight => "Traffic light",
            Upgrade::Highway => "Highway",
            Upgrade::ExtraRoads => "Road tiles",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Upgrade::Bridge => "Build roads across 4 water tiles",
            Upgrade::Tunnel => "Build roads through 4 hill tiles",
            Upgrade::Roundabout => "Junction where cars never wait for each other",
            Upgrade::TrafficLight => "Lets two straight-through directions go at once",
            Upgrade::Highway => "Fast link between two points, over anything",
            Upgrade::ExtraRoads => "+20 road tiles",
        }
    }
}

/// Upgrades the player has but hasn't used yet.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Inventory {
    /// Water tiles that can still take a road.
    pub bridge_tiles: u32,
    /// Hill tiles that can still take a road.
    pub tunnel_tiles: u32,
    pub roundabouts: u32,
    pub lights: u32,
    pub highways: u32,
}

/// How a junction tile manages traffic.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Junction {
    /// Default: only one car in the crossing at a time.
    Plain,
    /// Each direction flows independently.
    Roundabout,
    /// Alternates between east-west and north-south traffic.
    Light,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Highway {
    pub a: Pos,
    pub b: Pos,
}

impl Highway {
    pub fn len(&self) -> f32 {
        (((self.a.0 - self.b.0).pow(2) + (self.a.1 - self.b.1).pow(2)) as f32).sqrt()
    }
}

pub const BRIDGE_TILES: u32 = 4;
pub const TUNNEL_TILES: u32 = 4;
pub const EXTRA_ROADS: u32 = 20;
pub const HIGHWAY_MAX_LEN: i32 = 14;
pub const HIGHWAY_SPEED: f32 = 2.2;
/// Seconds each traffic light phase lasts.
pub const LIGHT_PHASE: f32 = 3.0;

/// Things that happened during a step, for sounds and effects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    TripDone,
    NewHome,
    NewDest,
    /// A destination just started overflowing.
    Warning,
    WeekEnd,
    GameOver,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CarState {
    Idle,
    ToDest(usize),
    Unloading(usize),
    ToHome,
}

/// One car per home; `cars[i]` belongs to `homes[i]`.
#[derive(Clone, Debug)]
pub struct Car {
    pub state: CarState,
    pub path: Vec<Pos>,
    /// Index of `cur` in `path`.
    pub i: usize,
    /// 0..1 progress moving from `prev` into `cur`.
    pub progress: f32,
    pub prev: Pos,
    pub cur: Pos,
    /// Lane slot held on `cur` (road tiles only).
    pub slot: Option<usize>,
    /// Retry countdown when the car has no route.
    pub retry: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditResult {
    Ok,
    NoChange,
    NoRoadsLeft,
    NeedBridge,
    NeedTunnel,
    NoneLeft,
    Blocked,
}

pub struct Sim {
    pub map: MapKind,
    pub terrain: Vec<Terrain>,
    /// Junction control per tile (only meaningful on road tiles with 3+ links).
    pub junctions: Vec<Junction>,
    pub highways: Vec<Highway>,
    pub inventory: Inventory,
    /// Two upgrades offered at the end of a week; the sim waits until one is picked.
    pub offer: Option<[Upgrade; 2]>,
    /// Events since the caller last drained them.
    pub events: Vec<Event>,
    pub cells: Vec<Cell>,
    /// Lane occupancy: one flag per tile per direction of travel.
    pub occ: Vec<bool>,
    pub homes: Vec<Home>,
    pub cars: Vec<Car>,
    pub dests: Vec<Dest>,
    pub roads_left: u32,
    pub score: u32,
    pub time: f32,
    pub week: u32,
    pub game_over: bool,
    /// Destination whose overflow ended the game.
    pub failed_dest: Option<usize>,
    /// Short message to show the player and how long it has left.
    pub toast: Option<(String, f32)>,
    rng: Rng,
    home_timer: f32,
    dest_timer: f32,
    dispatch_timer: f32,
    roads_dirty: bool,
}

impl Sim {
    pub fn new(seed: u64) -> Self {
        Sim::new_on(MapKind::Bayside, seed)
    }

    pub fn new_on(map: MapKind, seed: u64) -> Self {
        let terrain = map::generate_kind(map, seed);
        let inventory = match map {
            MapKind::Bayside => Inventory { bridge_tiles: BRIDGE_TILES, ..Default::default() },
            MapKind::TwinIslands => Inventory { bridge_tiles: BRIDGE_TILES * 2, ..Default::default() },
            MapKind::Highland => Inventory { tunnel_tiles: TUNNEL_TILES, ..Default::default() },
        };
        let mut s = Sim {
            map,
            junctions: vec![Junction::Plain; (W * H) as usize],
            highways: Vec::new(),
            inventory,
            offer: None,
            events: Vec::new(),
            terrain,
            cells: vec![Cell::Empty; (W * H) as usize],
            occ: vec![false; (W * H * 4) as usize],
            homes: Vec::new(),
            cars: Vec::new(),
            dests: Vec::new(),
            roads_left: START_ROADS,
            score: 0,
            time: 0.0,
            week: 1,
            game_over: false,
            failed_dest: None,
            toast: Some(("Draw roads from homes to matching destinations".into(), 6.0)),
            rng: Rng::new(seed ^ 0xA5A5),
            home_timer: 12.0,
            dest_timer: 35.0,
            dispatch_timer: 0.0,
            roads_dirty: false,
        };
        // Opening layout: one destination near the middle with two homes.
        let center = (W / 2 - 6, H / 2);
        if let Some(d) = s.spawn_dest_near(0, center) {
            s.spawn_home(d);
            s.spawn_home(d);
        }
        s
    }

    /// An empty sim with all-grass terrain and nothing spawned, for tests.
    pub fn empty(seed: u64) -> Self {
        let mut s = Sim::new(seed);
        s.terrain = vec![Terrain::Grass; (W * H) as usize];
        s.cells = vec![Cell::Empty; (W * H) as usize];
        s.homes.clear();
        s.cars.clear();
        s.dests.clear();
        s.inventory = Inventory::default();
        s.home_timer = f32::INFINITY;
        s.dest_timer = f32::INFINITY;
        s
    }

    pub fn cell(&self, p: Pos) -> Cell {
        if in_bounds(p) {
            self.cells[idx(p)]
        } else {
            Cell::Dest(usize::MAX)
        }
    }

    pub fn colors_available(&self) -> usize {
        (3 + (self.week as usize - 1) / 3).min(MAX_COLORS)
    }

    // ----- Player edits -------------------------------------------------

    pub fn place_road(&mut self, p: Pos) -> EditResult {
        if !in_bounds(p) || self.game_over {
            return EditResult::Blocked;
        }
        match self.cells[idx(p)] {
            Cell::Road => return EditResult::NoChange,
            Cell::Empty => {}
            _ => return EditResult::Blocked,
        }
        match self.terrain[idx(p)] {
            Terrain::Grass => {
                if self.roads_left == 0 {
                    return EditResult::NoRoadsLeft;
                }
                self.roads_left -= 1;
            }
            Terrain::Water => {
                if self.inventory.bridge_tiles == 0 {
                    return EditResult::NeedBridge;
                }
                self.inventory.bridge_tiles -= 1;
            }
            Terrain::Hill => {
                if self.inventory.tunnel_tiles == 0 {
                    return EditResult::NeedTunnel;
                }
                self.inventory.tunnel_tiles -= 1;
            }
        }
        self.cells[idx(p)] = Cell::Road;
        self.roads_dirty = true;
        self.update_facings_near(p);
        EditResult::Ok
    }

    pub fn erase_road(&mut self, p: Pos) -> EditResult {
        if !in_bounds(p) || self.game_over {
            return EditResult::Blocked;
        }
        if self.cells[idx(p)] != Cell::Road {
            return EditResult::NoChange;
        }
        if self.tile_occupancy(p) > 0 {
            return EditResult::Blocked;
        }
        // Removing a highway end removes that highway and gives it back.
        let before = self.highways.len();
        self.highways.retain(|h| h.a != p && h.b != p);
        self.inventory.highways += (before - self.highways.len()) as u32;
        match self.junctions[idx(p)] {
            Junction::Roundabout => self.inventory.roundabouts += 1,
            Junction::Light => self.inventory.lights += 1,
            Junction::Plain => {}
        }
        self.junctions[idx(p)] = Junction::Plain;
        self.cells[idx(p)] = Cell::Empty;
        match self.terrain[idx(p)] {
            Terrain::Grass => self.roads_left += 1,
            Terrain::Water => self.inventory.bridge_tiles += 1,
            Terrain::Hill => self.inventory.tunnel_tiles += 1,
        }
        self.roads_dirty = true;
        self.update_facings_near(p);
        EditResult::Ok
    }

    /// Number of neighbours this road tile actually links to (roads, driveways, highways).
    pub fn link_count(&self, p: Pos) -> usize {
        let mut n = DIRS
            .iter()
            .filter(|d| {
                let q = (p.0 + d.0, p.1 + d.1);
                in_bounds(q)
                    && match self.cells[idx(q)] {
                        Cell::Road | Cell::Door(_) => true,
                        Cell::Home(h) => self.driveway(h) == p,
                        _ => false,
                    }
            })
            .count();
        n += self.highways.iter().filter(|h| h.a == p || h.b == p).count();
        n
    }

    pub fn is_crossing(&self, p: Pos) -> bool {
        in_bounds(p) && self.cells[idx(p)] == Cell::Road && self.link_count(p) >= 3
    }

    /// Whether a tile is one of the eight around a roundabout, which its ring spills into.
    pub fn near_roundabout(&self, p: Pos) -> bool {
        (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                let q = (p.0 + dx, p.1 + dy);
                (dx, dy) != (0, 0) && in_bounds(q) && self.cells[idx(q)] == Cell::Road && self.junctions[idx(q)] == Junction::Roundabout
            })
        })
    }

    /// Put a roundabout or traffic light on a road tile.
    pub fn place_junction(&mut self, p: Pos, kind: Junction) -> EditResult {
        if !in_bounds(p) || self.game_over || self.cells[idx(p)] != Cell::Road || kind == Junction::Plain {
            return EditResult::Blocked;
        }
        // A roundabout's ring needs the corners around it clear of buildings.
        if kind == Junction::Roundabout {
            let crowded = [(-1, -1), (1, -1), (-1, 1), (1, 1)].iter().any(|d| {
                matches!(self.cell((p.0 + d.0, p.1 + d.1)), Cell::Home(_) | Cell::Dest(_) | Cell::Door(_) | Cell::Park(_))
            });
            if crowded {
                return EditResult::Blocked;
            }
        }
        if self.junctions[idx(p)] == kind {
            return EditResult::NoChange;
        }
        let have = match kind {
            Junction::Roundabout => &mut self.inventory.roundabouts,
            _ => &mut self.inventory.lights,
        };
        if *have == 0 {
            return EditResult::NoneLeft;
        }
        *have -= 1;
        match self.junctions[idx(p)] {
            Junction::Roundabout => self.inventory.roundabouts += 1,
            Junction::Light => self.inventory.lights += 1,
            Junction::Plain => {}
        }
        self.junctions[idx(p)] = kind;
        EditResult::Ok
    }

    /// Whether a highway may run between a and b.
    pub fn highway_ok(&self, a: Pos, b: Pos) -> bool {
        let ok_end = |p: Pos| {
            in_bounds(p)
                && self.terrain[idx(p)] == Terrain::Grass
                && matches!(self.cells[idx(p)], Cell::Empty | Cell::Road)
        };
        let d = (a.0 - b.0).abs().max((a.1 - b.1).abs());
        a != b && d >= 2 && d <= HIGHWAY_MAX_LEN && ok_end(a) && ok_end(b)
            && !self.highways.iter().any(|h| (h.a == a && h.b == b) || (h.a == b && h.b == a))
    }

    /// Build an elevated highway between two tiles. Its ends become free road tiles.
    pub fn place_highway(&mut self, a: Pos, b: Pos) -> EditResult {
        if self.game_over || !self.highway_ok(a, b) {
            return EditResult::Blocked;
        }
        if self.inventory.highways == 0 {
            return EditResult::NoneLeft;
        }
        self.inventory.highways -= 1;
        for p in [a, b] {
            if self.cells[idx(p)] == Cell::Empty {
                self.cells[idx(p)] = Cell::Road;
                self.update_facings_near(p);
            }
        }
        self.highways.push(Highway { a, b });
        self.roads_dirty = true;
        EditResult::Ok
    }

    /// Choose one of the two offered upgrades.
    pub fn choose(&mut self, i: usize) {
        let Some(offer) = self.offer.take() else { return };
        let u = offer[i.min(1)];
        let inv = &mut self.inventory;
        match u {
            Upgrade::Bridge => inv.bridge_tiles += BRIDGE_TILES,
            Upgrade::Tunnel => inv.tunnel_tiles += TUNNEL_TILES,
            Upgrade::Roundabout => inv.roundabouts += 1,
            Upgrade::TrafficLight => inv.lights += 1,
            Upgrade::Highway => inv.highways += 1,
            Upgrade::ExtraRoads => self.roads_left += EXTRA_ROADS,
        }
        self.toast = Some((format!("{} added", u.name()), 2.5));
    }

    fn make_offer(&mut self) -> [Upgrade; 2] {
        let has = |t: Terrain| self.terrain.iter().any(|x| *x == t);
        let mut pool = vec![Upgrade::Roundabout, Upgrade::TrafficLight, Upgrade::Highway, Upgrade::ExtraRoads];
        if has(Terrain::Water) {
            pool.push(Upgrade::Bridge);
        }
        if has(Terrain::Hill) {
            pool.push(Upgrade::Tunnel);
        }
        let a = pool.swap_remove(self.rng.below(pool.len()));
        let b = pool.swap_remove(self.rng.below(pool.len()));
        [a, b]
    }

    /// Whether the light at `p` currently lets traffic heading along `dir` (DIRS index) through.
    pub fn light_green(&self, p: Pos, dir: usize) -> bool {
        let phase = ((self.time / LIGHT_PHASE) as i64 + (p.0 + p.1) as i64) % 2;
        dir % 2 == phase as usize
    }

    fn tile_occupancy(&self, p: Pos) -> u32 {
        let b = idx(p) * 4;
        self.occ[b..b + 4].iter().filter(|o| **o).count() as u32
    }

    // ----- Spawning ----------------------------------------------------

    fn footprint_free(&self, origin: Pos, size: (i32, i32), doors: &[Pos]) -> bool {
        // The footprint, its driveways and a one-tile margin must be clear of buildings,
        // and the footprint and driveways must sit on empty grass.
        for y in origin.1 - 1..=origin.1 + size.1 {
            for x in origin.0 - 1..=origin.0 + size.0 {
                let p = (x, y);
                if !in_bounds(p) {
                    return false;
                }
                let inside = x >= origin.0 && x < origin.0 + size.0 && y >= origin.1 && y < origin.1 + size.1;
                let c = self.cells[idx(p)];
                if (inside || doors.contains(&p)) && self.near_roundabout(p) {
                    return false;
                }
                if inside || doors.contains(&p) {
                    if c != Cell::Empty || self.terrain[idx(p)] != Terrain::Grass {
                        return false;
                    }
                } else if !matches!(c, Cell::Empty | Cell::Road) {
                    return false;
                }
            }
        }
        true
    }

    fn place_dest(&mut self, color: usize, origin: Pos, big: bool) -> Option<usize> {
        let size = if big { BIG_SHOP } else { SMALL_SHOP };
        let front = origin.1 + size.1;
        let doors: Vec<Pos> = if big {
            vec![(origin.0, front), (origin.0 + size.0 - 1, front)]
        } else {
            vec![(origin.0 + 1, front)]
        };
        let lot: Vec<Pos> = (origin.0..origin.0 + size.0).map(|x| (x, front)).collect();
        if !self.footprint_free(origin, size, &lot) {
            return None;
        }
        let id = self.dests.len();
        for y in origin.1..origin.1 + size.1 {
            for x in origin.0..origin.0 + size.0 {
                self.cells[idx((x, y))] = Cell::Dest(id);
            }
        }
        for p in &lot {
            self.cells[idx(*p)] = if doors.contains(p) { Cell::Door(id) } else { Cell::Park(id) };
        }
        self.dests.push(Dest {
            born: self.time,
            origin,
            size,
            doors,
            color,
            pins: 0,
            assigned: 0,
            overflow: 0.0,
            pin_timer: self.rng.range_f32(3.0, 6.0),
        });
        self.roads_dirty = true;
        Some(id)
    }

    /// Try to place a destination of `color` close to `near`, searching outward.
    pub fn spawn_dest_near(&mut self, color: usize, near: Pos) -> Option<usize> {
        for r in 0..20 {
            let mut options = Vec::new();
            for y in near.1 - r..=near.1 + r {
                for x in near.0 - r..=near.0 + r {
                    if (x - near.0).abs().max((y - near.1).abs()) == r {
                        options.push((x, y));
                    }
                }
            }
            while !options.is_empty() {
                let o = options.swap_remove(self.rng.below(options.len()));
                if let Some(id) = self.place_dest(color, o, false) {
                    return Some(id);
                }
            }
        }
        None
    }

    fn spawn_random_dest(&mut self) -> Option<usize> {
        // Prefer the available colour with the fewest destinations.
        let n = self.colors_available();
        let mut counts = vec![0usize; n];
        for d in &self.dests {
            if d.color < n {
                counts[d.color] += 1;
            }
        }
        let min = *counts.iter().min().unwrap_or(&0);
        let choices: Vec<usize> = (0..n).filter(|c| counts[*c] == min).collect();
        let color = choices[self.rng.below(choices.len())];
        // Big shops (two driveways, busier) start appearing from week 2.
        let big = self.week >= 2 && self.rng.f32() < 0.35;
        let size = if big { BIG_SHOP } else { SMALL_SHOP };
        for _ in 0..200 {
            let o = (
                1 + self.rng.below((W - size.0 - 2) as usize) as i32,
                1 + self.rng.below((H - size.1 - 3) as usize) as i32,
            );
            // Keep destinations apart so the map stays readable.
            let far = self.dests.iter().all(|d| {
                (d.origin.0 - o.0).abs() + (d.origin.1 - o.1).abs() >= 7
            });
            if far {
                if let Some(id) = self.place_dest(color, o, big) {
                    return Some(id);
                }
            }
        }
        None
    }

    /// Place a home of the destination's colour a few tiles away from it.
    pub fn spawn_home(&mut self, dest: usize) -> Option<usize> {
        let d = &self.dests[dest];
        let (door, color) = (d.doors[0], d.color);
        let mut options = Vec::new();
        for y in door.1 - 9..=door.1 + 9 {
            for x in door.0 - 12..=door.0 + 12 {
                let p = (x, y);
                let dist = (x - door.0).abs() + (y - door.1).abs();
                if !(4..=12).contains(&dist) || !in_bounds(p) {
                    continue;
                }
                if self.cells[idx(p)] != Cell::Empty || self.terrain[idx(p)] != Terrain::Grass || self.near_roundabout(p) {
                    continue;
                }
                // Needs at least two open sides and no building directly next to it.
                let mut open = 0;
                let mut crowded = false;
                for dd in DIRS {
                    let n = (p.0 + dd.0, p.1 + dd.1);
                    match self.cell(n) {
                        Cell::Empty | Cell::Road if self.terrain[idx(n)] == Terrain::Grass => open += 1,
                        Cell::Empty | Cell::Road => {}
                        _ => crowded = true,
                    }
                }
                if open >= 2 && !crowded {
                    options.push(p);
                }
            }
        }
        if options.is_empty() {
            return None;
        }
        let p = options[self.rng.below(options.len())];
        Some(self.add_home_facing(p, color, Some(door)))
    }

    /// Place a home directly, for tests and scripted setups.
    pub fn add_home(&mut self, p: Pos, color: usize) -> usize {
        self.add_home_facing(p, color, None)
    }

    /// Place a home. Its driveway starts on the open side facing `toward` (if given),
    /// and turns later to meet the first road the player draws beside it.
    fn add_home_facing(&mut self, p: Pos, color: usize, toward: Option<Pos>) -> usize {
        let id = self.homes.len();
        self.cells[idx(p)] = Cell::Home(id);
        let mut facing = 1;
        let mut best = i32::MAX;
        for (k, d) in DIRS.iter().enumerate() {
            let n = (p.0 + d.0, p.1 + d.1);
            let open = in_bounds(n) && matches!(self.cells[idx(n)], Cell::Empty | Cell::Road) && self.terrain[idx(n)] == Terrain::Grass;
            if !open {
                continue;
            }
            let score = match toward {
                Some(t) => (t.0 - n.0).abs() + (t.1 - n.1).abs(),
                None => if k == 1 { 0 } else { 1 },
            };
            if score < best {
                best = score;
                facing = k;
            }
        }
        self.homes.push(Home { pos: p, color, born: self.time, facing });
        self.update_facing(id);
        self.cars.push(Car {
            state: CarState::Idle,
            path: vec![p],
            i: 0,
            progress: 1.0,
            prev: p,
            cur: p,
            slot: None,
            retry: 0.0,
        });
        id
    }

    /// Place a destination directly, for tests and scripted setups.
    pub fn add_dest(&mut self, origin: Pos, color: usize) -> Option<usize> {
        self.place_dest(color, origin, false)
    }

    /// Place a big two-driveway shop directly, for tests and scripted setups.
    pub fn add_big_dest(&mut self, origin: Pos, color: usize) -> Option<usize> {
        self.place_dest(color, origin, true)
    }

    /// The tile a home's driveway leads to.
    pub fn driveway(&self, h: usize) -> Pos {
        let home = &self.homes[h];
        let d = DIRS[home.facing];
        (home.pos.0 + d.0, home.pos.1 + d.1)
    }

    /// Keep a home's driveway if a road meets it; otherwise turn it toward a road beside the house.
    fn update_facing(&mut self, h: usize) {
        let p = self.homes[h].pos;
        let joined = |s: &Self, n: Pos| in_bounds(n) && matches!(s.cells[idx(n)], Cell::Road);
        if joined(self, self.driveway(h)) {
            return;
        }
        // Don't swing the driveway around while the family car is out.
        if self.cars.get(h).is_some_and(|c| c.state != CarState::Idle) {
            return;
        }
        for k in 0..4 {
            let d = DIRS[k];
            if joined(self, (p.0 + d.0, p.1 + d.1)) {
                self.homes[h].facing = k;
                return;
            }
        }
    }

    fn update_facings_near(&mut self, p: Pos) {
        for d in DIRS {
            if let Cell::Home(h) = self.cell((p.0 + d.0, p.1 + d.1)) {
                self.update_facing(h);
            }
        }
    }

    /// Whether a car may move between neighbouring tiles a and b (houses only via their driveway).
    fn link_ok(&self, a: Pos, b: Pos) -> bool {
        let ok = |h: Pos, other: Pos| match self.cells[idx(h)] {
            Cell::Home(id) => self.driveway(id) == other,
            // A car park only joins its own shop's car park and driveways.
            Cell::Park(id) => matches!(self.cells[idx(other)], Cell::Park(j) | Cell::Door(j) if j == id),
            _ => true,
        };
        ok(a, b) && ok(b, a)
    }

    /// Tiles a car may drive across on its way somewhere else: roads, and the car park and
    /// driveways of a big shop (which link its two driveways).
    pub fn passable(&self, p: Pos) -> bool {
        match self.cells[idx(p)] {
            Cell::Road => true,
            Cell::Park(d) | Cell::Door(d) => self.dests[d].big(),
            _ => false,
        }
    }

    // ----- Pathfinding -------------------------------------------------

    /// Cheapest route from `start` to the first tile satisfying `is_goal`.
    /// Only roads can be passed through; homes and doors can only be the start or the goal.
    /// Busy tiles cost more, so cars spread over alternative routes.
    pub fn find_path(&self, start: Pos, is_goal: &dyn Fn(Pos) -> bool) -> Option<Vec<Pos>> {
        let n = (W * H) as usize;
        let mut dist = vec![u32::MAX; n];
        let mut from = vec![usize::MAX; n];
        let mut heap = BinaryHeap::new();
        dist[idx(start)] = 0;
        heap.push(Reverse((0u32, idx(start))));
        while let Some(Reverse((d, i))) = heap.pop() {
            if d > dist[i] {
                continue;
            }
            let p = ((i as i32) % W, (i as i32) / W);
            if p != start && is_goal(p) {
                let mut path = vec![p];
                let mut c = i;
                while from[c] != usize::MAX {
                    c = from[c];
                    path.push(((c as i32) % W, (c as i32) / W));
                }
                path.reverse();
                return Some(path);
            }
            if p != start && !self.passable(p) {
                continue;
            }
            let mut nbrs: Vec<(Pos, u32)> = DIRS.iter().map(|dd| ((p.0 + dd.0, p.1 + dd.1), 10)).collect();
            for h in &self.highways {
                let len = (h.len() * 10.0 / HIGHWAY_SPEED) as u32;
                if h.a == p {
                    nbrs.push((h.b, len));
                } else if h.b == p {
                    nbrs.push((h.a, len));
                }
            }
            for (q, step) in nbrs {
                if !in_bounds(q) {
                    continue;
                }
                let qi = idx(q);
                let enterable = self.passable(q) || is_goal(q);
                if !enterable || !self.link_ok(p, q) {
                    continue;
                }
                let crossing = if self.junctions[qi] == Junction::Plain && self.is_crossing(q) { 4 } else { 0 };
                let cost = d + step + crossing + 6 * self.tile_occupancy(q);
                if cost < dist[qi] {
                    dist[qi] = cost;
                    from[qi] = i;
                    heap.push(Reverse((cost, qi)));
                }
            }
        }
        None
    }

    fn route_car(&mut self, c: usize) {
        let car = &self.cars[c];
        let start = car.cur;
        let path = match car.state {
            CarState::ToDest(d) => {
                let doors = self.dests[d].doors.clone();
                self.find_path(start, &|p| doors.contains(&p))
            }
            CarState::ToHome => {
                let home = self.homes[c].pos;
                if start == home {
                    Some(vec![home])
                } else {
                    self.find_path(start, &|p| p == home)
                }
            }
            _ => return,
        };
        match path {
            Some(p) => {
                let car = &mut self.cars[c];
                car.path = p;
                car.i = 0;
            }
            None => {
                if let CarState::ToDest(d) = self.cars[c].state {
                    // Can't reach the destination any more: release the trip and head home.
                    self.dests[d].assigned -= 1;
                    self.cars[c].state = CarState::ToHome;
                    self.route_car(c);
                } else {
                    let car = &mut self.cars[c];
                    car.path = vec![car.cur];
                    car.i = 0;
                    car.retry = 1.0;
                }
            }
        }
    }

    // ----- Update ------------------------------------------------------

    pub fn step(&mut self) {
        if self.game_over || self.offer.is_some() {
            return;
        }
        let dt = DT;
        self.time += dt;

        if let Some((_, t)) = &mut self.toast {
            *t -= dt;
            if *t <= 0.0 {
                self.toast = None;
            }
        }

        if self.time >= self.week as f32 * WEEK_LENGTH {
            self.week += 1;
            self.roads_left += ROADS_PER_WEEK;
            self.toast = Some((format!("Week {}: +{} road tiles", self.week, ROADS_PER_WEEK), 4.0));
            self.offer = Some(self.make_offer());
            self.events.push(Event::WeekEnd);
        }

        if self.roads_dirty {
            self.roads_dirty = false;
            for c in 0..self.cars.len() {
                if matches!(self.cars[c].state, CarState::ToDest(_) | CarState::ToHome) {
                    self.route_car(c);
                }
            }
        }

        self.update_spawning(dt);
        self.update_pins(dt);

        self.dispatch_timer -= dt;
        if self.dispatch_timer <= 0.0 {
            self.dispatch_timer = 0.25;
            self.dispatch();
        }

        for c in 0..self.cars.len() {
            self.update_car(c, dt);
        }
    }

    fn update_spawning(&mut self, dt: f32) {
        let pace = 1.0 + 0.15 * (self.week as f32 - 1.0);
        self.home_timer -= dt * pace;
        if self.home_timer <= 0.0 && !self.dests.is_empty() {
            self.home_timer = self.rng.range_f32(10.0, 16.0);
            // Favour destinations with few homes nearby.
            let mut best = 0;
            let mut best_score = f32::MAX;
            for (i, d) in self.dests.iter().enumerate() {
                let homes = self.homes.iter().filter(|h| h.color == d.color).count() as f32;
                let dests = self.dests.iter().filter(|e| e.color == d.color).count() as f32;
                let s = homes / dests + self.rng.f32();
                if s < best_score {
                    best_score = s;
                    best = i;
                }
            }
            if self.spawn_home(best).is_some() {
                self.events.push(Event::NewHome);
            }
        }
        self.dest_timer -= dt * pace;
        if self.dest_timer <= 0.0 {
            self.dest_timer = self.rng.range_f32(40.0, 55.0);
            if let Some(d) = self.spawn_random_dest() {
                self.spawn_home(d);
                self.toast = Some(("A new destination has opened".into(), 3.0));
                self.events.push(Event::NewDest);
            }
        }
    }

    fn update_pins(&mut self, dt: f32) {
        let interval = (8.0 - 0.6 * (self.week as f32 - 1.0)).max(2.5);
        for i in 0..self.dests.len() {
            let jitter = self.rng.range_f32(0.7, 1.3);
            let d = &mut self.dests[i];
            d.pin_timer -= dt;
            if d.pin_timer <= 0.0 {
                d.pin_timer = interval * jitter / if d.big() { 1.6 } else { 1.0 };
                d.pins += 1;
            }
            if d.pins > d.pin_limit() {
                if d.overflow == 0.0 {
                    self.events.push(Event::Warning);
                }
                d.overflow += dt / OVERFLOW_TIME;
            } else {
                d.overflow = (d.overflow - dt / (OVERFLOW_TIME * 0.5)).max(0.0);
            }
            if d.overflow >= 1.0 && !self.game_over {
                self.events.push(Event::GameOver);
                self.game_over = true;
                self.failed_dest = Some(i);
            }
        }
    }

    fn dispatch(&mut self) {
        for d in 0..self.dests.len() {
            while self.dests[d].pins > self.dests[d].assigned {
                let color = self.dests[d].color;
                // Search from each driveway and keep the nearest idle matching car.
                let mut found: Option<Vec<Pos>> = None;
                for door in self.dests[d].doors.clone() {
                    let cells = &self.cells;
                    let homes = &self.homes;
                    let cars = &self.cars;
                    let p = self.find_path(door, &|p| match cells[idx(p)] {
                        Cell::Home(h) => homes[h].color == color && cars[h].state == CarState::Idle,
                        _ => false,
                    });
                    if let Some(p) = p {
                        if found.as_ref().is_none_or(|f| p.len() < f.len()) {
                            found = Some(p);
                        }
                    }
                }
                let Some(mut path) = found else { break };
                path.reverse();
                let h = match self.cells[idx(path[0])] {
                    Cell::Home(h) => h,
                    _ => unreachable!(),
                };
                let car = &mut self.cars[h];
                car.state = CarState::ToDest(d);
                car.path = path;
                car.i = 0;
                car.progress = 1.0;
                car.prev = car.cur;
                self.dests[d].assigned += 1;
            }
        }
    }

    pub(crate) fn update_car(&mut self, c: usize, dt: f32) {
        match self.cars[c].state {
            CarState::Idle => {}
            CarState::Unloading(d) => {
                self.cars[c].retry -= dt;
                if self.cars[c].retry <= 0.0 {
                    let dest = &mut self.dests[d];
                    dest.pins -= 1;
                    dest.assigned -= 1;
                    self.score += 1;
                    self.events.push(Event::TripDone);
                    self.cars[c].state = CarState::ToHome;
                    self.route_car(c);
                }
            }
            CarState::ToDest(_) | CarState::ToHome => {
                let car = &mut self.cars[c];
                if car.path.len() <= 1 && car.cur != self.homes[c].pos && car.retry > 0.0 {
                    car.retry -= dt;
                    if car.retry <= 0.0 {
                        self.route_car(c);
                    }
                    return;
                }
                let seg = (((car.cur.0 - car.prev.0).pow(2) + (car.cur.1 - car.prev.1).pow(2)) as f32).sqrt().max(1.0);
                let speed = if seg > 1.5 { CAR_SPEED * HIGHWAY_SPEED / seg } else { CAR_SPEED };
                car.progress = (car.progress + speed * dt).min(1.0);
                if car.progress < 1.0 {
                    return;
                }
                if car.i + 1 >= car.path.len() {
                    self.arrive(c);
                    return;
                }
                let next = car.path[car.i + 1];
                let dir = dir_between(car.cur, next);
                let next_slot = if self.cells[idx(next)] == Cell::Road {
                    Some(idx(next) * 4 + dir)
                } else {
                    None
                };
                if let Some(s) = next_slot {
                    if self.occ[s] {
                        return; // queue behind the car ahead
                    }
                    if self.is_crossing(next) {
                        let b = idx(next) * 4;
                        let busy = self.occ[b..b + 4].iter().any(|o| *o);
                        let wait = match self.junctions[idx(next)] {
                            Junction::Plain => busy,
                            Junction::Roundabout => false,
                            Junction::Light => !self.light_green(next, dir),
                        };
                        if wait {
                            return; // wait at the junction
                        }
                    }
                    let car = &mut self.cars[c];
                    let _ = car;
                    self.occ[s] = true;
                }
                let car = &mut self.cars[c];
                if let Some(s) = car.slot {
                    self.occ[s] = false;
                }
                car.slot = next_slot;
                car.prev = car.cur;
                car.cur = next;
                car.i += 1;
                car.progress = 0.0;
            }
        }
    }

    fn arrive(&mut self, c: usize) {
        let car = &mut self.cars[c];
        if let Some(s) = car.slot.take() {
            self.occ[s] = false;
        }
        match car.state {
            CarState::ToDest(d) => {
                car.state = CarState::Unloading(d);
                car.retry = UNLOAD_TIME;
            }
            CarState::ToHome => {
                if car.cur == self.homes[c].pos {
                    car.state = CarState::Idle;
                    car.prev = car.cur;
                    car.path = vec![car.cur];
                    car.i = 0;
                } else {
                    car.retry = 1.0;
                }
            }
            _ => {}
        }
    }

    /// Fraction of the current week that has passed.
    pub fn week_progress(&self) -> f32 {
        let start = (self.week - 1) as f32 * WEEK_LENGTH;
        ((self.time - start) / WEEK_LENGTH).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &mut Sim, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            s.step();
        }
    }

    #[test]
    fn new_game_has_a_destination_and_two_homes() {
        let s = Sim::new(7);
        assert_eq!(s.dests.len(), 1);
        assert_eq!(s.homes.len(), 2);
        assert_eq!(s.roads_left, START_ROADS);
    }

    #[test]
    fn roads_cost_tiles_and_refund_on_erase() {
        let mut s = Sim::empty(1);
        assert_eq!(s.place_road((5, 5)), EditResult::Ok);
        assert_eq!(s.place_road((5, 5)), EditResult::NoChange);
        assert_eq!(s.roads_left, START_ROADS - 1);
        assert_eq!(s.erase_road((5, 5)), EditResult::Ok);
        assert_eq!(s.roads_left, START_ROADS);
    }

    #[test]
    fn cannot_build_on_water() {
        let mut s = Sim::empty(1);
        s.terrain[idx((3, 3))] = Terrain::Water;
        assert_eq!(s.place_road((3, 3)), EditResult::NeedBridge);
    }

    #[test]
    fn running_out_of_roads() {
        let mut s = Sim::empty(1);
        s.roads_left = 1;
        assert_eq!(s.place_road((1, 1)), EditResult::Ok);
        assert_eq!(s.place_road((2, 1)), EditResult::NoRoadsLeft);
    }

    /// Destination at (10,5) has its door at (11,7). Home at (11,12). Road (11,8)..(11,11).
    fn connected_setup() -> Sim {
        let mut s = Sim::empty(3);
        let d = s.add_dest((10, 5), 0).unwrap();
        assert_eq!(s.dests[d].doors, vec![(11, 7)]);
        s.add_home((11, 12), 0);
        for y in 8..=11 {
            assert_eq!(s.place_road((11, y)), EditResult::Ok);
        }
        s
    }

    #[test]
    fn connected_home_serves_destination() {
        let mut s = connected_setup();
        s.dests[0].pins = 2;
        run(&mut s, 12.0);
        assert!(s.score >= 2, "score was {}", s.score);
    }

    #[test]
    fn unconnected_home_does_not_serve() {
        let mut s = Sim::empty(3);
        s.add_dest((10, 5), 0).unwrap();
        s.add_home((11, 12), 0);
        s.dests[0].pins = 2;
        run(&mut s, 10.0);
        assert_eq!(s.score, 0);
        assert_eq!(s.cars[0].state, CarState::Idle);
    }

    #[test]
    fn wrong_colour_home_does_not_serve() {
        let mut s = Sim::empty(3);
        s.add_dest((10, 5), 0).unwrap();
        s.add_home((11, 12), 1);
        for y in 8..=11 {
            s.place_road((11, y));
        }
        s.dests[0].pins = 1;
        run(&mut s, 10.0);
        assert_eq!(s.score, 0);
    }

    #[test]
    fn overflow_ends_the_game() {
        let mut s = Sim::empty(3);
        s.add_dest((10, 5), 0).unwrap();
        s.dests[0].pins = PIN_LIMIT + 3;
        run(&mut s, OVERFLOW_TIME + 2.0);
        assert!(s.game_over);
        assert_eq!(s.failed_dest, Some(0));
    }

    #[test]
    fn cutting_a_road_mid_trip_sends_car_home() {
        let mut s = connected_setup();
        s.dests[0].pins = 1;
        run(&mut s, 0.6);
        assert!(matches!(s.cars[0].state, CarState::ToDest(_)));
        assert_ne!(s.cars[0].cur, (11, 8));
        assert_eq!(s.erase_road((11, 8)), EditResult::Ok);
        run(&mut s, 0.1);
        assert_eq!(s.dests[0].assigned, 0);
        assert_eq!(s.cars[0].state, CarState::ToHome);
        run(&mut s, 5.0);
        assert_eq!(s.cars[0].state, CarState::Idle);
        assert_eq!(s.score, 0);
    }

    #[test]
    fn cars_queue_instead_of_overlapping() {
        let mut s = Sim::empty(4);
        s.add_dest((10, 2), 0).unwrap(); // door (11,4)
        for y in 5..=14 {
            s.place_road((11, y));
        }
        for x in 12..=16 {
            s.place_road((x, 14));
        }
        for x in 12..=16 {
            s.add_home((x, 15), 0);
        }
        s.dests[0].pins = 5;
        for _ in 0..(8.0 / DT) as usize {
            s.step();
            // No two cars ever hold the same lane slot.
            let mut seen = std::collections::HashSet::new();
            for c in &s.cars {
                if let Some(sl) = c.slot {
                    assert!(seen.insert(sl), "two cars share a lane slot");
                }
            }
        }
        run(&mut s, 20.0);
        assert!(s.score >= 5, "score was {}", s.score);
    }

    #[test]
    fn house_turns_to_face_the_first_road() {
        let mut s = Sim::empty(5);
        let h = s.add_home((10, 10), 0);
        assert_eq!(s.homes[h].facing, 1); // starts facing down
        s.place_road((9, 10)); // road on the left
        assert_eq!(s.driveway(h), (9, 10));
        // A second road elsewhere doesn't steal the driveway.
        s.place_road((11, 10));
        assert_eq!(s.driveway(h), (9, 10));
        // Removing the joined road lets it turn to the remaining one.
        s.erase_road((9, 10));
        assert_eq!(s.driveway(h), (11, 10));
    }

    #[test]
    fn cars_only_use_the_driveway() {
        // Road passes beside the house on two sides but only one is the driveway.
        let mut s = Sim::empty(6);
        s.add_dest((10, 5), 0).unwrap(); // driveway (11,7)
        let h = s.add_home((12, 10), 0);
        for y in 8..=11 {
            s.place_road((11, y));
        }
        assert_eq!(s.driveway(h), (11, 10));
        // A road on the house's other side, not connected to the shop, is ignored.
        s.place_road((13, 10));
        assert_eq!(s.driveway(h), (11, 10));
        s.dests[0].pins = 1;
        run(&mut s, 8.0);
        assert!(s.score >= 1);
        // Cars never stepped onto the non-driveway road.
        assert_eq!(s.occ[idx((13, 10)) * 4..idx((13, 10)) * 4 + 4], [false; 4]);
    }

    #[test]
    fn a_path_cannot_cut_through_a_house() {
        let mut s = Sim::empty(7);
        let h = s.add_home((10, 10), 0);
        s.place_road((9, 10));
        s.place_road((11, 10));
        assert_eq!(s.driveway(h), (9, 10));
        assert!(s.find_path((9, 10), &|p| p == (11, 10)).is_none());
    }

    #[test]
    fn big_shop_has_two_driveways_and_either_works() {
        let mut s = Sim::empty(8);
        let d = s.add_big_dest((10, 5), 0).unwrap();
        assert_eq!(s.dests[d].doors, vec![(10, 7), (13, 7)]);
        assert!(s.dests[d].big());
        // Home connected only to the right-hand driveway.
        s.add_home((13, 11), 0);
        for y in 8..=10 {
            s.place_road((13, y));
        }
        s.dests[d].pins = 2;
        run(&mut s, 10.0);
        assert!(s.score >= 2, "score was {}", s.score);
    }

    #[test]
    fn shops_have_a_car_park_and_big_ones_can_be_driven_through() {
        let mut s = Sim::empty(8);
        let small = s.add_dest((3, 3), 1).unwrap();
        assert_eq!(s.cell((3, 5)), Cell::Park(small));
        assert_eq!(s.cell((4, 5)), Cell::Door(small));
        assert_eq!(s.cell((5, 5)), Cell::Park(small));
        let big = s.add_big_dest((10, 5), 2).unwrap();
        assert_eq!(s.cell((11, 7)), Cell::Park(big));
        // A teal home east of the big shop; its own shop lies west, reachable only by
        // driving in one driveway, across the car park and out the other.
        s.add_home((16, 7), 0);
        s.place_road((14, 7));
        s.place_road((15, 7));
        let target = s.add_dest((3, 9), 0).unwrap();
        for x in 7..=9 {
            s.place_road((x, 7));
        }
        for y in 8..=12 {
            s.place_road((7, y));
        }
        for x in 4..=6 {
            s.place_road((x, 12));
        }
        assert_eq!(s.cell((4, 11)), Cell::Door(target));
        // A road beside the car park doesn't connect into it except at a driveway.
        s.place_road((11, 8));
        let path = s.find_path((16, 7), &|p| p == (4, 11)).expect("route through the car park");
        assert!(path.contains(&(12, 7)) && path.contains(&(11, 7)), "{:?}", path);
        assert!(!path.contains(&(11, 8)));
        s.dests[target].pins = 2;
        s.dests[big].pins = 0;
        run(&mut s, 20.0);
        assert!(s.score >= 1, "score was {}", s.score);
    }

    #[test]
    fn roundabouts_need_clear_corners_and_keep_them_clear() {
        let mut s = Sim::empty(14);
        for x in 4..=8 {
            s.place_road((x, 6));
        }
        s.inventory.roundabouts = 2;
        s.add_home((5, 5), 0);
        assert_eq!(s.place_junction((6, 6), Junction::Roundabout), EditResult::Blocked);
        assert_eq!(s.place_junction((8, 6), Junction::Roundabout), EditResult::Ok);
        assert!(s.near_roundabout((9, 7)));
        // Shops won't open in its corners either.
        assert_eq!(s.add_dest((8, 7), 1), None);
    }

    #[test]
    fn bridges_and_tunnels_use_their_own_allowance() {
        let mut s = Sim::empty(9);
        s.terrain[idx((5, 5))] = Terrain::Water;
        s.terrain[idx((6, 5))] = Terrain::Hill;
        s.inventory.bridge_tiles = 1;
        assert_eq!(s.place_road((5, 5)), EditResult::Ok);
        assert_eq!(s.place_road((6, 5)), EditResult::NeedTunnel);
        assert_eq!(s.roads_left, START_ROADS);
        assert_eq!(s.inventory.bridge_tiles, 0);
        assert_eq!(s.erase_road((5, 5)), EditResult::Ok);
        assert_eq!(s.inventory.bridge_tiles, 1);
    }

    #[test]
    fn week_end_offers_two_upgrades_and_waits() {
        let mut s = Sim::new(12);
        for d in &mut s.dests {
            d.pins = 0;
        }
        s.time = WEEK_LENGTH - DT * 0.5;
        s.step();
        let offer = s.offer.expect("an offer at week end");
        assert_ne!(offer[0], offer[1]);
        let t = s.time;
        s.step();
        assert_eq!(s.time, t, "sim pauses until a choice is made");
        let before = (s.inventory, s.roads_left);
        s.choose(1);
        assert!(s.offer.is_none());
        assert_ne!((s.inventory, s.roads_left), before);
    }

    /// A plus-shaped crossing at (10,10) with roads one tile long on each arm.
    fn crossing() -> Sim {
        let mut s = Sim::empty(13);
        for p in [(10, 10), (9, 10), (11, 10), (10, 9), (10, 11)] {
            s.place_road(p);
        }
        s
    }

    #[test]
    fn plain_crossing_lets_one_car_through() {
        let mut s = crossing();
        assert!(s.is_crossing((10, 10)));
        // A car already crossing eastbound blocks a northbound one.
        s.occ[idx((10, 10)) * 4] = true;
        s.add_home((10, 12), 0);
        s.homes[0].facing = 3;
        let c = &mut s.cars[0];
        c.state = CarState::ToHome;
        c.path = vec![(10, 11), (10, 10), (10, 9)];
        c.cur = (10, 11);
        c.prev = (10, 12);
        c.i = 0;
        c.progress = 1.0;
        s.update_car(0, DT);
        assert_eq!(s.cars[0].cur, (10, 11), "waits at a busy plain crossing");
        // With a roundabout it goes straight in.
        s.inventory.roundabouts = 1;
        assert_eq!(s.place_junction((10, 10), Junction::Roundabout), EditResult::Ok);
        s.update_car(0, DT);
        assert_eq!(s.cars[0].cur, (10, 10));
    }

    #[test]
    fn traffic_light_alternates_directions() {
        let mut s = crossing();
        s.inventory.lights = 1;
        assert_eq!(s.place_junction((10, 10), Junction::Light), EditResult::Ok);
        let a = s.light_green((10, 10), 0);
        assert_ne!(a, s.light_green((10, 10), 1));
        s.time += LIGHT_PHASE;
        assert_ne!(a, s.light_green((10, 10), 0));
        // Erasing the tile gives the light back.
        s.erase_road((10, 10));
        assert_eq!(s.inventory.lights, 1);
    }

    #[test]
    fn highway_links_far_apart_roads_over_water() {
        let mut s = Sim::empty(14);
        for y in 0..H {
            for x in 14..17 {
                s.terrain[idx((x, y))] = Terrain::Water;
            }
        }
        s.add_dest((20, 5), 0).unwrap(); // driveway (21,7)
        s.add_home((10, 8), 0);
        for x in 11..=13 {
            s.place_road((x, 8));
        }
        for x in 17..=21 {
            s.place_road((x, 8));
        }
        assert!(s.find_path((13, 8), &|p| p == (17, 8)).is_none());
        s.inventory.highways = 1;
        assert_eq!(s.place_highway((13, 8), (17, 8)), EditResult::Ok);
        assert!(s.find_path((13, 8), &|p| p == (17, 8)).is_some());
        s.dests[0].pins = 1;
        run(&mut s, 12.0);
        assert!(s.score >= 1, "score was {}", s.score);
    }

    #[test]
    fn maps_have_their_own_character() {
        let count = |k: MapKind, t: Terrain| map::generate_kind(k, 3).iter().filter(|x| **x == t).count();
        assert!(count(MapKind::Highland, Terrain::Hill) > count(MapKind::Bayside, Terrain::Hill));
        assert!(count(MapKind::TwinIslands, Terrain::Water) > count(MapKind::Highland, Terrain::Water));
        for k in MapKind::ALL {
            let s = Sim::new_on(k, 5);
            assert_eq!(s.dests.len(), 1, "{:?} opens with a shop", k);
        }
    }

    #[test]
    fn spawning_continues_over_time() {
        let mut s = Sim::new(11);
        for _ in 0..(150.0 / DT) as usize {
            for d in &mut s.dests {
                d.pins = 0;
            }
            if s.offer.is_some() {
                s.choose(0);
            }
            s.step();
        }
        assert!(s.homes.len() > 2);
        assert!(s.dests.len() > 1);
        assert!(s.week >= 3);
    }
}
