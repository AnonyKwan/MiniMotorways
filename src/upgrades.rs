//! Weekly upgrade offers: weighted choice of two packages from the city's pool.
use crate::cities::KINDS;
use crate::consts::*;
use crate::game::*;
use crate::roads::{E_GAME_OVER, E_NO_OFFER, E_NOTHING, OK};

impl Game {
    pub fn tick_week(&mut self, dt: f32) {
        if self.offer_open || self.mode == MODE_CREATIVE || self.mode == MODE_ENDLESS {
            return;
        }
        self.week_timer += dt;
        if self.week_timer >= SEC_PER_WEEK {
            self.week_timer = 0.0;
            self.make_offer();
        }
    }

    fn rule_weight(&self, ri: usize) -> f32 {
        let r = &self.city.rules[ri];
        let week = self.week;
        if week < r.start_week as u32 || (r.last_week != 0 && week > r.last_week as u32) {
            return 0.0;
        }
        if r.max_count != 0 && self.taken[ri] >= r.max_count as u32 {
            return 0.0;
        }
        if self.mods.prevent[r.kind as usize] {
            return 0.0;
        }
        let mut w = r.base + r.inc * self.since[r.kind as usize] as f32;
        if !r.rel.is_empty() {
            w *= curve(r.rel, self.available(0) as f32);
        }
        w.max(0.0)
    }

    /// Builds this week's choices (normally two) and opens the offer.
    pub fn make_offer(&mut self) {
        let mut pool: Vec<(usize, f32)> = (0..self.city.rules.len())
            .map(|ri| (ri, self.rule_weight(ri)))
            .filter(|&(_, w)| w > 0.0)
            .collect();
        let mut picks: Vec<usize> = Vec::new();
        let choices = self.mods.choices.unwrap_or(MAX_UPGRADE_CHOICES);
        while picks.len() < choices && !pool.is_empty() {
            let total: f32 = pool.iter().map(|p| p.1).sum();
            let mut roll = self.rng.f32() * total;
            let mut chosen = pool.len() - 1;
            for (k, p) in pool.iter().enumerate() {
                if roll < p.1 {
                    chosen = k;
                    break;
                }
                roll -= p.1;
            }
            picks.push(pool.remove(chosen).0);
        }
        // shuffle the two so the better weighted one is not always first
        if picks.len() == 2 && self.rng.f32() < 0.5 {
            picks.swap(0, 1);
        }
        let mut seen = [false; KINDS];
        let city = self.city;
        let mods = self.mods.clone();
        self.offers = picks
            .iter()
            .map(|&ri| {
                let r = &city.rules[ri];
                seen[r.kind as usize] = true;
                let mult = mods.award_mult[r.kind as usize];
                let add = mods.free_concrete.unwrap_or(r.add_concrete as u32);
                Pack { kind: r.kind, amount: (r.amount as f32 * mult).round() as u32, add_concrete: if r.kind == 0 { 0 } else { add }, rule: ri }
            })
            .collect();
        if self.offers.is_empty() && choices > 0 {
            let most = self.city.rules.iter().map(|r| r.add_concrete as u32).max().unwrap_or(20).max(20);
            self.offers.push(Pack { kind: 0, amount: most, add_concrete: 0, rule: usize::MAX });
            seen[0] = true;
        }
        for k in 0..KINDS {
            if seen[k] {
                self.since[k] = 0;
            } else {
                self.since[k] += 1;
            }
        }
        self.week += 1;
        if self.offers.is_empty() {
            return;
        }
        self.offer_open = true;
    }

    /// Takes one of the offered packages.
    pub fn choose_upgrade(&mut self, i: usize) -> i32 {
        if self.over {
            return E_GAME_OVER;
        }
        if !self.offer_open || i >= self.offers.len() {
            return E_NO_OFFER;
        }
        let pack = self.offers[i].clone();
        self.owned[pack.kind as usize] += pack.amount;
        self.owned[0] += pack.add_concrete;
        if pack.rule != usize::MAX {
            self.taken[pack.rule] += 1;
        }
        self.offers.clear();
        self.offer_open = false;
        self.static_ver += 1;
        OK
    }

    /// Skips the offer without taking anything (used when leaving a game).
    pub fn dismiss_offer(&mut self) -> i32 {
        if !self.offer_open {
            return E_NOTHING;
        }
        self.offers.clear();
        self.offer_open = false;
        OK
    }
}
