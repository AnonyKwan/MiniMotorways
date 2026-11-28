//! New buildings: planned destinations from the city schedule, houses to keep supply up.
use crate::consts::*;
use crate::game::*;
use crate::terrain::LAND;

impl Game {
    fn cell_free(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let c = self.cell(x, y) as usize;
        self.terrain.kind[c] == LAND && self.bkind[c] == B_NONE && self.conn[c] == 0
    }

    /// A rectangle of free land with a one-cell margin that holds no building.
    fn area_free(&self, x0: i32, y0: i32, w: i32, h: i32) -> bool {
        if x0 < 1 || y0 < 1 || x0 + w > self.w - 1 || y0 + h > self.h - 1 {
            return false;
        }
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                if !self.cell_free(x, y) {
                    return false;
                }
            }
        }
        for y in y0 - 1..y0 + h + 1 {
            for x in x0 - 1..x0 + w + 1 {
                if self.in_bounds(x, y) && self.bkind[self.cell(x, y) as usize] != B_NONE {
                    return false;
                }
            }
        }
        true
    }

    pub fn groups_with_dest(&self, g: usize) -> usize {
        self.dests.iter().filter(|d| d.group as usize == g).count()
    }

    pub fn required_supply(&self, di: usize) -> f32 {
        let d = &self.dests[di];
        AVERAGE_CARS_PER_DAY * Self::demand_mult(d.level) + self.groups[d.group as usize].extra_demand
    }

    pub fn supply_for(&self, di: usize) -> f32 {
        let g = self.dests[di].group;
        let n = self.groups_with_dest(g as usize) as f32;
        let mult = curve(GROUP_DESTINATION_COUNT_CURVE, n);
        let (dx, dy) = self.dest_centre(di);
        let mut total = 0.0;
        for h in self.houses.iter().filter(|h| h.group == g) {
            let (hx, hy) = self.centre(h.cell);
            let dist = ((hx - dx) * (hx - dx) + (hy - dy) * (hy - dy)).sqrt();
            total += curve(HOUSE_DISTANCE_CURVE, dist) * HOUSE_CONTRIBUTION * mult;
        }
        total
    }

    fn place_dest(&mut self, group: u8) -> bool {
        let first = self.dests.is_empty();
        let mut best: Option<(f32, i32, i32)> = None;
        let (cx, cy) = (self.w as f32 / 2.0, self.h as f32 / 2.0);
        for _ in 0..120 {
            let x = 1 + self.rng.below((self.w - 3).max(1) as u32) as i32;
            let y = 1 + self.rng.below((self.h - 3).max(1) as u32) as i32;
            if !self.area_free(x, y, 2, 2) {
                continue;
            }
            let (px, py) = (x as f32 + 1.0, y as f32 + 1.0);
            let from_centre = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            if first && from_centre > self.h as f32 * 0.25 {
                continue;
            }
            let mut nearest = f32::INFINITY;
            for j in 0..self.dests.len() {
                let (ox, oy) = self.dest_centre(j);
                nearest = nearest.min(((px - ox).powi(2) + (py - oy).powi(2)).sqrt());
            }
            if nearest < 5.0 {
                continue;
            }
            let score = self.rng.f32() * 2.0 - from_centre * 0.04 + nearest.min(14.0) * 0.1;
            if best.map_or(true, |b| score > b.0) {
                best = Some((score, x, y));
            }
        }
        let (_, x, y) = match best {
            Some(b) => b,
            None => return false,
        };
        let cell = self.cell(x, y);
        let di = self.dests.len();
        for (cx, cy) in [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)] {
            let c = self.cell(cx, cy) as usize;
            self.bkind[c] = B_DEST;
            self.bidx[c] = di as u32;
            self.terrain.tree[c] = false;
        }
        self.dests.push(Dest {
            cell,
            group,
            level: 0,
            pins: 0,
            assigned: 0,
            pin_timer: FIRST_PIN_DELAY,
            born: self.time,
            ot: 0.0,
            ospeed: 0.0,
        });
        self.static_ver += 1;
        true
    }

    fn place_house(&mut self, group: u8, near: usize) -> bool {
        let (dx, dy) = self.dest_centre(near);
        for _ in 0..120 {
            let angle = self.rng.f32() * std::f32::consts::TAU;
            let far = self.rng.f32() < 0.12;
            let r = if far { 12.0 + self.rng.f32() * 8.0 } else { 4.0 + self.rng.f32().powi(2) * 8.0 };
            let x = (dx + angle.cos() * r).floor() as i32;
            let y = (dy + angle.sin() * r).floor() as i32;
            if !self.area_free(x, y, 1, 1) {
                continue;
            }
            let cell = self.cell(x, y);
            let hi = self.houses.len();
            self.bkind[cell as usize] = B_HOUSE;
            self.bidx[cell as usize] = hi as u32;
            self.terrain.tree[cell as usize] = false;
            let mut cars = Vec::new();
            for _ in 0..CARS_PER_HOUSE {
                cars.push(self.cars.len());
                self.cars.push(Car {
                    house: hi,
                    group,
                    state: CarState::Waiting,
                    path: Vec::new(),
                    idx: 0,
                    t: 0.0,
                    v: 0.0,
                    wait: 0.0,
                    dest: NO_DEST,
                    timer: 2.0,
                    hold: NONE,
                    ver: 0,
                });
            }
            self.houses.push(House { cell, group, cars });
            self.static_ver += 1;
            return true;
        }
        false
    }

    fn upgrade_oldest(&mut self, group: u8) {
        let pick = self
            .dests
            .iter()
            .enumerate()
            .filter(|(_, d)| d.group == group && d.level == 0)
            .min_by(|a, b| a.1.born.partial_cmp(&b.1.born).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i);
        if let Some(i) = pick {
            self.dests[i].level = 1;
            self.static_ver += 1;
        }
    }

    pub fn tick_spawn(&mut self) {
        let city = self.city;
        while self.next_plan < city.plan.len() && city.plan[self.next_plan].0 * SEC_PER_DAY <= self.time {
            let g = city.plan[self.next_plan].1.min(city.groups.saturating_sub(1));
            self.pending.push(PendingDest { group: g, retry_at: self.time, fails: 0 });
            self.next_plan += 1;
        }
        let mut i = 0;
        while i < self.pending.len() {
            let p = self.pending[i].clone();
            if self.time < p.retry_at {
                i += 1;
                continue;
            }
            let spaced = self.dests.is_empty() || self.time - self.last_dest_spawn >= 10.0;
            if !spaced {
                i += 1;
                continue;
            }
            if self.place_dest(p.group) {
                self.last_dest_spawn = self.time;
                self.pending.remove(i);
            } else if p.fails + 1 >= MAX_FAILED_DEST_BEFORE_UPGRADE {
                self.upgrade_oldest(p.group);
                self.pending.remove(i);
            } else {
                self.pending[i].fails += 1;
                self.pending[i].retry_at = self.time + FAILED_DEST_RETRY;
                i += 1;
            }
        }
        for g in 0..self.groups.len() {
            if self.time - self.groups[g].last_house < SAME_GROUP_HOUSE_DELAY || self.time < self.groups[g].house_retry {
                continue;
            }
            let houses = self.houses.iter().filter(|h| h.group as usize == g).count();
            if houses >= MAX_HOUSES_PER_GROUP {
                continue;
            }
            let short = (0..self.dests.len()).find(|&di| self.dests[di].group as usize == g && self.supply_for(di) < self.required_supply(di));
            let di = match short {
                Some(di) => di,
                None => {
                    self.groups[g].extra_demand = (self.groups[g].extra_demand - 0.01).max(0.0);
                    continue;
                }
            };
            if self.place_house(g as u8, di) {
                self.groups[g].last_house = self.time;
            } else {
                self.groups[g].house_retry = self.time + FAILED_HOUSE_COOLDOWN;
                self.groups[g].extra_demand = (self.groups[g].extra_demand + 0.25).min(6.0);
            }
        }
    }
}
