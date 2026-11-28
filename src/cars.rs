//! Car movement: lanes, following distance, junction reservations, lights, parking.
use crate::consts::*;
use crate::game::*;
use std::collections::HashMap;

const EPS: f32 = 0.001;

impl Game {
    /// Does this node currently let a car from direction class `class` through?
    pub fn light_green(&self, n: u32, class: usize) -> bool {
        let mut present = [false; 4];
        for d in 0..8 {
            if self.conn[n as usize] & (1 << d) != 0 {
                present[d % 4] = true;
            }
        }
        let classes: Vec<usize> = (0..4).filter(|&c| present[c]).collect();
        if classes.is_empty() || !present[class] {
            return true;
        }
        let phase_len = LIGHT_GREEN + LIGHT_AMBER;
        let total = phase_len * classes.len() as f32;
        let pos = self.time % total;
        let slot = (pos / phase_len) as usize;
        let within = pos - slot as f32 * phase_len;
        classes[slot.min(classes.len() - 1)] == class && within < LIGHT_GREEN
    }

    fn try_reserve(&mut self, n: u32, car: usize, class: usize, force: bool) -> bool {
        if force {
            return true;
        }
        if self.light[n as usize] && !self.light_green(n, class) {
            return false;
        }
        let cap = if self.round[n as usize] { 4 } else { 1 };
        let slots = &mut self.res[n as usize];
        if slots[..cap].iter().any(|&s| s == car as u32) {
            return true;
        }
        for s in slots[..cap].iter_mut() {
            if *s == NONE {
                *s = car as u32;
                return true;
            }
        }
        false
    }

    fn release(&mut self, n: u32, car: usize) {
        if n == NONE {
            return;
        }
        for s in self.res[n as usize].iter_mut() {
            if *s == car as u32 {
                *s = NONE;
            }
        }
    }

    fn release_hold(&mut self, ci: usize) {
        let h = self.cars[ci].hold;
        self.release(h, ci);
        self.cars[ci].hold = NONE;
    }

    fn path_valid(&self, ci: usize) -> bool {
        let c = &self.cars[ci];
        if c.path.len() < 2 {
            return false;
        }
        (c.idx..c.path.len() - 1).all(|k| self.has_edge(c.path[k], c.path[k + 1]))
    }

    /// Sends the car home without a pin when its route has vanished.
    fn strand(&mut self, ci: usize) {
        self.release_hold(ci);
        let (state, dest) = (self.cars[ci].state, self.cars[ci].dest);
        if state == CarState::ToDest && dest != NO_DEST {
            self.dests[dest].assigned = self.dests[dest].assigned.saturating_sub(1);
        }
        let c = &mut self.cars[ci];
        c.state = CarState::Waiting;
        c.timer = 2.0;
        c.path.clear();
        c.idx = 0;
        c.t = 0.0;
        c.v = 0.0;
        c.wait = 0.0;
        c.dest = NO_DEST;
    }

    fn reroute(&mut self, ci: usize) {
        let (state, dest, house) = (self.cars[ci].state, self.cars[ci].dest, self.cars[ci].house);
        let sources: Vec<u32> = if state == CarState::ToDest && dest != NO_DEST {
            self.dest_cells(dest).to_vec()
        } else {
            vec![self.houses[house].cell]
        };
        let (idx, t) = (self.cars[ci].idx, self.cars[ci].t);
        let (a, b) = (self.cars[ci].path[idx], self.cars[ci].path[(idx + 1).min(self.cars[ci].path.len() - 1)]);
        let keep_edge = t > 0.0 && self.has_edge(a, b);
        let start = if keep_edge { b } else { a };
        match self.route_from(start, &sources) {
            Some(rest) => {
                let mut path = Vec::with_capacity(rest.len() + 1);
                if keep_edge {
                    path.push(a);
                }
                path.extend(rest);
                if path.len() < 2 {
                    self.strand(ci);
                    return;
                }
                let held = self.cars[ci].hold;
                if held != NONE && !path.contains(&held) {
                    self.release_hold(ci);
                }
                let c = &mut self.cars[ci];
                c.path = path;
                c.idx = 0;
                if !keep_edge {
                    c.t = 0.0;
                }
            }
            None => self.strand(ci),
        }
    }

