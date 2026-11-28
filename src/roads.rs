//! Building and removing roads, motorways, traffic lights and roundabouts.
use crate::cities::{K_BRIDGE, K_LIGHT, K_MOTORWAY, K_ROUND, K_TUNNEL};
use crate::consts::*;
use crate::game::*;
use crate::terrain::{LAND, MOUNTAIN, WATER};

pub const OK: i32 = 0;
pub const E_BOUNDS: i32 = 1;
pub const E_NOT_ADJACENT: i32 = 2;
pub const E_BLOCKED: i32 = 3;
pub const E_NO_CONCRETE: i32 = 4;
pub const E_NO_BRIDGE: i32 = 5;
pub const E_NO_TUNNEL: i32 = 6;
pub const E_CROSSING: i32 = 7;
pub const E_EXISTS: i32 = 8;
pub const E_NO_UPGRADE: i32 = 9;
pub const E_BAD_PLACEMENT: i32 = 10;
pub const E_GAME_OVER: i32 = 11;
pub const E_NOTHING: i32 = 12;
pub const E_TOO_SHORT: i32 = 13;
pub const E_NO_OFFER: i32 = 14;
pub const E_PERMANENT: i32 = 15;

impl Game {
    pub fn has_ground_edge(&self, a: u32, b: u32) -> bool {
        match self.dir_of(a, b) {
            Some(d) => self.conn[a as usize] & (1 << d) != 0,
            None => false,
        }
    }

    pub fn motorway_index(&self, a: u32, b: u32) -> Option<usize> {
        self.motorways.iter().position(|&(x, y)| (x == a && y == b) || (x == b && y == a))
    }

    pub fn has_edge(&self, a: u32, b: u32) -> bool {
        self.has_ground_edge(a, b) || self.motorway_index(a, b).is_some()
    }

    pub fn degree(&self, n: u32) -> u32 {
        let ground = self.conn[n as usize].count_ones();
        let ramps = self.motorways.iter().filter(|&&(a, b)| a == n || b == n).count() as u32;
        ground + ramps
    }

    pub fn is_junction(&self, n: u32) -> bool {
        self.degree(n) >= 3
    }

    fn crosses_diagonal(&self, a: u32, d: usize) -> bool {
        if d % 2 == 0 {
            return false;
        }
        let (ax, ay) = self.xy(a);
        let (dx, dy) = DIRS[d];
        let c = self.cell(ax + dx, ay);
        let e = self.cell(ax, ay + dy);
        self.has_ground_edge(c, e)
    }

    /// Number of separate bridge (WATER) or tunnel (MOUNTAIN) passages currently built.
    pub fn count_passages(&self, kind: u8) -> u32 {
        let n = self.conn.len();
        let on = |i: usize| self.terrain.kind[i] == kind && self.conn[i] != 0;
        let mut seen = vec![false; n];
        let mut count = 0;
        for start in 0..n {
            if seen[start] || !on(start) {
                continue;
            }
            count += 1;
            let mut stack = vec![start as u32];
            seen[start] = true;
            while let Some(c) = stack.pop() {
                for d in 0..8 {
                    if self.conn[c as usize] & (1 << d) == 0 {
                        continue;
                    }
                    if let Some(m) = self.step_cell(c, d) {
                        if !seen[m as usize] && on(m as usize) {
                            seen[m as usize] = true;
                            stack.push(m);
                        }
                    }
                }
            }
        }
        count
    }

    fn refresh_passages(&mut self) {
        self.bridge_used = self.count_passages(WATER);
        self.tunnel_used = self.count_passages(MOUNTAIN);
    }

    fn drop_orphan_junction_parts(&mut self, n: u32) {
        if !self.is_junction(n) {
            self.light[n as usize] = false;
            self.round[n as usize] = false;
        }
    }

    fn touched(&mut self) {
        self.topo_ver += 1;
        self.static_ver += 1;
    }

