#![allow(dead_code)]
use little_lanes_core::cities::CITIES;
use little_lanes_core::game::*;
use little_lanes_core::roads::{E_EXISTS, OK};
use little_lanes_core::terrain::LAND;
use std::collections::VecDeque;

pub fn city_index(id: &str) -> usize {
    CITIES.iter().position(|c| c.id == id).unwrap_or_else(|| panic!("no city {id}"))
}

/// Runs the simulation for `secs` of game time.
pub fn run(g: &mut Game, secs: f32) {
    let steps = (secs / 0.05) as usize;
    for _ in 0..steps {
        g.advance(0.05);
    }
}

/// Finds `n` consecutive free land cells in a row, starting from the left.
pub fn land_run(g: &Game, n: i32) -> Option<(i32, i32)> {
    for y in 2..g.h - 2 {
        'x: for x in 2..g.w - n - 2 {
            for k in 0..n {
                let c = g.cell(x + k, y) as usize;
                if g.terrain.kind[c] != LAND || g.bkind[c] != B_NONE {
                    continue 'x;
                }
            }
            return Some((x, y));
        }
    }
    None
}

/// Finds a horizontal "land, <kind>, land" triple and returns the left cell.
pub fn land_over(g: &Game, kind: u8) -> Option<(i32, i32)> {
    for y in 1..g.h - 1 {
        for x in 1..g.w - 3 {
            let k = |dx: i32| g.terrain.kind[g.cell(x + dx, y) as usize];
            if k(0) == LAND && k(1) == kind && k(2) == LAND {
                return Some((x, y));
            }
        }
    }
    None
}

/// Lays a road with straight (up/down/left/right) steps from `from` to a cell next to the target set.
pub fn connect(g: &mut Game, from: u32, goals: &[u32]) -> bool {
    let n = g.conn.len();
    let mut prev = vec![NONE; n];
    let mut seen = vec![false; n];
    let mut q = VecDeque::new();
    seen[from as usize] = true;
    q.push_back(from);
    let mut found = NONE;
    while let Some(c) = q.pop_front() {
        if goals.contains(&c) && c != from {
            found = c;
            break;
        }
        for d in [0usize, 2, 4, 6] {
            if let Some(m) = g.step_cell(c, d) {
                let mi = m as usize;
                let open = g.terrain.kind[mi] == LAND && (g.bkind[mi] == B_NONE || goals.contains(&m));
                if open && !seen[mi] {
                    seen[mi] = true;
                    prev[mi] = c;
                    q.push_back(m);
                }
            }
        }
    }
    if found == NONE {
        return false;
    }
    let mut path = vec![found];
    let mut cur = found;
    while prev[cur as usize] != NONE {
        cur = prev[cur as usize];
        path.push(cur);
    }
    path.reverse();
    for w in path.windows(2) {
        let r = g.add_road(w[0], w[1]);
        if r != OK && r != E_EXISTS {
            return false;
        }
    }
    true
}

/// A very simple player: joins every house to a destination of its colour whenever concrete allows.
pub fn bot_connect_all(g: &mut Game) {
    for hi in 0..g.houses.len() {
        let (cell, group) = (g.houses[hi].cell, g.houses[hi].group);
        let mut targets: Vec<(f32, usize)> = (0..g.dests.len())
            .filter(|&di| g.dests[di].group == group)
            .map(|di| {
                let (hx, hy) = g.centre(cell);
                let (dx, dy) = g.dest_centre(di);
                ((hx - dx).powi(2) + (hy - dy).powi(2), di)
            })
            .collect();
        targets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        if let Some(&(_, di)) = targets.first() {
            let cells = g.dest_cells(di).to_vec();
            if g.conn[cell as usize] == 0 {
                connect(g, cell, &cells);
            }
        }
    }
}
