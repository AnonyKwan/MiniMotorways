//! The game world: grid, buildings, cars, clock and the main simulation tick.
use crate::cities::{City, CITIES, KINDS};
use crate::consts::*;
use crate::rng::Rng;
use crate::terrain::Terrain;

pub const NONE: u32 = u32::MAX;
pub const NO_DEST: usize = usize::MAX;
/// N, NE, E, SE, S, SW, W, NW as (dx, dy). Odd indices are diagonals.
pub const DIRS: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

pub const MODE_CLASSIC: u8 = 0;
pub const MODE_EXPERT: u8 = 1;
pub const MODE_CREATIVE: u8 = 2;
pub const MODE_ENDLESS: u8 = 3;

pub const B_NONE: u8 = 0;
pub const B_HOUSE: u8 = 1;
pub const B_DEST: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CarState {
    Waiting,
    ToDest,
    Parking,
    ToHome,
}

#[derive(Clone, Debug)]
pub struct Car {
    pub house: usize,
    pub group: u8,
    pub state: CarState,
    pub path: Vec<u32>,
    pub idx: usize,
    pub t: f32,
    /// Speed in design units (see `SPEED_UNIT`).
    pub v: f32,
    pub wait: f32,
    pub dest: usize,
    pub timer: f32,
    pub hold: u32,
    pub ver: u32,
}

#[derive(Clone, Debug)]
pub struct House {
    pub cell: u32,
    pub group: u8,
    pub cars: Vec<usize>,
}

/// A train or a ferry that brings pins to a station or terminal.
#[derive(Clone, Debug)]
pub struct Transit {
    /// 0 train, 1 ferry
    pub kind: u8,
    pub x: f32,
    pub y: f32,
    pub x0: f32,
    pub x1: f32,
    pub dir: f32,
    pub speed: f32,
    pub wait: f32,
    pub armed: bool,
}

#[derive(Clone, Debug)]
pub struct Dest {
    /// Top-left cell of the 2x2 footprint.
    pub cell: u32,
    pub group: u8,
    pub level: u8,
    pub pins: u32,
    pub assigned: u32,
    pub pin_timer: f32,
    pub born: f32,
    pub ot: f32,
    pub ospeed: f32,
    /// 0 ordinary, 1 railway station, 2 ferry terminal (their pins come from arrivals)
    pub kind: u8,
}

#[derive(Clone, Debug)]
pub struct Pack {
    pub kind: u8,
    pub amount: u32,
    pub add_concrete: u32,
    pub rule: usize,
}

#[derive(Clone, Debug)]
pub struct GroupState {
    pub phase: f32,
    pub last_house: f32,
    pub house_retry: f32,
    pub extra_demand: f32,
}

#[derive(Clone, Debug)]
pub struct PendingDest {
    pub group: u8,
    pub retry_at: f32,
    pub fails: u32,
}

/// Challenge modifiers (doc 09.3). The default changes nothing.
#[derive(Clone, Debug)]
pub struct Mods {
    pub start: Vec<(u8, u32)>,
    pub prevent: [bool; KINDS],
    pub award_mult: [f32; KINDS],
    pub choices: Option<usize>,
    pub free_concrete: Option<u32>,
    pub never_upgrade: bool,
    pub all_upgraded: bool,
    pub unlimited: [bool; KINDS],
    pub trees_solid: bool,
    pub straight_cost: u32,
    pub diagonal_cost: u32,
    pub bridge_cost: u32,
    pub tunnel_cost: u32,
    pub demand: [f32; 6],
    pub bridge_speed: f32,
    pub tunnel_speed: f32,
}

impl Default for Mods {
    fn default() -> Self {
        Mods {
            start: Vec::new(),
            prevent: [false; KINDS],
            award_mult: [1.0; KINDS],
            choices: None,
            free_concrete: None,
            never_upgrade: false,
            all_upgraded: false,
            unlimited: [false; KINDS],
            trees_solid: false,
            straight_cost: 1,
            diagonal_cost: 1,
            bridge_cost: 1,
            tunnel_cost: 1,
            demand: [1.0; 6],
            bridge_speed: 1.0,
            tunnel_speed: 1.0,
        }
    }
}

