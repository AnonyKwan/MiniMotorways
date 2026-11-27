//! A simple balance bot: it connects every home to the nearest shop of its colour, reusing
//! roads where it can, and picks road tiles when it runs low. It never uses roundabouts,
//! lights or highways, so it is a floor for how long a careful player can last.
//! Run with: cargo test --release --test bot -- --ignored --nocapture
use little_lanes::map::{idx, in_bounds, MapKind, Pos, Terrain, DIRS, H, W};
use little_lanes::sim::{Cell, Sim, Upgrade, DT, WEEK_LENGTH};
use std::collections::VecDeque;

fn connected(s: &Sim, h: usize) -> bool {
    let color = s.homes[h].color;
    s.find_path(s.homes[h].pos, &|p| matches!(s.cell(p), Cell::Door(d) if s.dests[d].color == color)).is_some()
}

/// Cheapest tiles to build from next to home `h` to a door of its colour (0-1 search: roads free).
fn plan(s: &Sim, h: usize) -> Option<Vec<Pos>> {
    let home = s.homes[h].pos;
    let color = s.homes[h].color;
    let n = (W * H) as usize;
    let mut cost = vec![u32::MAX; n];
    let mut from = vec![usize::MAX; n];
    let mut dq = VecDeque::new();
    let buildable = |p: Pos| match (s.cell(p), s.terrain[idx(p)]) {
        (Cell::Road, _) => Some(0),
        (Cell::Empty, Terrain::Grass) => Some(1),
        (Cell::Empty, Terrain::Water) if s.inventory.bridge_tiles > 0 => Some(3),
        (Cell::Empty, Terrain::Hill) if s.inventory.tunnel_tiles > 0 => Some(3),
        _ => None,
    };
    for d in DIRS {
        let q = (home.0 + d.0, home.1 + d.1);
        if in_bounds(q) {
            if let Some(c) = buildable(q) {
                cost[idx(q)] = c;
                dq.push_back(q);
            }
        }
    }
    let mut best: Option<(u32, Pos)> = None;
    while let Some(p) = dq.pop_front() {
        let c = cost[idx(p)];
        for d in DIRS {
            let q = (p.0 + d.0, p.1 + d.1);
            if !in_bounds(q) {
                continue;
            }
            if let Cell::Door(di) = s.cell(q) {
                if s.dests[di].color == color && best.is_none_or(|b| c < b.0) {
                    best = Some((c, p));
                }
                continue;
            }
            if let Some(step) = buildable(q) {
                if c + step < cost[idx(q)] {
                    cost[idx(q)] = c + step;
                    from[idx(q)] = idx(p);
                    dq.push_back(q);
                }
            }
        }
    }
    let (_, mut p) = best?;
    let mut out = vec![p];
    while from[idx(p)] != usize::MAX {
        let i = from[idx(p)];
        p = ((i as i32) % W, (i as i32) / W);
        out.push(p);
    }
    Some(out)
}

/// Tiles to build from shop `d` to the driveway of the nearest home of its colour.
fn plan_shop(s: &Sim, d: usize) -> Option<Vec<Pos>> {
    let color = s.dests[d].color;
    let goals: Vec<Pos> = (0..s.homes.len()).filter(|h| s.homes[*h].color == color).map(|h| s.driveway(h)).collect();
    let n = (W * H) as usize;
    let mut cost = vec![u32::MAX; n];
    let mut from = vec![usize::MAX; n];
    let mut dq = VecDeque::new();
    let buildable = |p: Pos| match (s.cell(p), s.terrain[idx(p)]) {
        (Cell::Road, _) => Some(0),
        (Cell::Empty, Terrain::Grass) => Some(1),
        (Cell::Empty, Terrain::Water) if s.inventory.bridge_tiles > 0 => Some(3),
        (Cell::Empty, Terrain::Hill) if s.inventory.tunnel_tiles > 0 => Some(3),
        _ => None,
    };
    for door in &s.dests[d].doors {
        let q = s.dests[d].entry(*door);
        if in_bounds(q) {
            if let Some(c) = buildable(q) {
                cost[idx(q)] = c;
                dq.push_back(q);
            }
        }
    }
    let mut best: Option<(u32, Pos)> = None;
    while let Some(p) = dq.pop_front() {
        let c = cost[idx(p)];
        if goals.contains(&p) && best.is_none_or(|b| c < b.0) {
            best = Some((c, p));
        }
        for dd in DIRS {
            let q = (p.0 + dd.0, p.1 + dd.1);
            if !in_bounds(q) {
                continue;
            }
            if let Some(step) = buildable(q) {
                if c + step < cost[idx(q)] {
                    cost[idx(q)] = c + step;
                    from[idx(q)] = idx(p);
                    dq.push_back(q);
                }
            }
        }
    }
    let (_, mut p) = best?;
    let mut out = vec![p];
    while from[idx(p)] != usize::MAX {
        let i = from[idx(p)];
        p = ((i as i32) % W, (i as i32) / W);
        out.push(p);
    }
    Some(out)
}

fn shop_served(s: &Sim, d: usize) -> bool {
    let color = s.dests[d].color;
    s.dests[d].doors.iter().any(|door| {
        s.find_path(*door, &|p| matches!(s.cell(p), Cell::Home(h) if s.homes[h].color == color)).is_some()
    })
}

