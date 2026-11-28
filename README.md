# Little Lanes

A small road-drawing traffic puzzle for the browser. Join every home to a big building of its own colour and shape, keep the pins from piling up, and choose an upgrade each week. Play it: https://anonykwan.github.io/LittleLanes/

The simulation is Rust compiled to WebAssembly (`src/`). The page (`web/`) draws it in 3D with WebGL2, with a flat 2D fallback. All artwork is drawn in code.

Game rules and numbers follow the design documents (140-second weeks, demand and supply curves, pin limits 6/9/7, overcrowd timer 50 s, weighted weekly packs per city). `tools/gen_cities.py` and `tools/gen_achievements.py` turn the design tables into `src/cities_data.rs` and `web/js/achievements_data.js`.

## Modes
Classic, Expert (roads set after 40 s), Creative, Endless (also after a loss), a daily map, a weekly map and city challenges.

## Tests and publishing (all in GitHub Actions)
| Job | What it runs |
| --- | --- |
| `rust` | `cargo test` (rules, roads, upgrades, modes, trains and ferries), then builds the `.wasm` |
| `web` | JavaScript tests against the real `.wasm` |
| `e2e` | Playwright in Chromium: menu, playing, drawing, 3D view, phone size |
| `status` | writes `PASS <commit>` when all of the above are green |
| `deploy` | publishes to GitHub Pages, only on `main` and only after everything passed |

Logs and screenshots of each run are published to the `ci-logs` branch.

### Working without breaking `main`
1. Work on a branch. Run `bash tools/preflight.sh` (instant checks, needs only Node).
2. Push the branch and wait for the run to finish.
3. Publish with `bash tools/ship.sh`. It refuses unless CI reported green for exactly that commit.

## Controls
Draw: drag. Erase: right-drag, Ctrl or Tab. Undo erase: Z. Pause: Space. Speed: 1 2 3. Zoom: wheel or pinch. Pan: middle-drag or two fingers.