/// Counters used by achievements.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub trees: u32,
    pub placed: [u32; KINDS],
    pub deleted: u32,
    pub big_pins: u32,
    pub milestones: u32,
}

pub struct Game {
    pub city_idx: usize,
    pub city: &'static City,
    pub seed: u32,
    pub mode: u8,
    pub mods: Mods,
    pub stats: Stats,
    pub eff: f32,
    /// Road connections removed by the last erase, so it can be undone.
    pub last_erase: Vec<(u32, u32)>,
    /// Removed road connections that cars are still driving on; they vanish once the cars are off.
    pub ghosts: Vec<(u32, u32)>,
    /// Kind of the upgrade taken last week (Expert never offers the same one twice running).
    pub last_pick_kind: Option<u8>,
    pub w: i32,
    pub h: i32,
    pub rng: Rng,
    pub terrain: Terrain,
    /// Bit d set: a road leaves this cell in direction d.
    pub conn: Vec<u8>,
    /// Seconds since each road connection was built (Expert mode makes roads permanent).
    pub age: Vec<[f32; 8]>,
    pub bkind: Vec<u8>,
    pub bidx: Vec<u32>,
    pub houses: Vec<House>,
    pub dests: Vec<Dest>,
    pub transits: Vec<Transit>,
    pub transit_arrivals: u32,
    pub cars: Vec<Car>,
    pub groups: Vec<GroupState>,
    pub motorways: Vec<(u32, u32)>,
    pub light: Vec<bool>,
    pub round: Vec<bool>,
    pub res: Vec<[u32; 4]>,
    // upgrades
    pub owned: [u32; KINDS],
    pub concrete_used: u32,
    pub bridge_used: u32,
    pub tunnel_used: u32,
    pub offers: Vec<Pack>,
    pub offer_open: bool,
    pub week: u32,
    pub week_timer: f32,
    pub since: [u32; KINDS],
    pub taken: Vec<u32>,
    // clock and scoring
    pub time: f32,
    pub acc: f32,
    pub score: u32,
    pub over: bool,
    pub over_dest: i32,
    // bookkeeping
    pub next_plan: usize,
    pub pending: Vec<PendingDest>,
    pub last_dest_spawn: f32,
    pub dispatch_timer: f32,
    pub topo_ver: u32,
    pub static_ver: u32,
    pub trips_total: u32,
}

impl Game {
    pub fn new(city_idx: usize, seed: u32) -> Game {
        Self::with_mode(city_idx, seed, MODE_CLASSIC)
    }

    pub fn with_mode(city_idx: usize, seed: u32, mode: u8) -> Game {
        Self::with_mods(city_idx, seed, mode, Mods::default())
    }