fn play(map: MapKind, seed: u64, weeks: u32) -> (u32, u32, bool) {
    let mut s = Sim::new_on(map, seed);
    let mut t = 0.0;
    let mut last: Vec<(Pos, usize, f32)> = Vec::new();
    while !s.game_over && s.week <= weeks {
        last.resize(s.cars.len(), ((0, 0), 0, 0.0));
        for (i, c) in s.cars.iter().enumerate() {
            let busy = !matches!(c.state, little_lanes::sim::CarState::Idle | little_lanes::sim::CarState::Unloading(_));
            if !busy || last[i].0 != c.cur || last[i].1 != c.i {
                last[i] = (c.cur, c.i, s.time);
            }
        }
        if let Some(o) = s.offer {
            let want = if s.roads_left < 25 { o.iter().position(|u| *u == Upgrade::ExtraRoads) } else { None };
            let alt = o.iter().position(|u| *u == Upgrade::Roundabout)
                .or(o.iter().position(|u| *u == Upgrade::TrafficLight))
                .or(o.iter().position(|u| matches!(u, Upgrade::Bridge | Upgrade::Tunnel | Upgrade::ExtraRoads)));
            s.choose(want.or(alt).unwrap_or(0));
        }
        t += DT;
        if t >= 1.0 {
            t = 0.0;
            // Put junction upgrades on the busiest plain crossings.
            if s.inventory.roundabouts + s.inventory.lights > 0 {
                let mut xs: Vec<(usize, Pos)> = (0..W * H)
                    .map(|i| (i % W, i / W))
                    .filter(|p| s.is_crossing(*p) && s.junctions[idx(*p)] == little_lanes::sim::Junction::Plain)
                    .map(|p| (s.link_count(p), p))
                    .collect();
                xs.sort_by(|a, b| b.0.cmp(&a.0));
                for (_, p) in xs {
                    if s.inventory.roundabouts > 0 && s.place_junction(p, little_lanes::sim::Junction::Roundabout) == little_lanes::sim::EditResult::Ok {
                        continue;
                    }
                    if s.inventory.lights > 0 {
                        s.place_junction(p, little_lanes::sim::Junction::Light);
                    }
                }
            }
            for d in 0..s.dests.len() {
                if !shop_served(&s, d) {
                    if let Some(path) = plan_shop(&s, d) {
                        for p in path {
                            s.place_road(p);
                        }
                    }
                }
            }
            for h in 0..s.homes.len() {
                if !connected(&s, h) {
                    if let Some(path) = plan(&s, h) {
                        for p in path {
                            s.place_road(p);
                        }
                    }
                }
            }
        }
        s.step();
    }
    if s.game_over {
        let f = s.failed_dest.unwrap_or(0);
        let d = &s.dests[f];
        let homes = s.homes.iter().filter(|h| h.color == d.color).count();
        let unconnected = (0..s.homes.len()).filter(|h| s.homes[*h].color == d.color && !connected(&s, *h)).count();
        let shops = s.dests.iter().filter(|e| e.color == d.color).count();
        let mut st = std::collections::BTreeMap::new();
        for (i, c) in s.cars.iter().enumerate() {
            if s.homes[i].color == d.color {
                *st.entry(format!("{:?}", c.state).chars().take(6).collect::<String>()).or_insert(0) += 1;
            }
        }
        let waiting = s.cars.iter().enumerate().filter(|(i, c)| s.homes[*i].color == d.color && c.progress < 1.0).count();
        let stuck: Vec<(usize, Pos, i32, f32)> = last.iter().enumerate().filter(|(_, l)| s.time - l.2 > 15.0).map(|(i, l)| (i, l.0, s.link_count(l.0) as i32, s.time - l.2)).collect();
        eprintln!("   stuck >15s: {:?}", stuck);
        eprintln!("   cars {:?}, moving {}, pins {} assigned {}", st, waiting, d.pins, d.assigned);
        eprintln!("   served {} plan {:?} bridge {} tunnel {} door {:?} front cell {:?} terrain {:?}", shop_served(&s, f), plan_shop(&s, f).map(|p| p.len()), s.inventory.bridge_tiles, s.inventory.tunnel_tiles, d.doors, s.cell(d.entry(d.doors[0])), s.terrain[idx(d.entry(d.doors[0]))]);
        eprintln!("   failed shop colour {} big {}: {} homes ({} unconnected) for {} shops, roads left {}, total shops {}",
            d.color, d.big(), homes, unconnected, shops, s.roads_left, s.dests.len());
    }
    (s.week, s.score, s.game_over)
}

#[test]
#[ignore]
fn balance_report() {
    let weeks = 20;
    for map in MapKind::ALL {
        let mut lasted = Vec::new();
        for seed in 1..=6 {
            let (w, score, over) = play(map, seed, weeks);
            lasted.push(if over { w } else { weeks + 1 });
            eprintln!("{:?} seed {}: {} at week {}, {} trips", map, seed, if over { "gridlock" } else { "survived" }, w, score);
        }
        lasted.sort();
        eprintln!("{:?}: median end week {}", map, lasted[lasted.len() / 2]);
    }
    let _ = WEEK_LENGTH;
}
