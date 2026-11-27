use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::map::{self, dir_index, idx, in_bounds, Pos, Terrain, DIRS, H, W};
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

pub const DEST_W: i32 = 3;
pub const DEST_H: i32 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    Empty,
    Road,
    Home(usize),
    Dest(usize),
    Door(usize),
}

#[derive(Clone, Debug)]
pub struct Home {
    pub pos: Pos,
    pub color: usize,
    /// Sim time when it appeared, for the arrival animation.
    pub born: f32,
}

#[derive(Clone, Debug)]
pub struct Dest {
    /// Top-left tile of the footprint.
    pub origin: Pos,
    pub door: Pos,
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
    Blocked,
}

pub struct Sim {
    pub terrain: Vec<Terrain>,
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
        let terrain = map::generate(seed);
        let mut s = Sim {
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
        if self.terrain[idx(p)] != Terrain::Grass {
            return EditResult::Blocked;
        }
        if self.roads_left == 0 {
            return EditResult::NoRoadsLeft;
        }
        self.cells[idx(p)] = Cell::Road;
        self.roads_left -= 1;
        self.roads_dirty = true;
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
        self.cells[idx(p)] = Cell::Empty;
        self.roads_left += 1;
        self.roads_dirty = true;
        EditResult::Ok
    }

    fn tile_occupancy(&self, p: Pos) -> u32 {
        let b = idx(p) * 4;
        self.occ[b..b + 4].iter().filter(|o| **o).count() as u32
    }

    // ----- Spawning ----------------------------------------------------

