//! Little Lanes simulation core. Pure Rust, no dependencies, deterministic.
//! The browser talks to it through the C-style functions in `ffi`.
pub mod cars;
pub mod cities;
mod cities_data;
pub mod consts;
pub mod dispatch;
pub mod ffi;
pub mod game;
pub mod json;
pub mod rng;
pub mod roads;
pub mod snapshot;
pub mod spawn;
pub mod terrain;
pub mod upgrades;

pub use game::Game;
