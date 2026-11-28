//! Endless mode, challenge modifiers, the growing map and neighbourhoods.
mod common;
use common::*;
use little_lanes_core::cities::*;
use little_lanes_core::game::*;
use little_lanes_core::roads::*;

#[test]
fn endless_never_ends_and_offers_nothing_on_a_timer() {
    let mut g = Game::with_mode(city_index("beijing"), 1, MODE_ENDLESS);
    run(&mut g, 400.0);
    assert!(!g.over);
    assert!(!g.offer_open, "endless gives upgrades at milestones, not every week");
}

#[test]
fn endless_milestones_follow_the_city_table_then_grow_by_eighty() {
    let mut g = Game::with_mode(city_index("beijing"), 1, MODE_ENDLESS);
    assert_eq!(g.milestone_target(), 60.0);
    g.stats.milestones = 1;
    assert_eq!(g.milestone_target(), 100.0);
    let n = g.city.milestones.len() as u32;
    g.stats.milestones = n;
    assert_eq!(g.milestone_target(), *g.city.milestones.last().unwrap() as f32 + 80.0);
}

#[test]
fn a_long_trip_earns_efficiency_and_a_milestone_gives_an_upgrade() {
    let mut g = Game::with_mode(city_index("beijing"), 1, MODE_ENDLESS);
    g.add_efficiency(100.0); // 100 cells is worth 75
    assert!((g.eff - 75.0).abs() < 1e-3 || g.stats.milestones == 1);
    assert_eq!(g.stats.milestones, 1, "75 passes the first milestone of 60");
    assert!(g.offer_open);
    assert_eq!(g.eff, 0.0);
    let mut c = Game::new(city_index("beijing"), 1);
    c.add_efficiency(100.0);
    assert_eq!(c.eff, 0.0, "classic ignores efficiency");
}

#[test]
fn efficiency_wears_off_with_time() {
    let mut g = Game::with_mode(city_index("beijing"), 1, MODE_ENDLESS);
    g.eff = 30.0;
    g.tick_efficiency(1.0);
    assert!(g.eff < 30.0 && g.eff > 26.0);
}

#[test]
fn endless_demand_is_half_as_fast() {
    let mut a = Game::new(city_index("london"), 3);
    let mut b = Game::with_mode(city_index("london"), 3, MODE_ENDLESS);
    run(&mut a, 1.0);
    run(&mut b, 1.0);
    assert!((b.pin_interval(0) / a.pin_interval(0) - 2.0).abs() < 0.05);
}

#[test]
fn the_map_starts_small_and_grows() {
    let mut g = Game::new(city_index("beijing"), 1);
    let (x0, y0, x1, y1) = g.play_region();
    assert!((y1 - y0) as f32 <= g.city.cam_start + 2.0, "starts at the camera start size");
    assert!(x0 > 0 && y0 > 0);
    g.time = 69.0 * 20.0;
    let (_, ay0, _, ay1) = g.play_region();
    assert_eq!((ay1 - ay0) as f32, g.city.cam_end.min(g.h as f32));
    assert!(ay1 - ay0 > y1 - y0);
}

#[test]
fn early_buildings_stay_inside_the_small_map() {
    for seed in 0..8 {
        let mut g = Game::new(city_index("london"), seed);
        run(&mut g, 40.0);
        let (x0, y0, x1, y1) = g.play_region();
        for d in 0..g.dests.len() {
            for c in g.dest_cells(d) {
                let (x, y) = g.xy(c);
                assert!(x >= x0 && x < x1 + 1 && y >= y0 && y < y1 + 1, "dest at {x},{y} outside {x0},{y0}-{x1},{y1}");
            }
        }
        for h in &g.houses {
            let (x, y) = g.xy(h.cell);
            assert!(x >= x0 && x < x1 && y >= y0 && y < y1, "house outside the region");
        }
    }
}

#[test]
fn houses_of_one_colour_gather_in_neighbourhoods() {
    let mut near = 0;
    let mut total = 0;
    for seed in 0..10 {
        let mut g = Game::new(city_index("tokyo"), seed);
        run(&mut g, 200.0);
        for h in &g.houses {
            let (hx, hy) = g.xy(h.cell);
            let mates: Vec<_> = g.houses.iter().filter(|o| o.group == h.group && o.cell != h.cell).collect();
            if mates.is_empty() {
                continue;
            }
            total += 1;
            let closest = mates.iter().map(|o| { let (ox, oy) = g.xy(o.cell); (hx - ox).abs().max((hy - oy).abs()) }).min().unwrap();
            if closest <= 2 {
                near += 1;
            }
        }
    }
    assert!(total > 20);
    assert!(near as f32 / total as f32 > 0.4, "{near} of {total} homes have a neighbour within two tiles");
}

