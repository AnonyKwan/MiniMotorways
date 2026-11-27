//! Browser services through a tiny JS plugin (see web/index.html): saved best scores and sound.
//! On desktop these fall back to in-memory storage and silence.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn ll_store_get(key: i32) -> i32;
    fn ll_store_set(key: i32, value: i32);
    fn ll_beep(kind: i32);
}

thread_local! {
    static MEM: RefCell<HashMap<i32, i32>> = RefCell::new(HashMap::new());
    static MUTED: Cell<bool> = const { Cell::new(false) };
}

/// Storage keys: 0..=2 best score per map, 10 = muted flag.
pub fn load(key: i32) -> i32 {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { ll_store_get(key) }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        MEM.with(|m| *m.borrow().get(&key).unwrap_or(&0))
    }
}

pub fn save(key: i32, value: i32) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        ll_store_set(key, value)
    }
    #[cfg(not(target_arch = "wasm32"))]
    MEM.with(|m| {
        m.borrow_mut().insert(key, value);
    });
}

pub fn best(map: usize) -> u32 {
    load(map as i32).max(0) as u32
}

pub fn record(map: usize, score: u32) -> bool {
    if score > best(map) {
        save(map as i32, score as i32);
        true
    } else {
        false
    }
}

#[derive(Clone, Copy)]
pub enum Sfx {
    Trip = 0,
    Road = 1,
    Pick = 2,
    Warning = 3,
    Pop = 4,
    GameOver = 5,
    Click = 6,
}

pub fn muted() -> bool {
    MUTED.with(|m| m.get())
}

pub fn init() {
    MUTED.with(|m| m.set(load(10) == 1));
}

pub fn set_muted(v: bool) {
    MUTED.with(|m| m.set(v));
    save(10, v as i32);
}

pub fn play(s: Sfx) {
    if muted() {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    unsafe {
        ll_beep(s as i32)
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = s;
}