    pub fn with_mods(city_idx: usize, seed: u32, mode: u8, mods: Mods) -> Game {
        let city_idx = city_idx.min(CITIES.len() - 1);
        let city: &'static City = &CITIES[city_idx];
        let mut rng = Rng::new(((city_idx as u64) << 32) ^ seed as u64);
        let terrain = Terrain::generate(city, seed, &mut rng);
        let n = (city.w * city.h) as usize;
        let mut owned = [0u32; KINDS];
        for &(kind, amount) in city.start {
            owned[kind as usize] += amount as u32;
        }
        let groups = (0..city.groups as usize)
            .map(|g| GroupState {
                phase: rng.range_f(0.0, city.osc[g].period),
                last_house: -SAME_GROUP_HOUSE_DELAY,
                house_retry: 0.0,
                extra_demand: 0.0,
            })
            .collect();
        for &(kind, n) in &mods.start {
            owned[kind as usize] = n;
        }
        for k in 0..KINDS {
            if mods.unlimited[k] {
                owned[k] = 9999;
            }
        }
        if mode == MODE_CREATIVE {
            owned = [9999; KINDS];
        }
        let mut transits = Vec::new();
        if terrain.rail_y >= 0 {
            transits.push(Transit { kind: 0, x: -6.0, y: terrain.rail_y as f32 + 0.5, x0: -6.0, x1: city.w as f32 + 6.0, dir: 1.0, speed: 3.0 * SPEED_UNIT, wait: 4.0, armed: true });
        }
        if let Some((yb, wx0, wx1)) = terrain.boat {
            transits.push(Transit { kind: 1, x: wx1 as f32 + 0.5, y: yb as f32 + 0.5, x0: wx0 as f32 + 0.5, x1: wx1 as f32 + 0.5, dir: -1.0, speed: 2.6, wait: 2.0, armed: true });
        }
        Game {
            city_idx,
            city,
            seed,
            mode,
            mods,
            stats: Stats::default(),
            eff: 0.0,
            last_erase: Vec::new(),
            ghosts: Vec::new(),
            last_pick_kind: None,
            w: city.w,
            h: city.h,
            rng,
            terrain,
            conn: vec![0; n],
            age: vec![[0.0; 8]; n],
            bkind: vec![B_NONE; n],
            bidx: vec![NONE; n],
            houses: Vec::new(),
            dests: Vec::new(),
            transits,
            transit_arrivals: 0,
            cars: Vec::new(),
            groups,
            motorways: Vec::new(),
            light: vec![false; n],
            round: vec![false; n],
            res: vec![[NONE; 4]; n],
            owned,
            concrete_used: 0,
            bridge_used: 0,
            tunnel_used: 0,
            offers: Vec::new(),
            offer_open: false,
            week: 0,
            week_timer: 0.0,
            since: [0; KINDS],
            taken: vec![0; city.rules.len()],
            time: 0.0,
            acc: 0.0,
            score: 0,
            over: false,
            over_dest: -1,
            next_plan: 0,
            pending: Vec::new(),
            last_dest_spawn: -100.0,
            dispatch_timer: 0.0,
            topo_ver: 1,
            static_ver: 1,
            trips_total: 0,
        }
    }

