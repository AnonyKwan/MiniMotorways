//! Road building rules (doc 04.2 to 04.5) and weekly upgrades (doc 08).
mod common;
use common::*;
use little_lanes_core::cities::*;
use little_lanes_core::game::*;
use little_lanes_core::roads::*;
use little_lanes_core::terrain::{MOUNTAIN, WATER};

fn lay(g: &mut Game, x: i32, y: i32, n: i32) -> Vec<u32> {
    let cells: Vec<u32> = (0..n).map(|k| g.cell(x + k, y)).collect();
    for w in cells.windows(2) {
        assert_eq!(g.add_road(w[0], w[1]), OK);
    }
    cells
}

#[test]
fn a_road_connection_costs_one_concrete_and_removal_refunds_it() {
    let mut g = Game::new(city_index("beijing"), 1);
    let (x, y) = land_run(&g, 5).expect("land");
    let cells = lay(&mut g, x, y, 5);
    assert_eq!(g.used(K_CONCRETE), 4);
    assert_eq!(g.available(K_CONCRETE), 26);
    assert_eq!(g.remove_road(cells[1], cells[2]), OK);
    assert_eq!(g.used(K_CONCRETE), 3);
}

#[test]
fn diagonal_roads_cost_the_same_as_straight_ones() {
    let mut g = Game::new(city_index("beijing"), 1);
    let (x, y) = land_run(&g, 3).expect("land");
    let (a, b) = (g.cell(x, y), g.cell(x + 1, y + 1));
    assert_eq!(g.add_road(a, b), OK);
    assert_eq!(g.used(K_CONCRETE), 1);
}

#[test]
fn crossing_diagonals_are_refused() {
    let mut g = Game::new(city_index("beijing"), 1);
    let (x, y) = land_run(&g, 4).expect("land");
    assert_eq!(g.add_road(g.cell(x, y), g.cell(x + 1, y + 1)), OK);
    assert_eq!(g.add_road(g.cell(x + 1, y), g.cell(x, y + 1)), E_CROSSING);
}

#[test]
fn duplicate_and_far_connections_are_refused() {
    let mut g = Game::new(city_index("beijing"), 1);
    let (x, y) = land_run(&g, 4).expect("land");
    let (a, b) = (g.cell(x, y), g.cell(x + 1, y));
    assert_eq!(g.add_road(a, b), OK);
    assert_eq!(g.add_road(a, b), E_EXISTS);
    assert_eq!(g.add_road(a, g.cell(x + 3, y)), E_NOT_ADJACENT);
}

#[test]
fn running_out_of_concrete_stops_building() {
    let mut g = Game::new(city_index("beijing"), 1);
    g.owned[0] = 3;
    let (x, y) = land_run(&g, 6).expect("land");
    let cells: Vec<u32> = (0..6).map(|k| g.cell(x + k, y)).collect();
    assert_eq!(g.add_road(cells[0], cells[1]), OK);
    assert_eq!(g.add_road(cells[1], cells[2]), OK);
    assert_eq!(g.add_road(cells[2], cells[3]), OK);
    assert_eq!(g.add_road(cells[3], cells[4]), E_NO_CONCRETE);
}

#[test]
fn water_needs_a_bridge_and_one_bridge_covers_a_whole_crossing() {
    let id = "reykjavik";
    let mut g = Game::new(city_index(id), 2);
    let (x, y) = land_over(&g, WATER).expect("a land-water-land strip");
    let (a, b, c) = (g.cell(x, y), g.cell(x + 1, y), g.cell(x + 2, y));
    g.owned[K_BRIDGE as usize] = 0;
    assert_eq!(g.add_road(a, b), E_NO_BRIDGE);
    assert_eq!(g.used(K_CONCRETE), 0, "a refused road costs nothing");
    g.owned[K_BRIDGE as usize] = 1;
    assert_eq!(g.add_road(a, b), OK);
    assert_eq!(g.add_road(b, c), OK);
    assert_eq!(g.used(K_BRIDGE), 1);
    assert_eq!(g.available(K_BRIDGE), 0);
    // removing the whole crossing gives the bridge back
    g.remove_road(a, b);
    g.remove_road(b, c);
    assert_eq!(g.used(K_BRIDGE), 0);
}

