//! C-style interface used by the browser through WebAssembly.
//!
//! The browser calls these functions; strings come back through a shared output buffer:
//! call a function that returns a length, then read that many bytes at `ll_out_ptr()`.
#![allow(static_mut_refs)]
use crate::cities::CITIES;
use crate::game::Game;
use crate::roads::{E_BOUNDS, E_NOTHING};
use crate::snapshot::catalog;

static mut GAME: Option<Game> = None;
static mut OUT: Vec<u8> = Vec::new();

fn game() -> Option<&'static mut Game> {
    unsafe { GAME.as_mut() }
}

fn set_out(bytes: &[u8]) -> u32 {
    unsafe {
        OUT.clear();
        OUT.extend_from_slice(bytes);
        OUT.len() as u32
    }
}

#[no_mangle]
pub extern "C" fn ll_out_ptr() -> *const u8 {
    unsafe { OUT.as_ptr() }
}

#[no_mangle]
pub extern "C" fn ll_city_count() -> u32 {
    CITIES.len() as u32
}

/// Writes the city catalog as JSON to the output buffer and returns its length.
#[no_mangle]
pub extern "C" fn ll_catalog() -> u32 {
    set_out(catalog().as_bytes())
}

/// Starts a new game. Returns 1 on success.
#[no_mangle]
pub extern "C" fn ll_new(city: u32, seed: u32) -> u32 {
    unsafe {
        GAME = Some(Game::new(city as usize, seed));
    }
    1
}

/// Starts a new game in a mode: 0 classic, 1 expert, 2 creative.
#[no_mangle]
pub extern "C" fn ll_new_mode(city: u32, seed: u32, mode: u32) -> u32 {
    unsafe {
        GAME = Some(Game::with_mode(city as usize, seed, mode as u8));
    }
    1
}

#[no_mangle]
pub extern "C" fn ll_step(dt: f32) {
    if let Some(g) = game() {
        g.advance(dt);
    }
}

#[no_mangle]
pub extern "C" fn ll_snapshot() -> u32 {
    match game() {
        Some(g) => set_out(g.snapshot().as_bytes()),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn ll_statics() -> u32 {
    match game() {
        Some(g) => set_out(g.statics().as_bytes()),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn ll_terrain() -> u32 {
    match game() {
        Some(g) => set_out(&g.terrain_bytes()),
        None => 0,
    }
}

fn with_game(f: impl FnOnce(&mut Game) -> i32) -> i32 {
    match game() {
        Some(g) => f(g),
        None => E_NOTHING,
    }
}

#[no_mangle]
pub extern "C" fn ll_add_road(a: u32, b: u32) -> i32 {
    with_game(|g| if a as usize >= g.conn.len() || b as usize >= g.conn.len() { E_BOUNDS } else { g.add_road(a, b) })
}

#[no_mangle]
pub extern "C" fn ll_remove_road(a: u32, b: u32) -> i32 {
    with_game(|g| if a as usize >= g.conn.len() || b as usize >= g.conn.len() { E_BOUNDS } else { g.remove_road(a, b) })
}

#[no_mangle]
pub extern "C" fn ll_erase_cell(c: u32) -> i32 {
    with_game(|g| g.erase_cell(c))
}

#[no_mangle]
pub extern "C" fn ll_add_motorway(a: u32, b: u32) -> i32 {
    with_game(|g| g.add_motorway(a, b))
}

#[no_mangle]
pub extern "C" fn ll_remove_motorway(i: u32) -> i32 {
    with_game(|g| g.remove_motorway(i as usize))
}

#[no_mangle]
pub extern "C" fn ll_toggle_light(c: u32) -> i32 {
    with_game(|g| g.toggle_light(c))
}

#[no_mangle]
pub extern "C" fn ll_toggle_roundabout(c: u32) -> i32 {
    with_game(|g| g.toggle_roundabout(c))
}

#[no_mangle]
pub extern "C" fn ll_choose_upgrade(i: u32) -> i32 {
    with_game(|g| g.choose_upgrade(i as usize))
}