    /// Builds one road connection between two neighbouring cells.
    pub fn add_road(&mut self, a: u32, b: u32) -> i32 {
        if self.over {
            return E_GAME_OVER;
        }
        let (ax, ay) = self.xy(a);
        let (bx, by) = self.xy(b);
        if !self.in_bounds(ax, ay) || !self.in_bounds(bx, by) || a as usize >= self.conn.len() || b as usize >= self.conn.len() {
            return E_BOUNDS;
        }
        let d = match self.dir_of(a, b) {
            Some(d) => d,
            None => return E_NOT_ADJACENT,
        };
        if self.conn[a as usize] & (1 << d) != 0 {
            return E_EXISTS;
        }
        let (ka, kb) = (self.bkind[a as usize], self.bkind[b as usize]);
        if ka != B_NONE && kb != B_NONE {
            return E_BLOCKED;
        }
        if self.crosses_diagonal(a, d) {
            return E_CROSSING;
        }
        let cost = if ka != B_NONE || kb != B_NONE { 0 } else { 1 };
        if cost > self.owned[0].saturating_sub(self.concrete_used) {
            return E_NO_CONCRETE;
        }
        let od = (d + 4) % 8;
        let (tree_a, tree_b) = (self.terrain.tree[a as usize], self.terrain.tree[b as usize]);
        self.conn[a as usize] |= 1 << d;
        self.conn[b as usize] |= 1 << od;
        self.age[a as usize][d] = 0.0;
        self.age[b as usize][od] = 0.0;
        self.terrain.tree[a as usize] = false;
        self.terrain.tree[b as usize] = false;
        self.concrete_used += cost;
        let (bridges, tunnels) = (self.count_passages(WATER), self.count_passages(MOUNTAIN));
        let err = if bridges > self.owned[K_BRIDGE as usize] {
            E_NO_BRIDGE
        } else if tunnels > self.owned[K_TUNNEL as usize] {
            E_NO_TUNNEL
        } else {
            OK
        };
        if err != OK {
            self.conn[a as usize] &= !(1 << d);
            self.conn[b as usize] &= !(1 << od);
            self.terrain.tree[a as usize] = tree_a;
            self.terrain.tree[b as usize] = tree_b;
            self.concrete_used -= cost;
            return err;
        }
        self.bridge_used = bridges;
        self.tunnel_used = tunnels;
        self.touched();
        OK
    }

    /// Removes one road connection and gives its concrete back.
    pub fn remove_road(&mut self, a: u32, b: u32) -> i32 {
        if self.over {
            return E_GAME_OVER;
        }
        let d = match self.dir_of(a, b) {
            Some(d) => d,
            None => return E_NOT_ADJACENT,
        };
        if self.conn[a as usize] & (1 << d) == 0 {
            return E_NOTHING;
        }
        if self.mode == MODE_EXPERT && self.age[a as usize][d] >= PERMANENCE_SECONDS * PERMANENCE_LOCK_FRACTION {
            return E_PERMANENT;
        }
        let od = (d + 4) % 8;
        self.conn[a as usize] &= !(1 << d);
        self.conn[b as usize] &= !(1 << od);
        let cost = if self.bkind[a as usize] != B_NONE || self.bkind[b as usize] != B_NONE { 0 } else { 1 };
        self.concrete_used = self.concrete_used.saturating_sub(cost);
        self.drop_orphan_junction_parts(a);
        self.drop_orphan_junction_parts(b);
        self.refresh_passages();
        self.touched();
        OK
    }

    /// Removes every road touching a cell, plus a motorway that ends there.
    pub fn erase_cell(&mut self, c: u32) -> i32 {
        if self.over {
            return E_GAME_OVER;
        }
        if c as usize >= self.conn.len() {
            return E_BOUNDS;
        }
        let mut changed = false;
        for d in 0..8 {
            if self.conn[c as usize] & (1 << d) != 0 {
                if let Some(m) = self.step_cell(c, d) {
                    if self.remove_road(c, m) == OK {
                        changed = true;
                    }
                }
            }
        }
        while let Some(i) = self.motorways.iter().position(|&(a, b)| a == c || b == c) {
            self.motorways.remove(i);
            changed = true;
        }
        if self.light[c as usize] || self.round[c as usize] {
            self.light[c as usize] = false;
            self.round[c as usize] = false;
            changed = true;
        }
        if changed {
            self.touched();
            OK
        } else {
            E_NOTHING
        }
    }

