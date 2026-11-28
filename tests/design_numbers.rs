//! The numbers in the design documents, checked against the code.
mod common;
use common::*;
use little_lanes_core::cities::*;
use little_lanes_core::consts::*;
use little_lanes_core::game::*;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn clock_matches_design() {
    assert!(near(SEC_PER_HOUR * 24.0, SEC_PER_DAY));
    assert!(near(SEC_PER_DAY * 7.0, SEC_PER_WEEK));
    assert_eq!(SEC_PER_DAY, 20.0);
    assert_eq!(SEC_PER_WEEK, 140.0);
}

#[test]
fn house_supply_curve_matches_design() {
    assert!(near(curve(HOUSE_DISTANCE_CURVE, 0.0), 1.0));
    assert!(near(curve(HOUSE_DISTANCE_CURVE, 7.0), 0.92));
    assert!(near(curve(HOUSE_DISTANCE_CURVE, 10.0), 0.87));
    assert!(near(curve(HOUSE_DISTANCE_CURVE, 16.0), 0.60));
    assert!(near(curve(HOUSE_DISTANCE_CURVE, 26.0), 0.45));
    assert!(near(curve(HOUSE_DISTANCE_CURVE, 100.0), 0.45));
    assert!(near(curve(HOUSE_DISTANCE_CURVE, 8.5), 0.895)); // halfway between 7 and 10
}

#[test]
fn group_destination_count_curve_matches_design() {
    for (n, want) in [(1.0, 1.0), (2.0, 0.5), (3.0, 0.375), (5.0, 0.325), (6.0, 0.25), (8.0, 0.2)] {
        assert!(near(curve(GROUP_DESTINATION_COUNT_CURVE, n), want), "count {n}");
    }
}

#[test]
fn overcrowd_speed_curve_matches_design() {
    assert!(near(curve(OVERCROWD_SPEED_BY_EXTRA_DEMAND, 0.0), 1.0));
    assert!(near(curve(OVERCROWD_SPEED_BY_EXTRA_DEMAND, 3.0), 1.0));
    assert!(near(curve(OVERCROWD_SPEED_BY_EXTRA_DEMAND, 15.0), 4.0));
    assert!(near(curve(OVERCROWD_SPEED_BY_EXTRA_DEMAND, 9.0), 2.5));
}

#[test]
fn pin_limits_match_design() {
    assert_eq!(Game::pin_threshold(0), 6);
    assert_eq!(Game::pin_threshold(1), 9);
    assert_eq!(Game::pin_hard_cap(0), 10);
    assert_eq!(Game::pin_hard_cap(1), 15);
}

#[test]
fn normal_destination_makes_a_pin_about_every_16_seconds() {
    let mut g = Game::new(city_index("beijing"), 1);
    run(&mut g, 0.5);
    assert!(!g.dests.is_empty(), "the first destination appears on day 0");
    // base demand 0.8 is the lowest the oscillation gets, so the interval is at most 20/(1.55*0.8*0.8)
    let slowest = SEC_PER_DAY / (AVERAGE_CARS_PER_DAY * DEMAND_MULT * g.city.osc[g.dests[0].group as usize].base);
    let i = g.pin_interval(0);
    assert!(i <= slowest + 1e-3 && i > 8.0, "interval {i}");
    // an upgraded building asks twice as often
    let normal = g.pin_interval(0);
    g.dests[0].level = 1;
    assert!(near(g.pin_interval(0) * 2.0, normal));
}

#[test]
fn first_pin_arrives_after_four_seconds() {
    let mut g = Game::new(city_index("london"), 3);
    run(&mut g, 0.5);
    assert_eq!(g.dests[0].pins, 0);
    run(&mut g, 4.0);
    assert!(g.dests[0].pins >= 1);
}

#[test]
fn overcrowding_ends_the_game_after_about_fifty_seconds() {
    let mut g = Game::new(city_index("tokyo"), 5);
    run(&mut g, 0.5);
    g.dests[0].pins = 7; // over the limit of 6, and nobody comes
    run(&mut g, 40.0);
    assert!(!g.over, "still alive after 40s");
    assert!(g.dests[0].ot > 30.0);
    run(&mut g, 20.0);
    assert!(g.over, "overcrowd timer reached 50");
    assert_eq!(g.over_dest, 0);
}

#[test]
fn timer_falls_back_to_the_grace_mark_when_pins_clear() {
    let mut g = Game::new(city_index("tokyo"), 5);
    run(&mut g, 0.5);
    g.dests[0].ot = 49.5;
    g.dests[0].pins = 2;
    g.advance(STEP);
    assert!(g.dests[0].ot <= MAX_OVERCROWD_TIME - GRACE_TIME);
    run(&mut g, 10.0);
    assert!(g.dests[0].ot < 30.0);
}

#[test]
fn arriving_car_scores_and_shortens_the_timer() {
    let mut g = Game::new(city_index("tokyo"), 5);
    run(&mut g, 0.5);
    g.dests[0].pins = 3;
    g.dests[0].assigned = 1;
    g.dests[0].ot = 20.0;
    g.collect_pin(0);
    assert_eq!(g.score, 1);
    assert_eq!(g.dests[0].pins, 2);
    assert!(near(g.dests[0].ot, 18.0)); // 10% of 20
    g.dests[0].ot = 100.0;
    g.collect_pin(0);
    assert!(near(g.dests[0].ot, 97.0)); // capped at 3 seconds
}

#[test]
fn city_catalog_matches_design_tables() {
    assert_eq!(CITIES.len(), 28, "27 maps plus the hidden one");
    let london = &CITIES[city_index("london")];
    assert_eq!(london.start, &[(0, 30), (1, 2)]);
    let la = &CITIES[city_index("losangeles")];
    assert_eq!(la.start, &[(0, 25), (1, 1)]);
    assert_eq!(CITIES[city_index("mexicocity")].groups, 6);
    assert_eq!(CITIES[city_index("tokyo")].groups, 6);
    assert_eq!(CITIES[city_index("beijing")].groups, 5);
    assert_eq!(CITIES[city_index("hongkong")].start, &[(0, 40), (2, 1), (1, 1)]);
    assert_eq!(CITIES[city_index("reykjavik")].start, &[(0, 30), (3, 2)]);
    assert_eq!(CITIES[city_index("daressalaam")].start, &[(0, 25), (4, 1)]);
    assert_eq!(CITIES[city_index("wellington")].start, &[(0, 40), (5, 1)]);
}

#[test]
fn every_city_has_a_plan_and_an_upgrade_pool() {
    for c in CITIES {
        assert!(!c.plan.is_empty(), "{} has planned buildings", c.id);
        assert!(!c.rules.is_empty(), "{} has upgrade rules", c.id);
        assert_eq!(c.osc.len(), c.groups as usize, "{} oscillation per group", c.id);
        assert!(c.plan.iter().all(|p| p.1 < c.groups || p.1 == 255), "{} plan groups in range", c.id);
        assert!(c.plan.windows(2).all(|w| w[0].0 <= w[1].0), "{} plan sorted", c.id);
    }
}
