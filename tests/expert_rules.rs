//! Expert-mode upgrade rules and lazy road deletion.
mod common;
use common::*;
use little_lanes_core::cities::*;
use little_lanes_core::game::*;
use little_lanes_core::roads::*;

#[test]
fn expert_offers_the_whole_library_for_eight_weeks_then_only_tiles() {
    let mut g = Game::with_mode(city_index("beijing"), 3, MODE_EXPERT);
    let mut max_offered = 0;
    for week in 0..12 {
        g.make_offer();
        assert!(g.offer_open);
        if week < 8 {
            max_offered = max_offered.max(g.offers.len());
            let kinds: std::collections::HashSet<u8> = g.offers.iter().map(|p| p.kind).collect();
            assert_eq!(kinds.len(), g.offers.len(), "one package per kind");
        } else {
            assert_eq!(g.offers.len(), 1, "week {week}");
            assert_eq!(g.offers[0].kind, 0, "only road tiles after the eighth week");
        }
        g.choose_upgrade(0);
    }
    assert!(max_offered >= 3, "several options at once, got {max_offered}");
}

#[test]
fn expert_never_offers_the_same_kind_twice_in_a_row() {
    for seed in 0..10 {
        let mut g = Game::with_mode(city_index("beijing"), seed, MODE_EXPERT);
        let mut last = None;
        for _ in 0..8 {
            g.make_offer();
            if let Some(k) = last {
                assert!(g.offers.iter().all(|p| p.kind != k), "seed {seed}: repeated kind {k}");
            }
            last = Some(g.offers[0].kind);
            g.choose_upgrade(0);
        }
    }
}

#[test]
fn classic_still_offers_two() {
    let mut g = Game::new(city_index("beijing"), 3);
    for _ in 0..12 {
        g.make_offer();
        assert_eq!(g.offers.len(), 2);
        g.choose_upgrade(0);
    }
}

#[test]
fn a_deleted_road_keeps_working_for_the_cars_on_it() {
    let mut g = Game::new(city_index("beijing"), 1);
    run(&mut g, 25.0);
    bot_connect_all(&mut g);
    // wait for a car to be on the road, then delete the stretch under it
    let mut victim = None;
    for _ in 0..600 {
        g.advance(0.1);
        if let Some(c) = g.cars.iter().find(|c| matches!(c.state, CarState::ToDest | CarState::ToHome) && c.idx + 1 < c.path.len() && c.path.len() > 3) {
            victim = Some((c.path[c.idx], c.path[c.idx + 1]));
            break;
        }
    }
    let (a, b) = victim.expect("a car on the road");
    let before = g.used(K_CONCRETE);
    if g.remove_road(a, b) == OK {
        assert!(g.used(K_CONCRETE) <= before, "the concrete comes back at once");
        assert!(!g.has_ground_edge(a, b), "the road is gone for planning");
        assert!(g.has_edge(a, b), "but still usable by the car on it");
        assert!(g.ghosts.contains(&(a, b)));
        run(&mut g, 30.0);
        assert!(!g.ghosts.contains(&(a, b)), "the ghost road disappears once the cars are off");
        assert!(!g.has_edge(a, b));
    }
}

#[test]
fn nobody_plans_a_route_over_a_ghost_road() {
    let mut g = Game::new(city_index("beijing"), 2);
    let (x, y) = land_run(&g, 4).unwrap();
    let cells: Vec<u32> = (0..4).map(|k| g.cell(x + k, y)).collect();
    for w in cells.windows(2) {
        g.add_road(w[0], w[1]);
    }
    g.ghosts.push((cells[1], cells[2]));
    g.remove_road(cells[1], cells[2]);
    let route = g.route_from(cells[0], &[cells[3]]);
    assert!(route.is_none(), "dijkstra only follows real roads");
}