    // ---- grid helpers ----
    pub fn cell(&self, x: i32, y: i32) -> u32 {
        (y * self.w + x) as u32
    }
    pub fn xy(&self, c: u32) -> (i32, i32) {
        (c as i32 % self.w, c as i32 / self.w)
    }
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }
    pub fn centre(&self, c: u32) -> (f32, f32) {
        let (x, y) = self.xy(c);
        (x as f32 + 0.5, y as f32 + 0.5)
    }
    /// Direction index from a to b when they are neighbours.
    pub fn dir_of(&self, a: u32, b: u32) -> Option<usize> {
        let (ax, ay) = self.xy(a);
        let (bx, by) = self.xy(b);
        let d = (bx - ax, by - ay);
        DIRS.iter().position(|&x| x == d)
    }
    pub fn step_cell(&self, c: u32, d: usize) -> Option<u32> {
        let (x, y) = self.xy(c);
        let (nx, ny) = (x + DIRS[d].0, y + DIRS[d].1);
        if self.in_bounds(nx, ny) {
            Some(self.cell(nx, ny))
        } else {
            None
        }
    }
    pub fn dest_cells(&self, di: usize) -> [u32; 4] {
        let (x, y) = self.xy(self.dests[di].cell);
        [self.cell(x, y), self.cell(x + 1, y), self.cell(x, y + 1), self.cell(x + 1, y + 1)]
    }
    pub fn dest_centre(&self, di: usize) -> (f32, f32) {
        let (x, y) = self.xy(self.dests[di].cell);
        (x as f32 + 1.0, y as f32 + 1.0)
    }

    // ---- clock ----
    pub fn day(&self) -> f32 {
        self.time / SEC_PER_DAY
    }

    /// Advances the world by `dt` real seconds (already multiplied by the game speed).
    pub fn advance(&mut self, dt: f32) {
        if self.over || !(dt > 0.0) {
            return;
        }
        self.acc += dt.min(2.0);
        let mut guard = 0;
        while self.acc >= STEP && !self.over && guard < 400 {
            self.acc -= STEP;
            self.tick(STEP);
            guard += 1;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        self.tick_spawn();
        self.tick_pins(dt);
        self.tick_overcrowd(dt);
        self.tick_transit(dt);
        self.tick_dispatch(dt);
        self.tick_cars(dt);
        self.tick_week(dt);
        self.tick_ghosts();
        if self.mode == MODE_EXPERT {
            self.tick_permanence(dt);
        }
        if self.mode == MODE_ENDLESS {
            self.tick_efficiency(dt);
        }
    }

    // ---- demand ----
    pub fn osc_value(&self, group: usize) -> f32 {
        let o = &self.city.osc[group];
        let phase = self.groups[group].phase;
        let s = ((self.day() + phase) * std::f32::consts::PI / o.period).sin().abs();
        o.base + s.powf(2.0 + o.ecc) * (o.burst - o.base)
    }

    pub fn demand_mult(level: u8) -> f32 {
        if level > 0 {
            DEMAND_MULT_UPGRADED
        } else {
            DEMAND_MULT
        }
    }

    pub fn pin_interval(&self, di: usize) -> f32 {
        let d = &self.dests[di];
        let osc = self.osc_value(d.group as usize).max(0.1);
        let mut interval = SEC_PER_DAY / (AVERAGE_CARS_PER_DAY * Self::demand_mult(d.level) * osc * self.mods.demand[(d.group as usize).min(5)].max(0.05));
        if self.mode == MODE_ENDLESS {
            interval /= ENDLESS_DEMAND_MULT;
        }
        interval
    }

    pub fn pin_threshold(level: u8) -> u32 {
        if level > 0 {
            MAX_PINS_UPGRADED
        } else {
            MAX_PINS
        }
    }

    pub fn pin_hard_cap(level: u8) -> u32 {
        if level > 0 {
            MAX_PINS_UPGRADED + OVERFLOW_PINS_UPGRADED
        } else {
            MAX_PINS + OVERFLOW_PINS
        }
    }

    fn tick_pins(&mut self, dt: f32) {
        for di in 0..self.dests.len() {
            self.dests[di].pin_timer -= dt;
            if self.dests[di].pin_timer > 0.0 {
                continue;
            }
            let interval = self.pin_interval(di);
            self.dests[di].pin_timer = interval;
            self.add_pin(di);
        }
    }

    pub fn add_pin_public(&mut self, di: usize) {
        self.add_pin(di);
    }

    fn add_pin(&mut self, di: usize) {
        let cap = Self::pin_hard_cap(self.dests[di].level);
        if self.dests[di].pins < cap {
            self.dests[di].pins += 1;
            return;
        }
        // Full: the pin moves to the nearest other destination of the same colour with room.
        let g = self.dests[di].group;
        let (cx, cy) = self.dest_centre(di);
        let mut best: Option<(f32, usize)> = None;
        for (j, d) in self.dests.iter().enumerate() {
            if j == di || d.group != g || d.pins >= Self::pin_hard_cap(d.level) {
                continue;
            }
            let (x, y) = self.dest_centre(j);
            let dist = (x - cx) * (x - cx) + (y - cy) * (y - cy);
            if best.map_or(true, |b| dist < b.0) {
                best = Some((dist, j));
            }
        }
        if let Some((_, j)) = best {
            self.dests[j].pins += 1;
        }
    }

    fn tick_overcrowd(&mut self, dt: f32) {
        if self.mode == MODE_CREATIVE || self.mode == MODE_ENDLESS {
            return;
        }
        for di in 0..self.dests.len() {
            let thr = Self::pin_threshold(self.dests[di].level);
            let extra = self.groups[self.dests[di].group as usize].extra_demand;
            let d = &mut self.dests[di];
            if d.pins > thr {
                d.ospeed = (d.ospeed + OVERCROWD_ACCEL * dt).min(1.0);
                let mult = curve(OVERCROWD_SPEED_BY_EXTRA_DEMAND, extra);
                d.ot += d.ospeed * mult * dt;
                if d.ot >= MAX_OVERCROWD_TIME {
                    self.over = true;
                    self.over_dest = di as i32;
                    return;
                }
            } else {
                d.ospeed = 0.0;
                if d.ot > MAX_OVERCROWD_TIME - GRACE_TIME {
                    d.ot = MAX_OVERCROWD_TIME - GRACE_TIME;
                }
                d.ot = (d.ot - OVERCROWD_RETURN_SPEED * dt).max(0.0);
            }
        }
    }

    /// A car delivered one pin.
    pub fn collect_pin(&mut self, di: usize) {
        let d = &mut self.dests[di];
        d.pins = d.pins.saturating_sub(1);
        d.assigned = d.assigned.saturating_sub(1);
        d.ospeed *= 0.02;
        let cut = (d.ot * PCT_REDUCE_ON_ARRIVAL).clamp(0.0, MAX_REDUCE_ON_ARRIVAL);
        let was_big = d.ot >= 20.0;
        d.ot = (d.ot - cut).max(0.0);
        if was_big && d.pins <= Self::pin_threshold(d.level) {
            self.stats.big_pins += 1;
        }
        self.score += 1;
        self.trips_total += 1;
    }
}

