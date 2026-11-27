use little_lanes::map::Pos;
use little_lanes::sim::{Cell, EditResult, Sim, DT};
use macroquad::prelude::*;

mod render3d;

use std::cell::RefCell;

const HUD_H: f32 = 48.0;

const INK: Color = Color::new(0.20, 0.20, 0.24, 1.0);
const WARN: Color = Color::new(0.85, 0.25, 0.22, 1.0);

const PALETTE: [Color; 6] = [
    Color::new(0.937, 0.490, 0.435, 1.0), // coral
    Color::new(0.310, 0.702, 0.663, 1.0), // teal
    Color::new(0.914, 0.706, 0.298, 1.0), // mustard
    Color::new(0.557, 0.486, 0.765, 1.0), // violet
    Color::new(0.357, 0.608, 0.835, 1.0), // sky
    Color::new(0.910, 0.561, 0.706, 1.0), // pink
];

thread_local! {
    static FONT: RefCell<Option<Font>> = const { RefCell::new(None) };
}

/// Draw text with the bundled Inter font (falls back to the built-in font if it failed to load).
fn text(t: &str, x: f32, y: f32, size: f32, color: Color) {
    FONT.with(|f| {
        let f = f.borrow();
        draw_text_ex(t, x, y, TextParams { font: f.as_ref(), font_size: size as u16, color, ..Default::default() });
    });
}

fn measure(t: &str, size: f32) -> TextDimensions {
    FONT.with(|f| measure_text(t, f.borrow().as_ref(), size as u16, 1.0))
}

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Draw,
    Erase,
}

struct Button {
    rect: Rect,
    label: &'static str,
}