#[test]
fn mountains_need_a_tunnel() {
    let mut g = Game::new(city_index("cairns"), 2);
    let (x, y) = land_over(&g, MOUNTAIN).expect("a land-mountain-land strip");
    let (a, b) = (g.cell(x, y), g.cell(x + 1, y));
    g.owned[K_TUNNEL as usize] = 0;
    assert_eq!(g.add_road(a, b), E_NO_TUNNEL);
    g.owned[K_TUNNEL as usize] = 1;
    assert_eq!(g.add_road(a, b), OK);
    assert_eq!(g.used(K_TUNNEL), 1);
}

#[test]
fn motorways_need_two_cells_and_an_upgrade() {
    let mut g = Game::new(city_index("hongkong"), 2);
    let (x, y) = land_run(&g, 6).expect("land");
    let (a, near, far) = (g.cell(x, y), g.cell(x + 1, y), g.cell(x + 5, y));
    assert_eq!(g.add_motorway(a, near), E_TOO_SHORT);
    assert_eq!(g.add_motorway(a, far), OK);
    assert_eq!(g.used(K_MOTORWAY), 1);
    assert_eq!(g.used(K_CONCRETE), 0, "motorways use no concrete");
    assert_eq!(g.add_motorway(g.cell(x + 1, y), g.cell(x + 4, y)), E_NO_UPGRADE);
    assert_eq!(g.remove_motorway(0), OK);
    assert_eq!(g.available(K_MOTORWAY), 1);
}

#[test]
fn lights_and_roundabouts_only_go_on_junctions() {
    let mut g = Game::new(city_index("lisbon"), 2); // starts with 2 traffic lights
    let (x, y) = land_run(&g, 3).expect("land");
    let centre = g.cell(x + 1, y);
    assert_eq!(g.toggle_light(centre), E_BAD_PLACEMENT, "no road yet");
    for d in [0usize, 2, 6] {
        let m = g.step_cell(centre, d).unwrap();
        assert_eq!(g.add_road(centre, m), OK);
    }
    assert!(g.is_junction(centre));
    assert_eq!(g.toggle_roundabout(centre), E_NO_UPGRADE, "lisbon has no roundabout yet");
    assert_eq!(g.toggle_light(centre), OK);
    assert_eq!(g.used(K_LIGHT), 1);
    assert_eq!(g.toggle_light(centre), OK, "toggling again removes it");
    assert_eq!(g.used(K_LIGHT), 0);
    assert_eq!(g.toggle_light(centre), OK);
    // removing a road until it is no longer a junction drops the light
    let m = g.step_cell(centre, 0).unwrap();
    g.remove_road(centre, m);
    assert_eq!(g.used(K_LIGHT), 0);
}

#[test]
fn the_first_offer_comes_after_one_week_with_two_choices() {
    let mut g = Game::new(city_index("beijing"), 4);
    run(&mut g, 139.0);
    assert!(!g.offer_open);
    run(&mut g, 2.0);
    assert!(g.offer_open);
    assert_eq!(g.offers.len(), 2);
    let before = g.owned;
    let pack = g.offers[0].clone();
    assert_eq!(g.choose_upgrade(0), OK);
    assert_eq!(g.owned[pack.kind as usize], before[pack.kind as usize] + pack.amount + if pack.kind == 0 { pack.add_concrete } else { 0 });
    assert!(!g.offer_open);
    assert_eq!(g.choose_upgrade(0), E_NO_OFFER);
}

#[test]
fn the_week_clock_waits_while_an_offer_is_open() {
    let mut g = Game::new(city_index("beijing"), 4);
    run(&mut g, 141.0);
    assert!(g.offer_open);
    run(&mut g, 200.0);
    assert!(g.week_timer < 1.0 || g.over);
    assert_eq!(g.week, 1);
}

#[test]
fn rules_respect_their_start_week() {
    // Beijing's motorway package only appears from the fourth offer on.
    for seed in 0..40 {
        let mut g = Game::new(city_index("beijing"), seed);
        g.make_offer();
        assert!(g.offers.iter().all(|p| p.kind != K_MOTORWAY), "seed {seed}");
        assert_eq!(g.offers.len(), 2);
    }
}

#[test]
fn motorway_packages_get_more_likely_the_longer_they_are_missing() {
    // weight 1.5 + 1.5 per missed week, only for weeks 3-4: by week 3 it is very likely
    let mut hits = 0;
    for seed in 0..60 {
        let mut g = Game::new(city_index("beijing"), seed);
        for _ in 0..3 {
            g.make_offer();
            g.choose_upgrade(0);
        }
        g.make_offer();
        if g.offers.iter().any(|p| p.kind == K_MOTORWAY) {
            hits += 1;
        }
    }
    assert!(hits > 30, "motorway offered in {hits}/60 games");
}