impl Game {
    /// Efficiency needed for the current Endless milestone.
    pub fn milestone_target(&self) -> f32 {
        let m = self.city.milestones;
        let i = self.stats.milestones as usize;
        if i < m.len() {
            m[i] as f32
        } else {
            m[m.len() - 1] as f32 + MILESTONE_INCREASE_AFTER_TABLE * (i + 1 - m.len()) as f32
        }
    }

    /// A delivered pin adds efficiency by how far the car drove.
    pub fn add_efficiency(&mut self, path_len: f32) {
        if self.mode != MODE_ENDLESS {
            return;
        }
        self.eff += curve(PATH_TO_EFFICIENCY, path_len);
        let target = self.milestone_target();
        if self.eff >= target {
            self.eff = 0.0;
            self.stats.milestones += 1;
            if !self.offer_open {
                self.make_offer();
            }
        }
    }

    pub fn tick_efficiency(&mut self, dt: f32) {
        let target = self.milestone_target().max(1.0);
        let k = curve(EFFICIENCY_LOSS_CURVE, (self.eff / target).min(1.0));
        self.eff = (self.eff - target * EFFICIENCY_LOSS_PER_SECOND * k * dt).max(0.0);
    }

    /// The part of the map in play: it starts small and grows around the middle until day 69.
    /// Returns (x0, y0, x1, y1) in cells, x1 and y1 exclusive.
    pub fn play_region(&self) -> (i32, i32, i32, i32) {
        let p = (self.day() / CAMERA_DAYS).clamp(0.0, 1.0);
        let full_h = self.h as f32;
        let h = (self.city.cam_start + (self.city.cam_end - self.city.cam_start) * p).clamp(8.0, full_h);
        let w = (h * self.w as f32 / full_h).clamp(8.0, self.w as f32);
        let (cx, cy) = (self.w as f32 / 2.0, self.h as f32 / 2.0);
        let x0 = (cx - w / 2.0).floor().max(0.0) as i32;
        let y0 = (cy - h / 2.0).floor().max(0.0) as i32;
        let x1 = ((cx + w / 2.0).ceil() as i32).min(self.w);
        let y1 = ((cy + h / 2.0).ceil() as i32).min(self.h);
        (x0, y0, x1, y1)
    }
}