    pub fn add_motorway(&mut self, a: u32, b: u32) -> i32 {
        if self.over {
            return E_GAME_OVER;
        }
        let n = self.conn.len() as u32;
        if a >= n || b >= n || a == b {
            return E_BOUNDS;
        }
        let ((ax, ay), (bx, by)) = (self.xy(a), self.xy(b));
        if (ax - bx).abs().max((ay - by).abs()) < MIN_MOTORWAY_LENGTH {
            return E_TOO_SHORT;
        }
        for c in [a, b] {
            if self.terrain.kind[c as usize] != LAND || self.bkind[c as usize] != B_NONE {
                return E_BAD_PLACEMENT;
            }
        }
        if self.motorway_index(a, b).is_some() {
            return E_EXISTS;
        }
        if self.motorways.len() as u32 >= self.owned[K_MOTORWAY as usize] {
            return E_NO_UPGRADE;
        }
        self.motorways.push((a, b));
        self.touched();
        OK
    }

    pub fn remove_motorway(&mut self, i: usize) -> i32 {
        if i >= self.motorways.len() {
            return E_NOTHING;
        }
        self.motorways.remove(i);
        self.touched();
        OK
    }

    fn toggle_junction_part(&mut self, c: u32, kind: u8) -> i32 {
        if self.over {
            return E_GAME_OVER;
        }
        if c as usize >= self.conn.len() {
            return E_BOUNDS;
        }
        let ci = c as usize;
        let (flags, other) = if kind == K_LIGHT { (&self.light, &self.round) } else { (&self.round, &self.light) };
        if flags[ci] {
            if kind == K_LIGHT {
                self.light[ci] = false;
            } else {
                self.round[ci] = false;
            }
            self.touched();
            return OK;
        }
        if other[ci] || self.bkind[ci] != B_NONE || self.terrain.kind[ci] != LAND || !self.is_junction(c) {
            return E_BAD_PLACEMENT;
        }
        let used = flags.iter().filter(|&&f| f).count() as u32;
        if used >= self.owned[kind as usize] {
            return E_NO_UPGRADE;
        }
        if kind == K_LIGHT {
            self.light[ci] = true;
        } else {
            self.round[ci] = true;
        }
        self.touched();
        OK
    }

    pub fn toggle_light(&mut self, c: u32) -> i32 {
        self.toggle_junction_part(c, K_LIGHT)
    }

    pub fn toggle_roundabout(&mut self, c: u32) -> i32 {
        self.toggle_junction_part(c, K_ROUND)
    }

    /// Expert mode: road connections dry and become permanent.
    pub fn tick_permanence(&mut self, dt: f32) {
        for i in 0..self.conn.len() {
            let bits = self.conn[i];
            if bits == 0 {
                continue;
            }
            for d in 0..8 {
                if bits & (1 << d) != 0 && self.age[i][d] < PERMANENCE_SECONDS {
                    self.age[i][d] += dt;
                }
            }
        }
    }

    /// Share (0..1) of the permanence timer used up by the youngest road at a cell.
    pub fn permanence_at(&self, c: u32) -> f32 {
        let i = c as usize;
        let mut youngest = PERMANENCE_SECONDS;
        for d in 0..8 {
            if self.conn[i] & (1 << d) != 0 {
                youngest = youngest.min(self.age[i][d]);
            }
        }
        (youngest / PERMANENCE_SECONDS).min(1.0)
    }

    // ---- accounting used by the UI and the tests ----
    pub fn used(&self, kind: u8) -> u32 {
        match kind {
            0 => self.concrete_used,
            K_BRIDGE => self.bridge_used,
            K_MOTORWAY => self.motorways.len() as u32,
            K_LIGHT => self.light.iter().filter(|&&f| f).count() as u32,
            K_ROUND => self.round.iter().filter(|&&f| f).count() as u32,
            K_TUNNEL => self.tunnel_used,
            _ => 0,
        }
    }

    pub fn available(&self, kind: u8) -> u32 {
        self.owned[kind as usize].saturating_sub(self.used(kind))
    }

    /// Cheapest way to lay a straight-ish road: returns the cells from `a` to `b` stepping one cell at a time.
    pub fn line_cells(&self, a: u32, b: u32) -> Vec<u32> {
        let ((mut x, mut y), (bx, by)) = (self.xy(a), self.xy(b));
        let mut out = vec![self.cell(x, y)];
        while (x, y) != (bx, by) {
            x += (bx - x).signum();
            y += (by - y).signum();
            out.push(self.cell(x, y));
        }
        out
    }
}
