//! City definitions. The numbers live in `cities_data.rs`, generated from the design tables.
pub use crate::cities_data::CITIES;

pub const K_CONCRETE: u8 = 0;
pub const K_BRIDGE: u8 = 1;
pub const K_MOTORWAY: u8 = 2;
pub const K_LIGHT: u8 = 3;
pub const K_ROUND: u8 = 4;
pub const K_TUNNEL: u8 = 5;
pub const KINDS: usize = 6;

/// One weekly upgrade package the city can offer.
pub struct UpgradeRule {
    pub kind: u8,
    pub amount: u8,
    pub add_concrete: u16,
    pub base: f32,
    pub inc: f32,
    pub max_count: u8,
    pub start_week: u8,
    pub last_week: u8,
    /// Weight multiplier as a function of concrete still available (piecewise linear).
    pub rel: &'static [(f32, f32)],
}

/// Demand oscillation of one colour group.
pub struct OscDef {
    pub base: f32,
    pub burst: f32,
    pub ecc: f32,
    pub period: f32,
}

pub struct City {
    pub id: &'static str,
    pub name: &'static str,
    pub w: i32,
    pub h: i32,
    pub groups: u8,
    pub water: u32,
    pub mountain: u32,
    pub trees: u32,
    pub start: &'static [(u8, u8)],
    /// Planned destinations: (day, colour group), sorted by day.
    pub plan: &'static [(f32, u8)],
    pub rules: &'static [UpgradeRule],
    pub osc: &'static [OscDef],
}

pub fn city_count() -> usize {
    CITIES.len()
}
