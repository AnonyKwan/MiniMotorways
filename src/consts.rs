//! Numbers from the design documents (docs 01, 04, 05, 08), in one place.

// ---- time (doc 01.3) ----
pub const SEC_PER_HOUR: f32 = 5.0 / 6.0;
pub const SEC_PER_DAY: f32 = 20.0;
pub const SEC_PER_WEEK: f32 = 140.0;
/// Fixed simulation step.
pub const STEP: f32 = 1.0 / 30.0;

// ---- vehicles (doc 05.4, 05.5) ----
/// Design speeds are unit-less; this converts them to grid cells per second. (Assumption.)
pub const SPEED_UNIT: f32 = 2.0;
pub const LANE_SPEED: f32 = 1.0;
pub const MOTORWAY_SPEED: f32 = 3.0;
pub const ROUNDABOUT_SPEED: f32 = 1.2;
pub const INTERSECTION_SPEED: f32 = 0.55;
pub const SHARP_TURN: f32 = 0.333_333;
pub const RIGHT_TURN: f32 = 0.666_667;
pub const MAX_ACCEL: f32 = 0.54;
pub const MAX_DECEL: f32 = 12.0;
pub const MAX_WAIT_AT_INTERSECTION: f32 = 45.0;
pub const CARS_PER_HOUSE: usize = 2;
pub const TIME_AT_HOUSE: f32 = 10.0;
/// Distance before a junction node at which a car must hold a reservation.
pub const APPROACH: f32 = 0.3;
pub const MIN_GAP: f32 = 0.45;
pub const PARK_TIME: f32 = 1.2;
pub const DISPATCH_EVERY: f32 = 0.5;

// ---- traffic lights (doc 05.6) ----
pub const LIGHT_GREEN: f32 = 3.0;
pub const LIGHT_AMBER: f32 = 0.7;

// ---- demand and pins (doc 05.9, 04.7) ----
pub const AVERAGE_CARS_PER_DAY: f32 = 1.55;
pub const DEMAND_MULT: f32 = 0.8;
pub const DEMAND_MULT_UPGRADED: f32 = 1.6;
pub const FIRST_PIN_DELAY: f32 = 4.0;
pub const MAX_PINS: u32 = 6;
pub const MAX_PINS_UPGRADED: u32 = 9;
pub const OVERFLOW_PINS: u32 = 4;
pub const OVERFLOW_PINS_UPGRADED: u32 = 6;

// ---- overcrowding (doc 05.8, 04.11) ----
pub const MAX_OVERCROWD_TIME: f32 = 50.0;
pub const GRACE_TIME: f32 = 2.0;
pub const OVERCROWD_ACCEL: f32 = 1.0;
pub const OVERCROWD_RETURN_SPEED: f32 = 2.0;
pub const PCT_REDUCE_ON_ARRIVAL: f32 = 0.10;
pub const MAX_REDUCE_ON_ARRIVAL: f32 = 3.0;

// ---- building spawning (doc 05.7) ----
pub const HOUSE_CONTRIBUTION: f32 = 0.875;
pub const SAME_GROUP_HOUSE_DELAY: f32 = 10.0;
pub const FAILED_HOUSE_COOLDOWN: f32 = 2.0;
pub const FAILED_DEST_RETRY: f32 = 20.0;
pub const MAX_FAILED_DEST_BEFORE_UPGRADE: u32 = 15;
pub const MAX_HOUSES_PER_GROUP: usize = 16;

// ---- roads and upgrades (doc 04.2, 04.3) ----
pub const MIN_MOTORWAY_LENGTH: i32 = 2;
pub const MAX_UPGRADE_CHOICES: usize = 2;

// ---- curves (doc 05.12) ----
pub const HOUSE_DISTANCE_CURVE: &[(f32, f32)] = &[
    (0.0, 1.0),
    (7.0, 0.92),
    (10.0, 0.87),
    (12.0, 0.78),
    (14.0, 0.68),
    (16.0, 0.6),
    (21.0, 0.5),
    (26.0, 0.45),
    (28.0, 0.45),
];
pub const GROUP_DESTINATION_COUNT_CURVE: &[(f32, f32)] = &[
    (0.0, 1.0),
    (1.0, 1.0),
    (2.0, 0.5),
    (3.0, 0.375),
    (5.0, 0.325),
    (6.0, 0.25),
    (8.0, 0.2),
    (10.0, 0.2),
];
pub const OVERCROWD_SPEED_BY_EXTRA_DEMAND: &[(f32, f32)] = &[(0.0, 1.0), (3.0, 1.0), (15.0, 4.0)];

/// Piecewise-linear lookup; clamps outside the range.
pub fn curve(points: &[(f32, f32)], x: f32) -> f32 {
    if points.is_empty() {
        return 1.0;
    }
    if x <= points[0].0 {
        return points[0].1;
    }
    for w in points.windows(2) {
        let (x0, y0) = w[0];
        let (x1, y1) = w[1];
        if x <= x1 {
            if x1 <= x0 {
                return y1;
            }
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        }
    }
    points[points.len() - 1].1
}