fn with(mods: Mods) -> Game {
    Game::with_mods(city_index("beijing"), 1, MODE_CLASSIC, mods)
}

#[test]
fn modifiers_change_costs() {
    let mut m = Mods::default();
    m.straight_cost = 2;
    m.diagonal_cost = 0;
    let mut g = with(m);
    let (x, y) = land_run(&g, 4).unwrap();
    assert_eq!(g.add_road(g.cell(x, y), g.cell(x + 1, y)), OK);
    assert_eq!(g.used(K_CONCRETE), 2, "straight roads cost double");
    assert_eq!(g.add_road(g.cell(x + 1, y), g.cell(x + 2, y + 1)), OK);
    assert_eq!(g.used(K_CONCRETE), 2, "diagonals are free");
    g.remove_road(g.cell(x, y), g.cell(x + 1, y));
    assert_eq!(g.used(K_CONCRETE), 0, "the same amount comes back");
}

#[test]
fn modifiers_change_what_you_start_with() {
    let mut m = Mods::default();
    m.start.push((K_BRIDGE, 0));
    m.start.push((K_MOTORWAY, 3));
    m.unlimited[K_LIGHT as usize] = true;
    let g = with(m);
    assert_eq!(g.owned[K_BRIDGE as usize], 0);
    assert_eq!(g.owned[K_MOTORWAY as usize], 3);
    assert!(g.owned[K_LIGHT as usize] >= 9999);
}

#[test]
fn modifiers_shape_the_weekly_offer() {
    let mut m = Mods::default();
    m.prevent[K_BRIDGE as usize] = true;
    m.choices = Some(1);
    m.award_mult[0] = 2.0;
    for seed in 0..20 {
        let mut g = Game::with_mods(city_index("beijing"), seed, MODE_CLASSIC, m.clone());
        for _ in 0..4 {
            g.make_offer();
            assert_eq!(g.offers.len(), 1);
            assert!(g.offers.iter().all(|p| p.kind != K_BRIDGE));
            if g.offers[0].kind == 0 {
                assert_eq!(g.offers[0].amount, 60, "concrete awards doubled");
            }
            g.choose_upgrade(0);
        }
    }
    let mut none = Mods::default();
    none.choices = Some(0);
    let mut g = Game::with_mods(city_index("beijing"), 1, MODE_CLASSIC, none);
    g.make_offer();
    assert!(!g.offer_open, "zero choices means no offer at all");
}

#[test]
fn modifiers_can_upgrade_every_destination_and_demand_changes() {
    let mut m = Mods::default();
    m.all_upgraded = true;
    m.demand[0] = 1.5;
    let mut g = with(m);
    run(&mut g, 2.0);
    assert_eq!(g.dests[0].level, 1);
    let base = Game::with_mods(city_index("beijing"), 1, MODE_CLASSIC, { let mut b = Mods::default(); b.all_upgraded = true; b });
    let mut base = base;
    run(&mut base, 2.0);
    assert!(g.pin_interval(0) < base.pin_interval(0));
}

#[test]
fn solid_trees_block_roads() {
    let mut m = Mods::default();
    m.trees_solid = true;
    let mut g = with(m);
    let tree = (0..g.terrain.tree.len()).find(|&i| g.terrain.tree[i] && g.step_cell(i as u32, 2).map_or(false, |n| !g.terrain.tree[n as usize])).unwrap();
    let n = g.step_cell(tree as u32, 2).unwrap();
    assert_eq!(g.add_road(tree as u32, n), E_BLOCKED);
}

#[test]
fn stats_count_what_you_did() {
    let mut g = Game::new(city_index("lisbon"), 2);
    let (x, y) = land_run(&g, 3).unwrap();
    let centre = g.cell(x + 1, y);
    for d in [0usize, 2, 6] {
        let m = g.step_cell(centre, d).unwrap();
        g.add_road(centre, m);
    }
    g.toggle_light(centre);
    assert_eq!(g.stats.placed[K_LIGHT as usize], 1);
    g.toggle_light(centre);
    assert_eq!(g.stats.deleted, 1);
    assert!(g.snapshot().contains("\"stats\":["));
}
