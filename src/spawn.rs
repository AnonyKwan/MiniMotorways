//! New buildings: planned destinations from the city schedule, houses to keep supply up.
use crate::consts::*;
use crate::game::*;
use crate::terrain::LAND;

/// Marker used by the generated city data for "any colour".
pub const ANY_GROUP: u8 = 255;

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
        let (rx0, ry0, rx1, ry1) = self.play_region();
        if x0 < rx0.max(1) || y0 < ry0.max(1) || x0 + w > rx1.min(self.w - 1) || y0 + h > ry1.min(self.h - 1) {
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

    /// The ferry terminal's spot (plus a margin) is kept free for it.
    fn overlaps_terminal(&self, x0: i32, y0: i32, w: i32, h: i32) -> bool {
        match self.terrain.terminal {
            Some((tx, ty)) if !self.dests.iter().any(|d| d.kind == 2) => x0 < tx + 3 && x0 + w > tx - 1 && y0 < ty + 3 && y0 + h > ty - 1,
            _ => false,
        }
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

    fn place_special(&mut self, group: u8, kind: u8) -> bool {
        let spot = if kind == 1 {
            let ry = self.terrain.rail_y;
            let mut found = None;
            for _ in 0..80 {
                let (rx0, _, rx1, _) = self.play_region();
                let x = rx0.max(2) + self.rng.below((rx1 - rx0 - 5).max(1) as u32) as i32;
                let y = if self.rng.f32() < 0.5 { ry - 2 } else { ry + 1 };
                if self.area_free(x, y, 2, 2) {
                    found = Some((x, y));
                    break;
                }
            }
            found
        } else {
            self.terrain.terminal.filter(|&(x, y)| self.area_free(x, y, 2, 2))
        };
        let (x, y) = match spot {
            Some(s) => s,
            None => return false,
        };
        self.insert_dest(group, x, y, kind);
        true
    }

    fn place_dest(&mut self, group: u8) -> bool {
        let first = self.dests.is_empty();
        let mut best: Option<(f32, i32, i32)> = None;
        let (cx, cy) = (self.w as f32 / 2.0, self.h as f32 / 2.0);
        for _ in 0..120 {
            let (rx0, ry0, rx1, ry1) = self.play_region();
            let x = rx0.max(1) + self.rng.below((rx1 - rx0 - 3).max(1) as u32) as i32;
            let y = ry0.max(1) + self.rng.below((ry1 - ry0 - 3).max(1) as u32) as i32;
            if !self.area_free(x, y, 2, 2) || self.overlaps_terminal(x, y, 2, 2) {
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
        self.insert_dest(group, x, y, 0);
        true
    }

    fn insert_dest(&mut self, group: u8, x: i32, y: i32, kind: u8) {
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
            level: if self.mods.all_upgraded { 1 } else { 0 },
            pins: 0,
            assigned: 0,
            pin_timer: if kind == 0 { FIRST_PIN_DELAY } else { f32::INFINITY },
            born: self.time,
            ot: 0.0,
            ospeed: 0.0,
            kind,
        });
        self.static_ver += 1;
    }

    fn place_house(&mut self, group: u8, near: usize) -> bool {
        let (dx, dy) = self.dest_centre(near);
        let mates: Vec<u32> = self.houses.iter().filter(|h| h.group == group).map(|h| h.cell).collect();
        for _ in 0..120 {
            // most new homes join a neighbourhood of their own colour (within two tiles)
            if !mates.is_empty() && self.rng.f32() < 0.65 {
                let m = mates[self.rng.below(mates.len() as u32) as usize];
                let (mx, my) = self.xy(m);
                let x = mx + self.rng.below(5) as i32 - 2;
                let y = my + self.rng.below(5) as i32 - 2;
                if self.area_free(x, y, 1, 1) && !self.overlaps_terminal(x, y, 1, 1) {
                    return self.add_house(group, x, y);
                }
                continue;
            }
            let angle = self.rng.f32() * std::f32::consts::TAU;
            let far = self.rng.f32() < 0.12;
            let r = if far { 12.0 + self.rng.f32() * 8.0 } else { 4.0 + self.rng.f32().powi(2) * 8.0 };
            let x = (dx + angle.cos() * r).floor() as i32;
            let y = (dy + angle.sin() * r).floor() as i32;
            if !self.area_free(x, y, 1, 1) || self.overlaps_terminal(x, y, 1, 1) {
                continue;
            }
            return self.add_house(group, x, y);
        }
        false
    }

    fn add_house(&mut self, group: u8, x: i32, y: i32) -> bool {
        {
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
            true
        }
    }

    fn upgrade_oldest(&mut self, group: u8) {
        if self.mods.never_upgrade {
            return;
        }
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
            let mut g = city.plan[self.next_plan].1;
            if g == ANY_GROUP {
                // "any colour": pick one that already has a destination
                let live: Vec<u8> = (0..city.groups).filter(|&c| self.groups_with_dest(c as usize) > 0).collect();
                g = if live.is_empty() { 0 } else { live[self.rng.below(live.len() as u32) as usize] };
            }
            let g = g.min(city.groups.saturating_sub(1));
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
            let n = self.dests.len();
            let special = if self.city.trains > 0 && n == 2 && !self.dests.iter().any(|d| d.kind == 1) {
                1
            } else if self.city.boats > 0 && n == 3 && !self.dests.iter().any(|d| d.kind == 2) {
                2
            } else {
                0
            };
            let placed = if special > 0 { self.place_special(p.group, special) } else { self.place_dest(p.group) };
            if placed {
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