/// Tiles on a 4-connected line from a to b, so fast drags still make joined roads.
fn tiles_between(a: Pos, b: Pos) -> Vec<Pos> {
    let mut out = Vec::new();
    let (mut x, mut y) = a;
    let (dx, dy) = ((b.0 - a.0).abs(), (b.1 - a.1).abs());
    let (sx, sy) = ((b.0 - a.0).signum(), (b.1 - a.1).signum());
    let mut err = dx - dy;
    while (x, y) != b {
        if 2 * err > -dy {
            err -= dy;
            x += sx;
        } else {
            err += dx;
            y += sy;
        }
        out.push((x, y));
    }
    out
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Little Lanes".to_owned(),
        window_width: 1280,
        window_height: 816,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

fn seed() -> u64 {
    // A fixed seed can be baked in for testing: LL_SEED=42 ./build_web.sh
    if let Some(s) = option_env!("LL_SEED").and_then(|s| s.parse().ok()) {
        return s;
    }
    (macroquad::miniquad::date::now() * 1000.0) as u64
}

struct Game {
    sim: Sim,
    tool: Tool,
    speed: u32, // 0 = paused, 1, 2
    acc: f32,
    last_tile: Option<Pos>,
    drag_erase: bool,
    flash: Option<(Pos, f32)>,
}

impl Game {
    fn new() -> Self {
        Game {
            sim: Sim::new(seed()),
            tool: Tool::Draw,
            speed: 1,
            acc: 0.0,
            last_tile: None,
            drag_erase: false,
            flash: None,
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    if let Ok(f) = load_ttf_font_from_bytes(include_bytes!("../assets/Inter-SemiBold.otf")) {
        FONT.with(|cell| *cell.borrow_mut() = Some(f));
    }
    let mut g = Game::new();

    loop {
        let scene = render3d::Scene::new(HUD_H);
        // Test builds print where each tile appears on screen so automated tests can click tiles.
        if option_env!("LL_SEED").is_some() && is_key_pressed(KeyCode::F9) {
            let mut out = String::new();
            for y in 0..little_lanes::map::H {
                for x in 0..little_lanes::map::W {
                    let p = scene.to_screen(vec3(x as f32 + 0.5, 0.0, y as f32 + 0.5));
                    out.push_str(&format!("{},{},{:.1},{:.1};", x, y, p.x, p.y));
                }
            }
            macroquad::miniquad::info!("TILES {}", out);
        }
        let buttons = hud_buttons();
        let (mx, my) = mouse_position();

        // ---- Input -------------------------------------------------
        if is_key_pressed(KeyCode::Space) {
            g.speed = if g.speed == 0 { 1 } else { 0 };
        }
        if is_key_pressed(KeyCode::Key1) {
            g.speed = 1;
        }
        if is_key_pressed(KeyCode::Key2) {
            g.speed = 2;
        }
        if is_key_pressed(KeyCode::E) {
            g.tool = if g.tool == Tool::Draw { Tool::Erase } else { Tool::Draw };
        }

        let pressed_l = is_mouse_button_pressed(MouseButton::Left);
        let pressed_r = is_mouse_button_pressed(MouseButton::Right);
        let mut over_ui = my < HUD_H;

        if g.sim.game_over {
            if is_key_pressed(KeyCode::R) || (pressed_l && restart_rect().contains(vec2(mx, my))) {
                g = Game::new();
                next_frame().await;
                continue;
            }
            over_ui = true;
        }

        if pressed_l && my < HUD_H {
            for (i, b) in buttons.iter().enumerate() {
                if b.rect.contains(vec2(mx, my)) {
                    match i {
                        0 => g.speed = 0,
                        1 => g.speed = 1,
                        2 => g.speed = 2,
                        3 => g.tool = if g.tool == Tool::Draw { Tool::Erase } else { Tool::Draw },
                        _ => {}
                    }
                }
            }
        }

        let down_l = is_mouse_button_down(MouseButton::Left);
        let down_r = is_mouse_button_down(MouseButton::Right);
        if (pressed_l || pressed_r) && !over_ui {
            g.last_tile = None;
            g.drag_erase = pressed_r || g.tool == Tool::Erase;
        }
        if (down_l || down_r) && !over_ui {
            if let Some(t) = scene.pick(mx, my) {
                let tiles = match g.last_tile {
                    Some(l) if l != t => tiles_between(l, t),
                    Some(_) => vec![],
                    None => vec![t],
                };
                for p in tiles {
                    let r = if g.drag_erase { g.sim.erase_road(p) } else { g.sim.place_road(p) };
                    if r == EditResult::NoRoadsLeft {
                        g.sim.toast = Some(("Out of road tiles. More arrive each week.".into(), 2.5));
                    }
                    if matches!(r, EditResult::NoRoadsLeft | EditResult::Blocked) && !matches!(g.sim.cell(p), Cell::Dest(_) | Cell::Home(_) | Cell::Door(_)) {
                        g.flash = Some((p, 0.3));
                    }
                }
                g.last_tile = Some(t);
            }
        } else {
            g.last_tile = None;
        }

        // ---- Simulation ---------------------------------------------
        g.acc += get_frame_time().min(0.1) * g.speed as f32;
        let mut steps = 0;
        while g.acc >= DT && steps < 16 {
            g.sim.step();
            g.acc -= DT;
            steps += 1;
        }
        if let Some((_, t)) = &mut g.flash {
            *t -= get_frame_time();
            if *t <= 0.0 {
                g.flash = None;
            }
        }

        // ---- Drawing -------------------------------------------------
        clear_background(render3d::SKY);
        let hover = if !over_ui && !g.sim.game_over { scene.pick(mx, my) } else { None };
        set_camera(&scene.cam);
        render3d::draw_world(
            &g.sim,
            get_time() as f32,
            &render3d::Highlight { tile: hover, erase: g.tool == Tool::Erase || down_r, flash: g.flash },
        );
        set_default_camera();
        render3d::draw_overlays(&g.sim, &scene, get_time() as f32);
        draw_hud(&g, &buttons);
        if g.sim.game_over {
            draw_game_over(&g);
        }

        next_frame().await
    }
}

fn hud_buttons() -> Vec<Button> {
    let labels = ["||", "1x", "2x", "Erase"];
    let mut x = screen_width() - 12.0;
    let mut out: Vec<Button> = Vec::new();
    for l in labels.iter().rev() {
        let w = if *l == "Erase" { 72.0 } else { 44.0 };
        x -= w;
        out.push(Button { rect: Rect::new(x, 8.0, w, 32.0), label: l });
        x -= 8.0;
    }
    out.reverse();
    out
}

/// Filled rounded rectangle in screen pixels.
fn pill(x: f32, y: f32, w: f32, h: f32, c: Color) {
    let r = (h / 2.0).min(w / 2.0);
    draw_rectangle(x + r, y, w - 2.0 * r, h, c);
    draw_circle(x + r, y + r, r, c);
    draw_circle(x + w - r, y + r, r, c);
}

fn draw_hud(g: &Game, buttons: &[Button]) {
    let s = &g.sim;
    draw_rectangle(0.0, 0.0, screen_width(), HUD_H, Color::new(0.98, 0.97, 0.94, 1.0));
    draw_rectangle(0.0, HUD_H - 3.0, screen_width(), 3.0, Color::new(0.0, 0.0, 0.0, 0.08));

    let chip_bg = Color::new(0.0, 0.0, 0.0, 0.06);
    let mut x = 12.0;
    // Trips: a little car icon
    let label = format!("{}", s.score);
    let w = 54.0 + measure(&label, 24.0).width;
    pill(x, 8.0, w, 30.0, chip_bg);
    pill(x + 10.0, 16.0, 26.0, 14.0, PALETTE[0]);
    draw_rectangle(x + 18.0, 18.0, 10.0, 10.0, Color::new(1.0, 1.0, 1.0, 0.7));
    text(&label, x + 44.0, 31.0, 24.0, INK);
    x += w + 10.0;
    // Week with progress ring
    let label = format!("Week {}", s.week);
    let w = 50.0 + measure(&label, 24.0).width;
    pill(x, 8.0, w, 30.0, chip_bg);
    draw_circle_lines(x + 22.0, 23.0, 9.0, 3.0, Color::new(0.0, 0.0, 0.0, 0.12));
    draw_arc(x + 22.0, 23.0, 32, 7.5, -90.0, 3.0, 360.0 * s.week_progress(), INK);
    text(&label, x + 40.0, 31.0, 24.0, INK);
    x += w + 10.0;
    // Road tiles left: a road icon
    let label = format!("{}", s.roads_left);
    let w = 54.0 + measure(&label, 24.0).width;
    let low = s.roads_left < 5;
    pill(x, 8.0, w, 30.0, if low { Color::new(WARN.r, WARN.g, WARN.b, 0.18) } else { chip_bg });
    draw_rectangle(x + 10.0, 16.0, 26.0, 14.0, Color::new(0.33, 0.35, 0.4, 1.0));
    draw_rectangle(x + 13.0, 22.0, 6.0, 2.0, WHITE);
    draw_rectangle(x + 27.0, 22.0, 6.0, 2.0, WHITE);
    text(&label, x + 44.0, 31.0, 24.0, if low { WARN } else { INK });

    for (i, b) in buttons.iter().enumerate() {
        let active = match i {
            0 => g.speed == 0,
            1 => g.speed == 1,
            2 => g.speed == 2,
            _ => g.tool == Tool::Erase,
        };
        let fill = if active { if i == 3 { WARN } else { INK } } else { chip_bg };
        let tc = if active { WHITE } else { INK };
        pill(b.rect.x, b.rect.y, b.rect.w, b.rect.h, fill);
        let m = measure(b.label, 22.0);
        text(b.label, b.rect.x + (b.rect.w - m.width) / 2.0, b.rect.y + 22.0, 22.0, tc);
    }

    if let Some((msg, t)) = &s.toast {
        let a = t.min(1.0);
        let m = measure(msg, 24.0);
        let (x, y) = ((screen_width() - m.width) / 2.0, screen_height() - 28.0);
        pill(x - 20.0, y - 26.0, m.width + 40.0, 38.0, Color::new(INK.r + (0.84 - INK.r) * (1.0 - a), INK.g + (0.91 - INK.g) * (1.0 - a), INK.b + (0.92 - INK.b) * (1.0 - a), 1.0));
        text(msg, x, y, 24.0, Color::new(1.0, 1.0, 1.0, a));
    }
}

fn restart_rect() -> Rect {
    Rect::new(screen_width() / 2.0 - 80.0, screen_height() / 2.0 + 30.0, 160.0, 44.0)
}

fn draw_game_over(g: &Game) {
    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.95, 0.94, 0.9, 0.6));
    let (cx, cy) = (screen_width() / 2.0, screen_height() / 2.0);
    draw_rectangle(cx - 194.0, cy - 102.0, 400.0, 200.0, Color::new(0.0, 0.0, 0.0, 0.15));
    draw_rectangle(cx - 200.0, cy - 110.0, 400.0, 200.0, Color::new(0.98, 0.97, 0.94, 1.0));
    let summary = format!("{} trips in {} weeks", g.sim.score, g.sim.week);
    let lines = [("Traffic jam!", 40.0, -55.0), (summary.as_str(), 26.0, -12.0)];
    for (t, size, dy) in lines {
        let m = measure(t, size);
        text(t, cx - m.width / 2.0, cy + dy, size, INK);
    }
    let r = restart_rect();
    pill(r.x, r.y, r.w, r.h, INK);
    let m = measure("Play again", 24.0);
    text("Play again", r.x + (r.w - m.width) / 2.0, r.y + 29.0, 24.0, WHITE);
}