impl Game {
    /// After a loss the town can carry on as Endless: no game over, no timers.
    pub fn continue_endless(&mut self) {
        self.mode = MODE_ENDLESS;
        self.over = false;
        self.over_dest = -1;
        for d in self.dests.iter_mut() {
            d.ot = 0.0;
            d.ospeed = 0.0;
        }
    }

    /// Puts back the roads taken away by the last erase (as far as concrete allows).
    pub fn undo_erase(&mut self) -> i32 {
        let edges = std::mem::take(&mut self.last_erase);
        let mut any = false;
        for (a, b) in edges {
            if self.add_road(a, b) == crate::roads::OK {
                any = true;
            }
        }
        if any {
            crate::roads::OK
        } else {
            crate::roads::E_NOTHING
        }
    }
}

impl Game {
    /// Trains run the length of the map; ferries shuttle across their channel.
    pub fn tick_transit(&mut self, dt: f32) {
        for ti in 0..self.transits.len() {
            let t = &mut self.transits[ti];
            if t.wait > 0.0 {
                t.wait -= dt;
                continue;
            }
            t.x += t.dir * t.speed * dt;
            if t.x >= t.x1 {
                t.x = t.x1;
                t.dir = -1.0;
                t.wait = 3.0;
            } else if t.x <= t.x0 {
                t.x = t.x0;
                t.dir = 1.0;
                t.wait = 3.0;
            }
            let (kind, x, armed) = (t.kind, t.x, t.armed);
            let target = self.dests.iter().position(|d| d.kind == kind + 1);
            let di = match target {
                Some(di) => di,
                None => continue,
            };
            let at = if kind == 0 { self.dest_centre(di).0 } else { self.transits[ti].x0 };
            if armed && (x - at).abs() < 0.35 {
                self.transits[ti].armed = false;
                if kind == 0 {
                    self.transits[ti].wait = 2.0;
                }
                self.transit_arrive(di, kind);
            } else if !armed && (x - at).abs() > 3.0 {
                self.transits[ti].armed = true;
            }
        }
    }

    fn transit_arrive(&mut self, di: usize, kind: u8) {
        let g = self.dests[di].group;
        let n = if kind == 0 {
            let houses = self.houses.iter().filter(|h| h.group == g).count() as f32;
            ((houses * 0.25).round() as u32).clamp(1, 4)
        } else {
            1 + self.rng.below(3)
        };
        for _ in 0..n {
            self.add_pin_public(di);
        }
        self.transit_arrivals += 1;
    }

    /// Is a train close enough to a rail cell that cars must wait?
    pub fn train_near(&self, cell: u32) -> bool {
        if !self.terrain.rail[cell as usize] {
            return false;
        }
        let (x, _) = self.centre(cell);
        self.transits.iter().any(|t| t.kind == 0 && t.wait <= 0.0 && (t.x - x).abs() < 2.6)
            || self.transits.iter().any(|t| t.kind == 0 && (t.x - x).abs() < 1.2)
    }
}

impl Game {
    fn car_on_edge(&self, a: u32, b: u32) -> bool {
        self.cars.iter().any(|c| {
            matches!(c.state, CarState::ToDest | CarState::ToHome)
                && c.idx + 1 < c.path.len()
                && ((c.path[c.idx] == a && c.path[c.idx + 1] == b) || (c.path[c.idx] == b && c.path[c.idx + 1] == a))
        })
    }

    /// A deleted road keeps working until the cars already on it have driven off.
    pub fn keep_ghost(&mut self, a: u32, b: u32) {
        if self.car_on_edge(a, b) && !self.ghosts.contains(&(a, b)) {
            self.ghosts.push((a, b));
        }
    }

    pub fn tick_ghosts(&mut self) {
        if self.ghosts.is_empty() {
            return;
        }
        let ghosts = std::mem::take(&mut self.ghosts);
        self.ghosts = ghosts.into_iter().filter(|&(a, b)| self.car_on_edge(a, b)).collect();
    }
}
