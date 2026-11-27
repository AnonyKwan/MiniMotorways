# Little Lanes

A minimalist road-drawing traffic puzzle that runs in the browser, written in Rust and compiled to WebAssembly with [macroquad](https://github.com/not-fl3/macroquad).

Homes and destinations appear on a small map. Draw roads so cars from each home can reach destinations of the same colour. Every week brings more road tiles. If a destination waits too long, the game ends.

## Play locally

```sh
./build_web.sh
python3 -m http.server -d dist 8000
# open http://localhost:8000
```

You can also run it as a desktop window with `cargo run --release`.

## Controls

| Action | Mouse / keys | Touch |
| --- | --- | --- |
| Draw road | Left-drag | Drag |
| Erase road | Right-drag, or toggle **Erase** (E) | Toggle **Erase**, then drag |
| Pause / speed | Space, 1, 2 or the top-right buttons | Top-right buttons |
| Restart after game over | R or **Play again** | **Play again** |

## Code layout

- `src/sim.rs` — game rules: roads, homes, destinations, cars, pathfinding, spawning, overflow. No rendering, fully unit tested (`cargo test --lib`).
- `src/map.rs` — grid size and terrain generation.
- `src/main.rs` — drawing, input and the HUD.
- `web/` — the HTML page and miniquad's `gl.js` loader.
