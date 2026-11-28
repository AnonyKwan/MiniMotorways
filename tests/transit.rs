//! Trains and ferries.
mod common;
use common::*;
use little_lanes_core::cities::*;
use little_lanes_core::game::*;

#[test]
fn only_the_design_maps_have_a_railway_or_a_ferry() {
    for (i, c) in CITIES.iter().enumerate() {
        let g = Game::new(i, 1);
        let rails = g.terrain.rail.iter().filter(|&&r| r).count();
        assert_eq!(rails > 0, c.trains > 0, "{} railway", c.id);
        assert_eq!(g.terrain.boat.is_some(), c.boats > 0, "{} ferry", c.id);
        assert_eq!(g.transits.len(), (c.trains + c.boats) as usize, "{} transits", c.id);
    }
    for id in ["london", "copenhagen", "mumbai", "newyorkcity"] {
        assert_eq!(CITIES[city_index(id)].trains, 1, "{id}");
    }
    for id in ["hongkong", "istanbul"] {
        assert_eq!(CITIES[city_index(id)].boats, 1, "{id}");
    }
}

#[test]
fn the_railway_crosses_the_whole_map_on_dry_land() {
    let g = Game::new(city_index("london"), 4);
    let y = g.terrain.rail_y;
    assert!(y > 0);
    for x in 0..g.w {
        let c = g.cell(x, y) as usize;
        assert!(g.terrain.rail[c]);
        assert_eq!(g.terrain.kind[c], 0, "rails never run over water");
        assert!(!g.terrain.tree[c]);
    }
    let bytes = g.terrain_bytes();
    assert_eq!(bytes.iter().filter(|&&b| b & 8 != 0).count() as i32, g.w);
}

#[test]
fn a_station_appears_beside_the_line_and_trains_bring_pins() {
    let mut found = false;
    for seed in 0..4 {
        let mut g = Game::new(city_index("london"), seed);
        run(&mut g, 400.0);
        let st = match g.dests.iter().position(|d| d.kind == 1) {
            Some(s) => s,
            None => continue,
        };
        found = true;
        let (_, y) = g.xy(g.dests[st].cell);
        assert!(y + 2 == g.terrain.rail_y || y == g.terrain.rail_y + 1, "next to the rails");
        assert!(g.transit_arrivals >= 1, "the train stopped at the station");
        assert!(g.dests[st].pins >= 1, "and left pins behind");
        assert!(g.dests[st].pin_timer.is_infinite(), "a station has no timer of its own");
    }
    assert!(found, "at least one seed placed the station");
}

#[test]
fn the_train_goes_back_and_forth() {
    let mut g = Game::new(city_index("london"), 4);
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    for _ in 0..4000 {
        g.advance(0.05);
        let x = g.transits[0].x;
        min = min.min(x);
        max = max.max(x);
    }
    assert!(max > g.w as f32, "runs off the right edge");
    assert!(min < 0.0, "and back off the left");
}

#[test]
fn cars_wait_while_a_train_is_at_the_crossing() {
    let mut g = Game::new(city_index("london"), 4);
    let c = g.cell(10, g.terrain.rail_y);
    g.transits[0].wait = 0.0;
    g.transits[0].x = 10.5;
    assert!(g.train_near(c));
    g.transits[0].x = 30.0;
    assert!(!g.train_near(c));
    let off_rail = g.cell(10, g.terrain.rail_y - 3);
    g.transits[0].x = 10.5;
    assert!(!g.train_near(off_rail), "only rail cells have crossings");
}

#[test]
fn roads_can_cross_the_railway() {
    let mut g = Game::new(city_index("london"), 4);
    let y = g.terrain.rail_y;
    let (a, b, c) = (g.cell(12, y - 1), g.cell(12, y), g.cell(12, y + 1));
    assert_eq!(g.add_road(a, b), little_lanes_core::roads::OK);
    assert_eq!(g.add_road(b, c), little_lanes_core::roads::OK);
}

#[test]
fn a_ferry_shuttles_and_the_terminal_gets_pins() {
    let mut found = false;
    for seed in 0..4 {
        let mut g = Game::with_mode(city_index("hongkong"), seed, MODE_ENDLESS); // no game over before the terminal opens
        assert!(g.transits.iter().any(|t| t.kind == 1));
        run(&mut g, 700.0);
        if let Some(t) = g.dests.iter().position(|d| d.kind == 2) {
            found = true;
            assert!(g.dests[t].pins >= 1, "the ferry dropped people off");
            let (tx, ty) = g.terrain.terminal.unwrap();
            assert_eq!(g.xy(g.dests[t].cell), (tx, ty), "terminal sits at the water's edge");
        }
    }
    assert!(found);
}

#[test]
fn the_ferry_channel_is_water_and_the_shore_is_land() {
    let g = Game::new(city_index("istanbul"), 9);
    let (yb, wx0, wx1) = g.terrain.boat.unwrap();
    for x in wx0..=wx1 {
        assert_eq!(g.terrain.kind[g.cell(x, yb) as usize], 1);
    }
    let (tx, ty) = g.terrain.terminal.unwrap();
    for (x, y) in [(tx, ty), (tx + 1, ty), (tx, ty + 1), (tx + 1, ty + 1)] {
        assert_eq!(g.terrain.kind[g.cell(x, y) as usize], 0);
    }
    assert_eq!(tx + 2, wx0, "the water starts right next to the terminal");
}

#[test]
fn snapshots_describe_transits() {
    let mut g = Game::new(city_index("london"), 4);
    run(&mut g, 5.0);
    assert!(g.snapshot().contains("\"transit\":[["));
    assert!(g.statics().contains("\"railY\":"));
    let plain = Game::new(city_index("beijing"), 4);
    assert!(plain.snapshot().contains("\"transit\":[]"));
}
