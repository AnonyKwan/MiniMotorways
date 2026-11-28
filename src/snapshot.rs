//! JSON views of the game for the browser: catalog, static layout and per-frame state.
use crate::cities::{CITIES, KINDS};
use crate::consts::*;
use crate::game::*;
use crate::json::{array, num, quote};

pub fn catalog() -> String {
    let items: Vec<String> = CITIES
        .iter()
        .map(|c| {
            let start: Vec<String> = c.start.iter().map(|&(k, a)| format!("[{},{}]", k, a)).collect();
            format!(
                "{{\"id\":{},\"name\":{},\"w\":{},\"h\":{},\"groups\":{},\"planned\":{},\"start\":{}}}",
                quote(c.id),
                quote(c.name),
                c.w,
                c.h,
                c.groups,
                c.plan.len(),
                array(&start)
            )
        })
        .collect();
    array(&items)
}

impl Game {
    /// One byte per cell: bits 0-1 terrain, bit 2 tree.
    pub fn terrain_bytes(&self) -> Vec<u8> {
        self.terrain
            .kind
            .iter()
            .zip(self.terrain.tree.iter())
            .zip(self.terrain.rail.iter())
            .map(|((&k, &t), &r)| k | (if t { 4 } else { 0 }) | (if r { 8 } else { 0 }))
            .collect()
    }

    /// Buildings and roads; only changes when `static_ver` changes.
    pub fn statics(&self) -> String {
        let houses: Vec<String> = self.houses.iter().map(|h| format!("[{},{}]", h.cell, h.group)).collect();
        let dests: Vec<String> = self.dests.iter().map(|d| format!("[{},{},{},{}]", d.cell, d.group, d.level, d.kind)).collect();
        let mut roads = Vec::new();
        for c in 0..self.conn.len() as u32 {
            for d in 0..8 {
                if self.conn[c as usize] & (1 << d) == 0 {
                    continue;
                }
                if let Some(m) = self.step_cell(c, d) {
                    if m > c {
                        roads.push(format!("[{},{}]", c, m));
                    }
                }
            }
        }
        let mw: Vec<String> = self.motorways.iter().map(|&(a, b)| format!("[{},{}]", a, b)).collect();
        let lights: Vec<String> = (0..self.light.len()).filter(|&i| self.light[i]).map(|i| i.to_string()).collect();
        let rounds: Vec<String> = (0..self.round.len()).filter(|&i| self.round[i]).map(|i| i.to_string()).collect();
        format!(
            "{{\"ver\":{},\"mode\":{},\"w\":{},\"h\":{},\"houses\":{},\"dests\":{},\"roads\":{},\"mw\":{},\"lights\":{},\"rounds\":{},\"railY\":{},\"boat\":{}}}",
            self.static_ver,
            self.mode,
            self.w,
            self.h,
            array(&houses),
            array(&dests),
            array(&roads),
            array(&mw),
            array(&lights),
            array(&rounds),
            self.terrain.rail_y,
            match self.terrain.boat {
                Some((y, a, b)) => format!("[{},{},{}]", y, a, b),
                None => "[]".to_string(),
            }
        )
    }

    /// Everything that changes every frame.
    pub fn snapshot(&self) -> String {
        let up: Vec<String> = (0..KINDS as u8).map(|k| format!("[{},{}]", self.owned[k as usize], self.used(k))).collect();
        let offers: Vec<String> = self.offers.iter().map(|p| format!("[{},{},{}]", p.kind, p.amount, p.add_concrete)).collect();
        let mut cars = Vec::new();
        for ci in 0..self.cars.len() {
            if let Some((x, y, dx, dy)) = self.car_pose(ci) {
                let parked = if self.cars[ci].state == CarState::Parking { 1 } else { 0 };
                cars.push(format!("[{},{},{},{},{},{}]", num(x), num(y), num(dx), num(dy), self.cars[ci].group, parked));
            }
        }
        let dests: Vec<String> = self
            .dests
            .iter()
            .map(|d| format!("[{},{},{},{}]", d.pins, d.level, num(d.ot), Self::pin_threshold(d.level)))
            .collect();
        let region = self.play_region();
        let transit: Vec<String> = self.transits.iter().map(|t| format!("[{},{},{},{}]", t.kind, num(t.x), num(t.y), num(t.dir))).collect();
        let mut dry = Vec::new();
        if self.mode == MODE_EXPERT {
            for c in 0..self.conn.len() as u32 {
                if self.conn[c as usize] != 0 {
                    let f = self.permanence_at(c);
                    if f < 1.0 {
                        dry.push(format!("[{},{}]", c, num(f)));
                    }
                }
            }
        }
        format!(
            "{{\"t\":{},\"tick\":{},\"day\":{},\"week\":{},\"wp\":{},\"score\":{},\"over\":{},\"overDest\":{},\"offerOpen\":{},\"ver\":{},\"up\":{},\"offers\":{},\"cars\":{},\"dests\":{},\"dry\":{},\"eff\":[{},{},{}],\"region\":[{},{},{},{}],\"transit\":{},\"stats\":[{},{},{},{},{},{},{},{},{},{}]}}",
            num(self.time),
            self.ticks,
            num(self.day()),
            self.week,
            num((self.week_timer / SEC_PER_WEEK).min(1.0)),
            self.score,
            self.over,
            self.over_dest,
            self.offer_open,
            self.static_ver,
            array(&up),
            array(&offers),
            array(&cars),
            array(&dests),
            array(&dry),
            num(self.eff),
            num(self.milestone_target()),
            self.stats.milestones,
            region.0,
            region.1,
            region.2,
            region.3,
            array(&transit),
            self.stats.milestones,
            self.stats.trees,
            self.stats.deleted,
            self.stats.big_pins,
            self.stats.placed[1],
            self.stats.placed[2],
            self.stats.placed[3],
            self.stats.placed[4],
            self.stats.placed[5],
            self.trips_total
        )
    }
}
