# Little Lanes

A minimalist road-drawing traffic puzzle that runs in the browser, written in Rust and compiled to WebAssembly with [macroquad](https://github.com/not-fl3/macroquad).

Homes and destinations appear on a small map. Draw roads so cars from each home can reach destinations of the same colour. Every week brings more road tiles and a choice of one upgrade. If a destination waits too long, the game ends. Your best score on each map is saved in the browser.

## Maps

- **Bayside**: a calm town by a winding river.
- **Twin Islands**: two shores split by a channel. Bridges matter.
- **Highland**: rocky hills everywhere. Tunnels matter.

## Upgrades

At the end of each week the game pauses and offers two of these:

- **Bridge**: build roads across 4 water tiles.
- **Tunnel**: build roads through 4 hill tiles.
- **Roundabout**: a junction where cars never wait for each other. A plain crossing lets one car through at a time.
- **Traffic light**: lets two straight-through directions go at once, switching every few seconds.
- **Highway**: a fast elevated link between two points up to 14 tiles apart, over anything.
- **Road tiles**: 20 more road tiles.

Shops have a car park along their front. A big shop has two driveways, and cars may drive through its car park from one to the other as a short cut.

## Play

The `main` branch is published to GitHub Pages by `.github/workflows/pages.yml`. To play locally:

```sh
./build_web.sh
python3 -m http.server -d dist 8000
# open http://localhost:8000
```

You can also run it as a desktop window with `cargo run --release`.

## Controls

| Action | Mouse / keys | Touch |
| --- | --- | --- |
| Draw road | Left-drag (tool **R**) | Drag |
| Erase road | Right-drag, or tool **E** | Erase tool, then drag |
| Roundabout / traffic light | Tool **O** / **L**, click a road tile | Tap the tool, tap a road |
| Highway | Tool **H**, click the start, click the end. Right click or Esc cancels | Tap the tool, tap both ends |
| Zoom | Mouse wheel, or + and - | Pinch |
| Pan | WASD / arrow keys, or middle-drag | Two-finger drag |
| Pause / speed | Space, 1, 2 or the top-right buttons | Top-right buttons |
| Pick an upgrade | Click a card, or 1 / 2 | Tap a card |
| Mute | M or the speaker button | Speaker button |
| Back to maps | Esc or **Menu** | Menu button |

## Code layout

- `src/sim.rs` — game rules: roads, homes, destinations, cars, pathfinding, spawning, overflow. No rendering, fully unit tested (`cargo test --lib`).
- `src/map.rs` — grid size and terrain generation.
- `src/render3d.rs` — the 3D camera, mesh building and all world artwork.
- `src/main.rs` — input, the title screen, HUD, tool bar, upgrade cards and the game loop.
- `src/platform.rs` — saved best scores and sound effects through a small JavaScript plugin in `web/index.html`.
- `web/` — the HTML page and miniquad's `gl.js` loader.

## Look

The world is rendered in 3D with a gently tilted camera; on tall phone screens the board turns so its long side runs up the screen. Static scenery is built once and reused, and cars are blended between simulation steps, so play stays smooth. Countryside, rivers and groves carry on past the edge of the play area, so the game fills the whole screen. Every model (terrain, houses, shops, trees, cars) is a flat-shaded low-poly mesh generated in code, so the game ships no model or texture files. Text uses the bundled Inter font (SIL Open Font License, see `assets/Inter-LICENSE.txt`).
