//! Whole-game behaviour: buildings appear, cars deliver, the world is deterministic.
mod common;
use common::*;
use little_lanes_core::cities::*;
use little_lanes_core::game::*;
use little_lanes_core::snapshot::catalog;

#[test]
fn the_world_is_deterministic() {
    let mut a = Game::new(city_index("london"), 77);
    let mut b = Game::new(city_index("london"), 77);
    run(&mut a, 90.0);
    run(&mut b, 90.0);
    assert_eq!(a.snapshot(), b.snapshot());
    assert_eq!(a.statics(), b.statics());
    let mut c = Game::new(city_index("london"), 78);
    run(&mut c, 90.0);
    assert_ne!(a.statics(), c.statics(), "a different seed gives a different town");
}

#[test]
fn houses_appear_for_each_destination_colour() {
    let mut g = Game::new(city_index("beijing"), 1);
    run(&mut g, 60.0);
    assert!(!g.dests.is_empty());
    assert!(!g.houses.is_empty());
    for h in &g.houses {
        assert!(g.dests.iter().any(|d| d.group == h.group), "a house only exists for a colour that has a destination");
    }
    for h in &g.houses {
        assert_eq!(h.cars.len(), 2, "two cars per house");
    }
}

#[test]
fn buildings_never_overlap_water_roads_or_each_other() {
    let mut g = Game::new(city_index("capetown"), 9);
    run(&mut g, 200.0);
    let mut seen = std::collections::HashSet::new();
    for d in 0..g.dests.len() {
        for c in g.dest_cells(d) {
            assert!(seen.insert(c), "cell used twice");
            assert_eq!(g.terrain.kind[c as usize], 0, "destination on land");
        }
    }
    for h in &g.houses {
        assert!(seen.insert(h.cell), "house cell used twice");
        assert_eq!(g.terrain.kind[h.cell as usize], 0);
    }
}

#[test]
fn a_connected_house_delivers_pins_and_scores() {
    let mut g = Game::new(city_index("beijing"), 1);
    run(&mut g, 25.0);
    bot_connect_all(&mut g);
    run(&mut g, 120.0);
    assert!(g.score > 0, "cars delivered {} pins", g.score);
    assert!(g.cars.iter().any(|c| c.state != CarState::Waiting) || g.score > 0);
}

#[test]
fn an_unconnected_town_gets_overcrowded() {
    let mut g = Game::new(city_index("beijing"), 1);
    run(&mut g, 400.0);
    assert!(g.over, "with no roads the game must end");
    assert_eq!(g.score, 0);
}

#[test]
fn erasing_a_road_under_a_car_does_not_lose_it() {
    let mut g = Game::new(city_index("beijing"), 1);
    run(&mut g, 25.0);
    bot_connect_all(&mut g);
    for _ in 0..40 {
        run(&mut g, 3.0);
        // cut a random road cell now and then
        let roads: Vec<u32> = (0..g.conn.len() as u32).filter(|&c| g.conn[c as usize] != 0 && g.bkind[c as usize] == B_NONE).collect();
        if !roads.is_empty() {
            let c = roads[(g.time as usize) % roads.len()];
            g.erase_cell(c);
        }
        bot_connect_all(&mut g);
    }
    // every car is somewhere sensible: waiting at home, or on a path whose edges exist
    for (i, c) in g.cars.iter().enumerate() {
        if matches!(c.state, CarState::ToDest | CarState::ToHome) && c.ver == g.topo_ver {
            for k in c.idx..c.path.len().saturating_sub(1) {
                assert!(g.has_edge(c.path[k], c.path[k + 1]), "car {i} drives on a missing road");
            }
        }
    }
    for d in &g.dests {
        let on_way = g.cars.iter().filter(|c| c.state == CarState::ToDest).count() as u32;
        assert!(d.assigned <= d.pins.max(on_way) + 1);
    }
}

#[test]
fn no_two_cars_share_a_single_lane_junction() {
    let mut g = Game::new(city_index("beijing"), 3);
    run(&mut g, 30.0);
    for _ in 0..30 {
        bot_connect_all(&mut g);
        run(&mut g, 4.0);
        for (n, slots) in g.res.iter().enumerate() {
            let held: Vec<u32> = slots.iter().copied().filter(|&s| s != NONE).collect();
            let cap = if g.round[n] { 4 } else { 1 };
            assert!(held.len() <= cap, "node {n} holds {} cars", held.len());
        }
    }
}

#[test]
fn snapshot_is_always_valid_numbers() {
    let mut g = Game::new(city_index("tokyo"), 4);
    run(&mut g, 60.0);
    bot_connect_all(&mut g);
    run(&mut g, 60.0);
    let s = g.snapshot();
    assert!(!s.contains("NaN") && !s.contains("inf"), "{s}");
    assert!(s.starts_with('{') && s.ends_with('}'));
    let st = g.statics();
    assert!(st.starts_with('{') && st.ends_with('}'));
}

#[test]
fn catalog_lists_every_city_once() {
    let c = catalog();
    for city in CITIES {
        assert_eq!(c.matches(&format!("\"id\":\"{}\"", city.id)).count(), 1, "{}", city.id);
    }
}

#[test]
fn every_city_runs_for_three_game_weeks_without_trouble() {
    for ci in 0..CITIES.len() {
        let mut g = Game::new(ci, 123);
        for _ in 0..3 {
            run(&mut g, 60.0);
            bot_connect_all(&mut g);
            if g.offer_open {
                g.choose_upgrade(0);
            }
        }
        run(&mut g, 120.0);
        let s = g.snapshot();
        assert!(!s.contains("NaN"), "{}", CITIES[ci].id);
        assert!(g.time > 100.0 || g.over, "{}", CITIES[ci].id);
        assert!(!g.dests.is_empty(), "{} has destinations", CITIES[ci].id);
    }
}
