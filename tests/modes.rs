//! Game modes: Classic, Expert (permanent roads) and Creative.
mod common;
use common::*;
use little_lanes_core::cities::*;
use little_lanes_core::game::*;
use little_lanes_core::roads::*;

fn two_cells(g: &Game) -> (u32, u32) {
    let (x, y) = land_run(g, 3).expect("land");
    (g.cell(x, y), g.cell(x + 1, y))
}

#[test]
fn classic_roads_can_always_be_removed() {
    let mut g = Game::new(city_index("beijing"), 1);
    let (a, b) = two_cells(&g);
    assert_eq!(g.add_road(a, b), OK);
    run(&mut g, 60.0 * 0.0 + 12.0);
    assert_eq!(g.remove_road(a, b), OK);
}

#[test]
fn expert_roads_set_after_ten_seconds() {
    let mut g = Game::with_mode(city_index("beijing"), 1, MODE_EXPERT);
    let (a, b) = two_cells(&g);
    assert_eq!(g.add_road(a, b), OK);
    run(&mut g, 5.0);
    assert!(g.permanence_at(a) > 0.0 && g.permanence_at(a) < 0.25);
    // inside the first quarter of the 40 second timer the road can still be taken back
    let (c, d) = (g.step_cell(b, 2).unwrap(), g.step_cell(b, 2).and_then(|x| g.step_cell(x, 2)).unwrap());
    assert_eq!(g.add_road(c, d), OK);
    assert_eq!(g.remove_road(c, d), OK);
    run(&mut g, 7.0);
    assert_eq!(g.remove_road(a, b), E_PERMANENT, "more than 25% of the timer has passed");
    assert_eq!(g.erase_cell(a), E_NOTHING, "erasing the cell removes nothing either");
    assert!(g.has_edge(a, b));
    run(&mut g, 40.0);
    assert!(g.permanence_at(a) >= 1.0);
}

#[test]
fn expert_snapshot_lists_drying_roads() {
    let mut g = Game::with_mode(city_index("beijing"), 1, MODE_EXPERT);
    let (a, b) = two_cells(&g);
    g.add_road(a, b);
    assert!(g.snapshot().contains("\"dry\":[["));
    run(&mut g, 45.0);
    assert!(g.snapshot().contains("\"dry\":[]"));
}

#[test]
fn creative_has_unlimited_upgrades_no_offers_and_no_game_over() {
    let mut g = Game::with_mode(city_index("reykjavik"), 1, MODE_CREATIVE);
    assert!(g.owned.iter().all(|&n| n >= 1000));
    run(&mut g, 400.0);
    assert!(!g.over, "creative never ends");
    assert!(!g.offer_open, "no weekly offers");
    assert!(!g.dests.is_empty() && !g.houses.is_empty());
    let (x, y) = land_run(&g, 3).unwrap_or((2, 2));
    let _ = (x, y);
}

#[test]
fn modes_have_independent_state() {
    let a = Game::with_mode(city_index("london"), 5, MODE_CLASSIC);
    let b = Game::with_mode(city_index("london"), 5, MODE_EXPERT);
    assert_eq!(a.statics().replace("\"mode\":0", ""), b.statics().replace("\"mode\":1", ""));
    assert_eq!(a.mode, 0);
    assert_eq!(b.mode, 1);
    assert_eq!(a.owned[K_CONCRETE as usize], b.owned[K_CONCRETE as usize]);
}
