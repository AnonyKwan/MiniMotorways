//! Path finding over the road graph and sending waiting cars to destinations with pins.
use crate::consts::*;
use crate::game::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const UNREACHED: u32 = u32::MAX;

impl Game {
    pub fn edge_length(&self, a: u32, b: u32) -> f32 {
        let ((ax, ay), (bx, by)) = (self.xy(a), self.xy(b));
        let (dx, dy) = ((ax - bx) as f32, (ay - by) as f32);
        (dx * dx + dy * dy).sqrt()
    }

    pub fn is_motorway_edge(&self, a: u32, b: u32) -> bool {
        self.dir_of(a, b).is_none() && self.motorway_index(a, b).is_some()
    }

    /// Calls `f(neighbour, travel cost in milliseconds)` for every road leaving `n`.
    fn each_neighbour(&self, n: u32, mut f: impl FnMut(u32, u32)) {
        let c = self.conn[n as usize];
        for d in 0..8 {
            if c & (1 << d) == 0 {
                continue;
            }
            if let Some(m) = self.step_cell(n, d) {
                let len = if d % 2 == 1 { 1414 } else { 1000 };
                f(m, len);
            }
        }
        for &(a, b) in &self.motorways {
            let other = if a == n {
                b
            } else if b == n {
                a
            } else {
                continue;
            };
            let ms = (self.edge_length(n, other) * 1000.0 / MOTORWAY_SPEED) as u32;
            f(other, ms);
        }
    }

    fn junction_penalty(&self, m: u32) -> u32 {
        if self.bkind[m as usize] != B_NONE || !self.is_junction(m) {
            0
        } else if self.round[m as usize] {
            100
        } else if self.light[m as usize] {
            600
        } else {
            400
        }
    }

    /// Dijkstra from the given source cells. Building cells are dead ends: a path may
    /// start or finish in one but never runs through one.
    pub fn dijkstra(&self, sources: &[u32]) -> (Vec<u32>, Vec<u32>) {
        let n = self.conn.len();
        let mut dist = vec![UNREACHED; n];
        let mut prev = vec![NONE; n];
        let mut is_source = vec![false; n];
        let mut heap = BinaryHeap::new();
        for &s in sources {
            dist[s as usize] = 0;
            is_source[s as usize] = true;
            heap.push(Reverse((0u32, s)));
        }
        while let Some(Reverse((d, u))) = heap.pop() {
            if d > dist[u as usize] {
                continue;
            }
            if self.bkind[u as usize] != B_NONE && !is_source[u as usize] {
                continue;
            }
            let mut next: Vec<(u32, u32)> = Vec::new();
            self.each_neighbour(u, |m, cost| next.push((m, cost)));
            for (m, cost) in next {
                let nd = d + cost + self.junction_penalty(m);
                if nd < dist[m as usize] {
                    dist[m as usize] = nd;
                    prev[m as usize] = u;
                    heap.push(Reverse((nd, m)));
                }
            }
        }
        (dist, prev)
    }

    /// Follows `prev` from `start` back to the source it came from.
    pub fn chain(prev: &[u32], dist: &[u32], start: u32) -> Option<Vec<u32>> {
        if dist[start as usize] == UNREACHED {
            return None;
        }
        let mut out = vec![start];
        let mut cur = start;
        while prev[cur as usize] != NONE {
            cur = prev[cur as usize];
            out.push(cur);
            if out.len() > prev.len() {
                return None;
            }
        }
        Some(out)
    }

    pub fn route_from(&self, start: u32, sources: &[u32]) -> Option<Vec<u32>> {
        let (dist, prev) = self.dijkstra(sources);
        Self::chain(&prev, &dist, start)
    }

    fn waiting_car(&self, hi: usize) -> Option<usize> {
        self.houses[hi]
            .cars
            .iter()
            .copied()
            .find(|&ci| self.cars[ci].state == CarState::Waiting && self.cars[ci].timer <= 0.0)
    }

    pub fn tick_dispatch(&mut self, dt: f32) {
        self.dispatch_timer += dt;
        if self.dispatch_timer < DISPATCH_EVERY {
            return;
        }
        self.dispatch_timer = 0.0;
        let mut order: Vec<(u32, usize)> = (0..self.dests.len())
            .filter_map(|i| {
                let d = &self.dests[i];
                let free = d.pins.saturating_sub(d.assigned);
                if free > 0 {
                    Some((free, i))
                } else {
                    None
                }
            })
            .collect();
        order.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, di) in order {
            self.dispatch_to(di);
        }
    }

    fn dispatch_to(&mut self, di: usize) {
        let group = self.dests[di].group;
        let mut need = self.dests[di].pins.saturating_sub(self.dests[di].assigned);
        if need == 0 || !self.houses.iter().any(|h| h.group == group) {
            return;
        }
        let sources = self.dest_cells(di);
        let (dist, prev) = self.dijkstra(&sources);
        let mut cands: Vec<(u32, usize)> = (0..self.houses.len())
            .filter(|&hi| {
                let h = &self.houses[hi];
                h.group == group && dist[h.cell as usize] != UNREACHED
            })
            .map(|hi| (dist[self.houses[hi].cell as usize], hi))
            .collect();
        cands.sort();
        for (_, hi) in cands {
            while need > 0 {
                let ci = match self.waiting_car(hi) {
                    Some(ci) => ci,
                    None => break,
                };
                let path = match Self::chain(&prev, &dist, self.houses[hi].cell) {
                    Some(p) if p.len() >= 2 => p,
                    _ => break,
                };
                let car = &mut self.cars[ci];
                car.path = path;
                car.idx = 0;
                car.t = 0.0;
                car.v = 0.0;
                car.wait = 0.0;
                car.dest = di;
                car.state = CarState::ToDest;
                car.hold = NONE;
                car.ver = self.topo_ver;
                self.dests[di].assigned += 1;
                need -= 1;
            }
            if need == 0 {
                break;
            }
        }
    }
}
