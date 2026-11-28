//! C-style interface used by the browser through WebAssembly.
//!
//! The browser calls these functions; strings come back through a shared output buffer:
//! call a function that returns a length, then read that many bytes at `ll_out_ptr()`.
#![allow(static_mut_refs)]
use crate::cities::CITIES;
use crate::game::{Game, Mods};
use crate::roads::{E_BOUNDS, E_NOTHING};
use crate::snapshot::catalog;

static mut GAME: Option<Game> = None;
static mut MODS: Option<Mods> = None;
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

/// Clears the pending challenge modifiers.
#[no_mangle]
pub extern "C" fn ll_mod_reset() {
    unsafe {
        MODS = Some(Mods::default());
    }
}

/// Sets one challenge modifier for the next game. `key` follows doc 09.3; `a` is an upgrade
/// kind or group, `b` a count or a multiplier scaled by 100.
#[no_mangle]
pub extern "C" fn ll_mod(key: u32, a: i32, b: i32) {
    unsafe {
        let m = MODS.get_or_insert_with(Mods::default);
        let k = (a.max(0) as usize).min(5);
        match key {
            0 => m.start.push((k as u8, b.max(0) as u32)),
            1 => m.prevent[k] = true,
            3 => m.award_mult[k] = b as f32 / 100.0,
            4 => m.choices = Some(b.max(0) as usize),
            5 => m.free_concrete = Some(b.max(0) as u32),
            7 => m.demand[k] = b as f32 / 100.0,
            9 => m.never_upgrade = true,
            10 => m.all_upgraded = true,
            17 => m.unlimited[k] = true,
            18 => m.trees_solid = true,
            21 => {
                if a == 1 {
                    m.bridge_speed = b as f32 / 100.0;
                } else {
                    m.tunnel_speed = b as f32 / 100.0;
                }
            }
            22 => m.straight_cost = b.max(0) as u32,
            23 => m.diagonal_cost = b.max(0) as u32,
            24 => {
                if a == 1 {
                    m.bridge_cost = b.max(0) as u32;
                } else {
                    m.tunnel_cost = b.max(0) as u32;
                }
            }
            _ => {}
        }
    }
}

/// Starts a new game in a mode: 0 classic, 1 expert, 2 creative.
#[no_mangle]
pub extern "C" fn ll_new_mode(city: u32, seed: u32, mode: u32) -> u32 {
    unsafe {
        let mods = MODS.take().unwrap_or_default();
        GAME = Some(Game::with_mods(city as usize, seed, mode as u8, mods));
    }
    1
}

#[no_mangle]
pub extern "C" fn ll_step(dt: f32) {
    if let Some(g) = game() {
        g.advance(dt);
    }
}

/// Steps taken so far.
#[no_mangle]
pub extern "C" fn ll_ticks() -> u32 {
    game().map_or(0, |g| g.ticks)
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

/// Starts a new erase gesture (what it removes can be undone together).
#[no_mangle]
pub extern "C" fn ll_begin_erase() {
    if let Some(g) = game() {
        g.last_erase.clear();
    }
}

/// Puts back the roads removed by the last erase.
#[no_mangle]
pub extern "C" fn ll_undo() -> i32 {
    with_game(|g| g.undo_erase())
}

/// Carries a lost game on as Endless.
#[no_mangle]
pub extern "C" fn ll_continue_endless() -> i32 {
    with_game(|g| {
        g.continue_endless();
        0
    })
}

#[no_mangle]
pub extern "C" fn ll_choose_upgrade(i: u32) -> i32 {
    with_game(|g| g.choose_upgrade(i as usize))
}
