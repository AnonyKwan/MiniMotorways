use little_lanes::map::MapKind;
use little_lanes::sim::{Sim, DT, WEEK_LENGTH};

/// Sixteen weeks on every map with demand cleared each step: spawning, schedules and
/// rush waves keep working and never panic.
#[test]
fn sixteen_weeks_on_every_map() {
    for k in MapKind::ALL {
        for seed in [1, 2, 3] {
            let mut s = Sim::new_on(k, seed);
            for _ in 0..(16.0 * WEEK_LENGTH / DT) as usize {
                for d in &mut s.dests {
                    d.pins = 0;
                    d.assigned = 0;
                }
                if s.offer.is_some() {
                    s.choose(seed as usize % 2);
                }
                s.step();
            }
            assert!(s.week >= 16);
            assert!(s.dests.len() >= 8, "{:?}/{}: only {} shops", k, seed, s.dests.len());
            assert!(s.homes.len() <= 5 * s.dests.len() + 2);
            eprintln!("{:?} seed {}: {} shops, {} homes", k, seed, s.dests.len(), s.homes.len());
        }
    }
}
