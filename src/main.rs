use little_lanes::map::{idx, in_bounds, Pos, Terrain, DIRS, H, W};
use little_lanes::sim::{Cell, CarState, EditResult, Sim, DEST_H, DEST_W, DT, PIN_LIMIT};
use macroquad::prelude::*;

const HUD_H: f32 = 48.0;

const BG: Color = Color::new(0.953, 0.937, 0.902, 1.0);
const WATER: Color = Color::new(0.663, 0.827, 0.910, 1.0);
const HILL: Color = Color::new(0.851, 0.796, 0.690, 1.0);
const ROAD: Color = Color::new(0.47, 0.47, 0.52, 1.0);
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

fn darker(c: Color, f: f32) -> Color {
    Color::new(c.r * f, c.g * f, c.b * f, c.a)
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

struct View {
    ts: f32,
    ox: f32,
    oy: f32,
}

impl View {
    fn compute() -> Self {
        let ts = (screen_width() / W as f32).min((screen_height() - HUD_H) / H as f32).floor().max(4.0);
        let ox = ((screen_width() - ts * W as f32) / 2.0).floor();
        let oy = HUD_H + ((screen_height() - HUD_H - ts * H as f32) / 2.0).floor();
        View { ts, ox, oy }
    }
    fn tile_at(&self, x: f32, y: f32) -> Option<Pos> {
        let p = (((x - self.ox) / self.ts).floor() as i32, ((y - self.oy) / self.ts).floor() as i32);
        in_bounds(p).then_some(p)
    }
    /// Screen position of a point given in tile units (tile centre = x + 0.5).
    fn pt(&self, x: f32, y: f32) -> Vec2 {
        vec2(self.ox + x * self.ts, self.oy + y * self.ts)
    }
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
    let mut g = Game::new();

    loop {
        let view = View::compute();
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
            if let Some(t) = view.tile_at(mx, my) {
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
        clear_background(BG);
        draw_world(&g, &view);
        if !over_ui && !g.sim.game_over {
            if let Some(t) = view.tile_at(mx, my) {
                let p = view.pt(t.0 as f32, t.1 as f32);
                let c = if g.tool == Tool::Erase || down_r { WARN } else { INK };
                draw_rectangle_lines(p.x, p.y, view.ts, view.ts, 2.0, Color::new(c.r, c.g, c.b, 0.5));
            }
        }
        draw_hud(&g, &buttons);
        if g.sim.game_over {
            draw_game_over(&g);
        }

        next_frame().await
    }
}

fn draw_world(g: &Game, v: &View) {
    let s = &g.sim;
    let ts = v.ts;

    // Terrain
    for y in 0..H {
        for x in 0..W {
            let p = v.pt(x as f32, y as f32);
            match s.terrain[idx((x, y))] {
                Terrain::Water => draw_rectangle(p.x, p.y, ts + 0.5, ts + 0.5, WATER),
                Terrain::Hill => {
                    draw_rectangle(p.x, p.y, ts + 0.5, ts + 0.5, HILL);
                    draw_circle(p.x + ts * 0.5, p.y + ts * 0.5, ts * 0.08, darker(HILL, 0.85));
                }
                Terrain::Grass => {}
            }
        }
    }
    // Faint grid
    let grid = Color::new(0.0, 0.0, 0.0, 0.035);
    for x in 0..=W {
        let a = v.pt(x as f32, 0.0);
        draw_line(a.x, a.y, a.x, a.y + ts * H as f32, 1.0, grid);
    }
    for y in 0..=H {
        let a = v.pt(0.0, y as f32);
        draw_line(a.x, a.y, a.x + ts * W as f32, a.y, 1.0, grid);
    }

    // Roads: a disc per tile plus a bar to each connected neighbour.
    let rw = ts * 0.56;
    for y in 0..H {
        for x in 0..W {
            let c = s.cells[idx((x, y))];
            let is_link = matches!(c, Cell::Road | Cell::Door(_));
            if !is_link {
                continue;
            }
            let ctr = v.pt(x as f32 + 0.5, y as f32 + 0.5);
            let col = match c {
                Cell::Door(d) => darker(PALETTE[s.dests[d].color], 0.8),
                _ => ROAD,
            };
            draw_circle(ctr.x, ctr.y, rw * 0.5, col);
            for d in DIRS {
                let n = (x + d.0, y + d.1);
                let connects = match s.cell(n) {
                    Cell::Road => true,
                    Cell::Home(_) => c == Cell::Road,
                    Cell::Door(_) => c == Cell::Road,
                    Cell::Dest(id) => matches!(c, Cell::Door(did) if did == id),
                    Cell::Empty => false,
                };
                if connects {
                    let e = v.pt(x as f32 + 0.5 + d.0 as f32 * 0.5, y as f32 + 0.5 + d.1 as f32 * 0.5);
                    draw_line(ctr.x, ctr.y, e.x, e.y, rw, if matches!(s.cell(n), Cell::Dest(_)) { col } else { ROAD });
                }
            }
        }
    }

    // Destinations
    for (i, d) in s.dests.iter().enumerate() {
        let col = PALETTE[d.color];
        let p = v.pt(d.origin.0 as f32 + 0.08, d.origin.1 as f32 + 0.08);
        let (w, h) = (ts * (DEST_W as f32 - 0.16), ts * (DEST_H as f32 - 0.16));
        draw_rectangle(p.x, p.y, w, h, col);
        draw_rectangle_lines(p.x, p.y, w, h, 2.0, darker(col, 0.75));
        // Pins: white dots, hollow once a car is on its way.
        let r = ts * 0.13;
        for k in 0..d.pins.min(12) {
            let (cx, cy) = (k % 6, k / 6);
            let px = p.x + ts * 0.3 + cx as f32 * ts * 0.45;
            let py = p.y + ts * 0.42 + cy as f32 * ts * 0.6;
            if k < d.assigned {
                draw_circle_lines(px, py, r, 2.0, WHITE);
            } else {
                draw_circle(px, py, r, WHITE);
            }
        }
        if d.overflow > 0.0 || d.pins > PIN_LIMIT {
            let c = v.pt(d.origin.0 as f32 + DEST_W as f32 / 2.0, d.origin.1 as f32 + DEST_H as f32 / 2.0);
            let rad = ts * 2.1;
            draw_circle_lines(c.x, c.y, rad, 3.0, Color::new(WARN.r, WARN.g, WARN.b, 0.2));
            let pulse = if g.sim.failed_dest == Some(i) { 1.0 } else { 0.75 + 0.25 * (get_time() as f32 * 6.0).sin() };
            draw_arc(c.x, c.y, 48, rad, -90.0, 4.0, 360.0 * d.overflow.min(1.0), Color::new(WARN.r, WARN.g, WARN.b, pulse));
        }
    }

    // Homes
    for (i, h) in s.homes.iter().enumerate() {
        let col = PALETTE[h.color];
        let p = v.pt(h.pos.0 as f32 + 0.15, h.pos.1 as f32 + 0.15);
        draw_rectangle(p.x, p.y, ts * 0.7, ts * 0.7, col);
        draw_rectangle_lines(p.x, p.y, ts * 0.7, ts * 0.7, 2.0, darker(col, 0.75));
        if s.cars[i].state == CarState::Idle {
            draw_circle(p.x + ts * 0.35, p.y + ts * 0.35, ts * 0.12, WHITE);
        }
    }

    // Cars, driving on the right-hand side of the road.
    for (i, c) in s.cars.iter().enumerate() {
        if c.state == CarState::Idle {
            continue;
        }
        let col = darker(PALETTE[s.homes[i].color], 0.9);
        let (ax, ay) = (c.prev.0 as f32, c.prev.1 as f32);
        let (bx, by) = (c.cur.0 as f32, c.cur.1 as f32);
        let t = c.progress;
        let (x, y) = (ax + (bx - ax) * t, ay + (by - ay) * t);
        let mut dir = vec2(bx - ax, by - ay);
        if dir.length_squared() < 0.01 {
            dir = c.path.get(c.i + 1).map(|n| vec2((n.0 - c.cur.0) as f32, (n.1 - c.cur.1) as f32)).unwrap_or(vec2(1.0, 0.0));
        }
        let normal = vec2(-dir.y, dir.x) * 0.14;
        let ctr = v.pt(x + 0.5 + normal.x, y + 0.5 + normal.y);
        let (len, wid) = (ts * 0.42, ts * 0.24);
        let horizontal = dir.x.abs() > dir.y.abs();
        let (w, h) = if horizontal { (len, wid) } else { (wid, len) };
        draw_rectangle(ctr.x - w / 2.0, ctr.y - h / 2.0, w, h, col);
    }

    if let Some((p, t)) = g.flash {
        let q = v.pt(p.0 as f32, p.1 as f32);
        draw_rectangle(q.x, q.y, ts, ts, Color::new(WARN.r, WARN.g, WARN.b, t));
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

fn draw_hud(g: &Game, buttons: &[Button]) {
    let s = &g.sim;
    draw_rectangle(0.0, 0.0, screen_width(), HUD_H, Color::new(1.0, 1.0, 1.0, 0.85));
    draw_line(0.0, HUD_H, screen_width(), HUD_H, 1.0, Color::new(0.0, 0.0, 0.0, 0.08));
    let text = format!("Trips {}    Week {}    Roads {}", s.score, s.week, s.roads_left);
    draw_text(&text, 16.0, 31.0, 26.0, INK);
    // Week progress bar
    let bx = 16.0 + measure_text(&text, None, 26, 1.0).width + 16.0;
    draw_rectangle(bx, 21.0, 80.0, 6.0, Color::new(0.0, 0.0, 0.0, 0.08));
    draw_rectangle(bx, 21.0, 80.0 * s.week_progress(), 6.0, INK);

    for (i, b) in buttons.iter().enumerate() {
        let active = match i {
            0 => g.speed == 0,
            1 => g.speed == 1,
            2 => g.speed == 2,
            _ => g.tool == Tool::Erase,
        };
        let fill = if active { if i == 3 { WARN } else { INK } } else { Color::new(0.0, 0.0, 0.0, 0.06) };
        let tc = if active { WHITE } else { INK };
        draw_rectangle(b.rect.x, b.rect.y, b.rect.w, b.rect.h, fill);
        let m = measure_text(b.label, None, 22, 1.0);
        draw_text(b.label, b.rect.x + (b.rect.w - m.width) / 2.0, b.rect.y + 22.0, 22.0, tc);
    }

    if let Some((msg, t)) = &s.toast {
        let a = t.min(1.0);
        let m = measure_text(msg, None, 24, 1.0);
        let (x, y) = ((screen_width() - m.width) / 2.0, screen_height() - 28.0);
        draw_rectangle(x - 14.0, y - 26.0, m.width + 28.0, 38.0, Color::new(INK.r, INK.g, INK.b, 0.85 * a));
        draw_text(msg, x, y, 24.0, Color::new(1.0, 1.0, 1.0, a));
    }
}

fn restart_rect() -> Rect {
    Rect::new(screen_width() / 2.0 - 80.0, screen_height() / 2.0 + 30.0, 160.0, 44.0)
}

fn draw_game_over(g: &Game) {
    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.95, 0.94, 0.9, 0.6));
    let (cx, cy) = (screen_width() / 2.0, screen_height() / 2.0);
    draw_rectangle(cx - 200.0, cy - 110.0, 400.0, 200.0, WHITE);
    draw_rectangle_lines(cx - 200.0, cy - 110.0, 400.0, 200.0, 2.0, INK);
    let summary = format!("{} trips in {} weeks", g.sim.score, g.sim.week);
    let lines = [("Traffic jam!", 40.0, -55.0), (summary.as_str(), 26.0, -12.0)];
    for (t, size, dy) in lines {
        let m = measure_text(t, None, size as u16, 1.0);
        draw_text(t, cx - m.width / 2.0, cy + dy, size, INK);
    }
    let r = restart_rect();
    draw_rectangle(r.x, r.y, r.w, r.h, INK);
    let m = measure_text("Play again", None, 24, 1.0);
    draw_text("Play again", r.x + (r.w - m.width) / 2.0, r.y + 29.0, 24.0, WHITE);
}