    fn turn_multiplier(&self, a: u32, b: u32, c: u32) -> f32 {
        let (ax, ay) = self.xy(a);
        let (bx, by) = self.xy(b);
        let (cx, cy) = self.xy(c);
        let v1 = ((bx - ax) as f32, (by - ay) as f32);
        let v2 = ((cx - bx) as f32, (cy - by) as f32);
        let l1 = (v1.0 * v1.0 + v1.1 * v1.1).sqrt();
        let l2 = (v2.0 * v2.0 + v2.1 * v2.1).sqrt();
        if l1 < EPS || l2 < EPS {
            return 1.0;
        }
        let cos = (v1.0 * v2.0 + v1.1 * v2.1) / (l1 * l2);
        if cos > 0.98 {
            1.0
        } else if cos > 0.65 {
            0.85
        } else if cos > -0.1 {
            RIGHT_TURN
        } else {
            SHARP_TURN
        }
    }

    /// For every moving car: distance to the car ahead on the same road segment and its speed.
    fn following_gaps(&self) -> Vec<(f32, f32)> {
        let mut groups: HashMap<(u32, u32), Vec<(f32, usize)>> = HashMap::new();
        for (i, c) in self.cars.iter().enumerate() {
            if matches!(c.state, CarState::ToDest | CarState::ToHome) && c.idx + 1 < c.path.len() {
                groups.entry((c.path[c.idx], c.path[c.idx + 1])).or_default().push((c.t, i));
            }
        }
        let mut gaps = vec![(f32::INFINITY, 0.0f32); self.cars.len()];
        for ((a, b), mut list) in groups {
            let len = self.edge_length(a, b);
            list.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal).then(x.1.cmp(&y.1)));
            for k in 1..list.len() {
                let ahead = list[k - 1];
                let me = list[k];
                gaps[me.1] = ((ahead.0 - me.0) * len, self.cars[ahead.1].v);
            }
        }
        gaps
    }

    pub fn tick_cars(&mut self, dt: f32) {
        let gaps = self.following_gaps();
        for ci in 0..self.cars.len() {
            self.step_car(ci, dt, gaps[ci]);
        }
    }

    fn step_car(&mut self, ci: usize, dt: f32, gap: (f32, f32)) {
        match self.cars[ci].state {
            CarState::Waiting => {
                let c = &mut self.cars[ci];
                c.timer = (c.timer - dt).max(0.0);
                return;
            }
            CarState::Parking => {
                self.cars[ci].timer -= dt;
                if self.cars[ci].timer <= 0.0 {
                    self.begin_return(ci);
                }
                return;
            }
            _ => {}
        }
        if self.cars[ci].ver != self.topo_ver {
            if !self.path_valid(ci) {
                self.reroute(ci);
                if !matches!(self.cars[ci].state, CarState::ToDest | CarState::ToHome) {
                    return;
                }
            }
            self.cars[ci].ver = self.topo_ver;
        }
        let (idx, t) = (self.cars[ci].idx, self.cars[ci].t);
        let path_len = self.cars[ci].path.len();
        if idx + 1 >= path_len {
            self.arrive(ci);
            return;
        }
        let a = self.cars[ci].path[idx];
        let b = self.cars[ci].path[idx + 1];
        let mut len = self.edge_length(a, b).max(EPS);
        let last = idx + 2 >= path_len;

        // let go of the junction just passed
        if self.cars[ci].hold == a && t * len >= APPROACH {
            self.release_hold(ci);
        }

        // reservation of the junction ahead
        let rem = (1.0 - t) * len;
        let mut max_travel = f32::INFINITY;
        let mut blocked = false;
        if !last && self.is_junction(b) && self.cars[ci].hold != b {
            if rem <= APPROACH + EPS {
                let class = self.dir_of(a, b).map(|d| d % 4).unwrap_or(0);
                let force = self.cars[ci].wait >= MAX_WAIT_AT_INTERSECTION;
                if self.try_reserve(b, ci, class, force) {
                    self.cars[ci].hold = b;
                    self.cars[ci].wait = 0.0;
                } else {
                    max_travel = 0.0;
                    blocked = true;
                }
            } else {
                max_travel = rem - APPROACH;
            }
        }

        // speed
        let motorway = self.is_motorway_edge(a, b);
        let mut target = if motorway { MOTORWAY_SPEED } else { LANE_SPEED };
        if self.round[a as usize] || self.round[b as usize] {
            target = ROUNDABOUT_SPEED;
        }
        if self.mods.bridge_speed != 1.0 && (self.terrain.kind[a as usize] == 1 || self.terrain.kind[b as usize] == 1) {
            target *= self.mods.bridge_speed;
        } else if self.mods.tunnel_speed != 1.0 && (self.terrain.kind[a as usize] == 2 || self.terrain.kind[b as usize] == 2) {
            target *= self.mods.tunnel_speed;
        }
        if !last && rem < 1.0 {
            let c = self.cars[ci].path[idx + 2];
            target *= self.turn_multiplier(a, b, c);
            if self.is_junction(b) && !self.round[b as usize] {
                target = target.min(INTERSECTION_SPEED);
            }
        }
        let mut limit = f32::INFINITY;
        if gap.0 < MIN_GAP + 0.6 {
            target = target.min(gap.1);
            limit = (gap.0 - MIN_GAP).max(0.0);
        }
        if blocked {
            target = 0.0;
        }
        let v = self.cars[ci].v;
        let v = if v < target { (v + MAX_ACCEL * dt).min(target) } else { (v - MAX_DECEL * dt).max(target) };
        self.cars[ci].v = v;
        let travel = (v * SPEED_UNIT * dt).min(max_travel).min(limit);
        if blocked {
            self.cars[ci].wait += dt;
        } else if travel > EPS * 0.1 {
            self.cars[ci].wait = 0.0;
        }
        let mut t = t + travel / len;

        // roll over onto following segments
        let mut idx = idx;
        while t >= 1.0 - 1e-6 {
            let leftover = (t - 1.0) * len;
            idx += 1;
            if idx + 1 >= path_len {
                self.cars[ci].idx = idx;
                self.cars[ci].t = 0.0;
                self.arrive(ci);
                return;
            }
            let na = self.cars[ci].path[idx];
            let nb = self.cars[ci].path[idx + 1];
            let nlen = self.edge_length(na, nb).max(EPS);
            t = (leftover / nlen).max(0.0);
            len = nlen;
        }
        self.cars[ci].idx = idx;
        self.cars[ci].t = t.min(1.0);
    }

    fn arrive(&mut self, ci: usize) {
        self.release_hold(ci);
        match self.cars[ci].state {
            CarState::ToDest => {
                let di = self.cars[ci].dest;
                if di != NO_DEST {
                    let len = self.path_length(ci);
                    self.collect_pin(di);
                    self.add_efficiency(len);
                }
                let c = &mut self.cars[ci];
                c.state = CarState::Parking;
                c.timer = PARK_TIME;
                c.v = 0.0;
                c.t = 0.0;
                c.idx = c.path.len().saturating_sub(1);
            }
            _ => {
                let c = &mut self.cars[ci];
                c.state = CarState::Waiting;
                c.timer = TIME_AT_HOUSE;
                c.path.clear();
                c.idx = 0;
                c.t = 0.0;
                c.v = 0.0;
                c.dest = NO_DEST;
            }
        }
    }

    /// Distance driven on this trip, in cells.
    fn path_length(&self, ci: usize) -> f32 {
        let p = &self.cars[ci].path;
        p.windows(2).map(|w| self.edge_length(w[0], w[1])).sum()
    }

    fn begin_return(&mut self, ci: usize) {
        let ver = self.topo_ver;
        let c = &mut self.cars[ci];
        c.path.reverse();
        c.idx = 0;
        c.t = 0.0;
        c.v = 0.0;
        c.wait = 0.0;
        c.hold = NONE;
        c.dest = NO_DEST;
        c.state = CarState::ToHome;
        c.ver = ver;
        if c.path.len() < 2 {
            c.state = CarState::Waiting;
            c.timer = TIME_AT_HOUSE;
            c.path.clear();
        }
    }

    /// World position and heading of a moving car.
    pub fn car_pose(&self, ci: usize) -> Option<(f32, f32, f32, f32)> {
        let c = &self.cars[ci];
        if !matches!(c.state, CarState::ToDest | CarState::ToHome | CarState::Parking) {
            return None;
        }
        if c.idx + 1 >= c.path.len() {
            let n = *c.path.last()?;
            let (x, y) = self.centre(n);
            return Some((x, y, 0.0, 0.0));
        }
        let (ax, ay) = self.centre(c.path[c.idx]);
        let (bx, by) = self.centre(c.path[c.idx + 1]);
        let (dx, dy) = (bx - ax, by - ay);
        let l = (dx * dx + dy * dy).sqrt().max(EPS);
        Some((ax + dx * c.t, ay + dy * c.t, dx / l, dy / l))
    }
}