    fn footprint_free(&self, origin: Pos, door: Pos) -> bool {
        // The footprint, its door and a one-tile margin must be clear of buildings,
        // and the footprint and door must sit on empty grass.
        for y in origin.1 - 1..=origin.1 + DEST_H {
            for x in origin.0 - 1..=origin.0 + DEST_W {
                let p = (x, y);
                if !in_bounds(p) {
                    return false;
                }
                let inside = x >= origin.0 && x < origin.0 + DEST_W && y >= origin.1 && y < origin.1 + DEST_H;
                let c = self.cells[idx(p)];
                if inside || p == door {
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

    fn place_dest(&mut self, color: usize, origin: Pos) -> Option<usize> {
        let door = (origin.0 + 1, origin.1 + DEST_H);
        if !self.footprint_free(origin, door) {
            return None;
        }
        let id = self.dests.len();
        for y in origin.1..origin.1 + DEST_H {
            for x in origin.0..origin.0 + DEST_W {
                self.cells[idx((x, y))] = Cell::Dest(id);
            }
        }
        self.cells[idx(door)] = Cell::Door(id);
        self.dests.push(Dest {
            born: self.time,
            origin,
            door,
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
                if let Some(id) = self.place_dest(color, o) {
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
        for _ in 0..200 {
            let o = (
                1 + self.rng.below((W - DEST_W - 2) as usize) as i32,
                1 + self.rng.below((H - DEST_H - 3) as usize) as i32,
            );
            // Keep destinations apart so the map stays readable.
            let far = self.dests.iter().all(|d| {
                (d.origin.0 - o.0).abs() + (d.origin.1 - o.1).abs() >= 7
            });
            if far {
                if let Some(id) = self.place_dest(color, o) {
                    return Some(id);
                }
            }
        }
        None
    }

    /// Place a home of the destination's colour a few tiles away from it.
    pub fn spawn_home(&mut self, dest: usize) -> Option<usize> {
        let d = &self.dests[dest];
        let (door, color) = (d.door, d.color);
        let mut options = Vec::new();
        for y in door.1 - 9..=door.1 + 9 {
            for x in door.0 - 12..=door.0 + 12 {
                let p = (x, y);
                let dist = (x - door.0).abs() + (y - door.1).abs();
                if !(4..=12).contains(&dist) || !in_bounds(p) {
                    continue;
                }
                if self.cells[idx(p)] != Cell::Empty || self.terrain[idx(p)] != Terrain::Grass {
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
        let id = self.homes.len();
        self.cells[idx(p)] = Cell::Home(id);
        self.homes.push(Home { pos: p, color, born: self.time });
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
        Some(id)
    }

    /// Place a home directly, for tests and scripted setups.
    pub fn add_home(&mut self, p: Pos, color: usize) -> usize {
        let id = self.homes.len();
        self.cells[idx(p)] = Cell::Home(id);
        self.homes.push(Home { pos: p, color, born: self.time });
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
        self.place_dest(color, origin)
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
            if p != start && self.cells[i] != Cell::Road {
                continue;
            }
            for dd in DIRS {
                let q = (p.0 + dd.0, p.1 + dd.1);
                if !in_bounds(q) {
                    continue;
                }
                let qi = idx(q);
                let enterable = self.cells[qi] == Cell::Road || is_goal(q);
                if !enterable {
                    continue;
                }
                let cost = d + 10 + 6 * self.tile_occupancy(q);
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
                let door = self.dests[d].door;
                self.find_path(start, &|p| p == door)
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
        if self.game_over {
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
            self.spawn_home(best);
        }
        self.dest_timer -= dt * pace;
        if self.dest_timer <= 0.0 {
            self.dest_timer = self.rng.range_f32(40.0, 55.0);
            if let Some(d) = self.spawn_random_dest() {
                self.spawn_home(d);
                self.toast = Some(("A new destination has opened".into(), 3.0));
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
                d.pin_timer = interval * jitter;
                d.pins += 1;
            }
            if d.pins > PIN_LIMIT {
                d.overflow += dt / OVERFLOW_TIME;
            } else {
                d.overflow = (d.overflow - dt / (OVERFLOW_TIME * 0.5)).max(0.0);
            }
            if d.overflow >= 1.0 {
                self.game_over = true;
                self.failed_dest = Some(i);
            }
        }
    }

    fn dispatch(&mut self) {
        for d in 0..self.dests.len() {
            while self.dests[d].pins > self.dests[d].assigned {
                let color = self.dests[d].color;
                let door = self.dests[d].door;
                let found = {
                    let cells = &self.cells;
                    let homes = &self.homes;
                    let cars = &self.cars;
                    self.find_path(door, &|p| match cells[idx(p)] {
                        Cell::Home(h) => homes[h].color == color && cars[h].state == CarState::Idle,
                        _ => false,
                    })
                };
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

    fn update_car(&mut self, c: usize, dt: f32) {
        match self.cars[c].state {
            CarState::Idle => {}
            CarState::Unloading(d) => {
                self.cars[c].retry -= dt;
                if self.cars[c].retry <= 0.0 {
                    let dest = &mut self.dests[d];
                    dest.pins -= 1;
                    dest.assigned -= 1;
                    self.score += 1;
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
                car.progress = (car.progress + CAR_SPEED * dt).min(1.0);
                if car.progress < 1.0 {
                    return;
                }
                if car.i + 1 >= car.path.len() {
                    self.arrive(c);
                    return;
                }
                let next = car.path[car.i + 1];
                let next_slot = if self.cells[idx(next)] == Cell::Road {
                    Some(idx(next) * 4 + dir_index(car.cur, next))
                } else {
                    None
                };
                if let Some(s) = next_slot {
                    if self.occ[s] {
                        return; // queue behind the car ahead
                    }
                    self.occ[s] = true;
                }
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
        assert_eq!(s.place_road((3, 3)), EditResult::Blocked);
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
        assert_eq!(s.dests[d].door, (11, 7));
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
    fn spawning_continues_over_time() {
        let mut s = Sim::new(11);
        for _ in 0..(150.0 / DT) as usize {
            for d in &mut s.dests {
                d.pins = 0;
            }
            s.step();
        }
        assert!(s.homes.len() > 2);
        assert!(s.dests.len() > 1);
        assert!(s.week >= 3);
    }
}
