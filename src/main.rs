use little_lanes::map::{idx, MapKind, Pos, Terrain, H, W};
use little_lanes::sim::{Cell, EditResult, Event, Junction, Sim, Upgrade, DT};
use macroquad::prelude::*;

mod platform;
mod render3d;

use platform::Sfx;
use std::cell::RefCell;

const HUD_H: f32 = 48.0;
/// Space kept free at the bottom for the tool bar.
const BAR_H: f32 = 76.0;

const INK: Color = Color::new(0.20, 0.20, 0.24, 1.0);
const PAPER: Color = Color::new(0.98, 0.97, 0.94, 1.0);
const CHIP: Color = Color::new(0.0, 0.0, 0.0, 0.06);
const WARN: Color = Color::new(0.85, 0.25, 0.22, 1.0);
const GOLD: Color = Color::new(0.96, 0.74, 0.22, 1.0);
const ROAD_GREY: Color = Color::new(0.33, 0.35, 0.40, 1.0);

const PALETTE: [Color; 6] = [
    Color::new(0.937, 0.490, 0.435, 1.0), // coral
    Color::new(0.310, 0.702, 0.663, 1.0), // teal
    Color::new(0.914, 0.706, 0.298, 1.0), // mustard
    Color::new(0.557, 0.486, 0.765, 1.0), // violet
    Color::new(0.357, 0.608, 0.835, 1.0), // sky
    Color::new(0.910, 0.561, 0.706, 1.0), // pink
];

const MIN_ZOOM: f32 = 1.0;
const MAX_ZOOM: f32 = 3.5;

thread_local! {
    static FONT: RefCell<Option<Font>> = const { RefCell::new(None) };
}

// ---- Small drawing helpers ------------------------------------------------------

/// Screen units per UI unit. macroquad already reports sizes in logical (CSS) pixels on the
/// web, so this is 1; it is kept as one place to change if a platform reports device pixels.
fn ui_scale() -> f32 {
    1.0
}

/// Logical UI width and height.
fn uw() -> f32 {
    screen_width() / ui_scale()
}

fn uh() -> f32 {
    screen_height() / ui_scale()
}

/// Narrow screens (phones held upright) get a compact top bar.
fn narrow() -> bool {
    uw() < 600.0
}

fn ui_camera() -> Camera2D {
    let (w, h) = (uw(), uh());
    Camera2D { target: vec2(w / 2.0, h / 2.0), zoom: vec2(2.0 / w, 2.0 / h), ..Default::default() }
}

fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

/// Draw text with the bundled Inter font (falls back to the built-in font if it failed to load).
fn text(t: &str, x: f32, y: f32, size: f32, color: Color) {
    FONT.with(|f| {
        let f = f.borrow();
        let k = ui_scale();
        draw_text_ex(t, x, y, TextParams { font: f.as_ref(), font_size: (size * k) as u16, font_scale: 1.0 / k, color, ..Default::default() });
    });
}

fn measure(t: &str, size: f32) -> TextDimensions {
    let k = ui_scale();
    FONT.with(|f| measure_text(t, f.borrow().as_ref(), (size * k) as u16, 1.0 / k))
}

/// Text centred horizontally on `cx`, with its baseline at `y`.
fn text_centered(t: &str, cx: f32, y: f32, size: f32, color: Color) {
    let m = measure(t, size);
    text(t, cx - m.width / 2.0, y, size, color);
}

/// Split text into lines no wider than `max_w`.
fn wrap(t: &str, size: f32, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in t.split_whitespace() {
        let next = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
        if measure(&next, size).width > max_w && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = next;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Rounded rectangle drawn as one triangle fan, so translucent fills don't double up at the corners.
fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32, c: Color) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let mut pts = Vec::with_capacity(40);
    let corners = [(x + w - r, y + r, -90.0f32), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, 90.0), (x + r, y + r, 180.0)];
    for (cx, cy, start) in corners {
        for k in 0..=8 {
            let a = (start + k as f32 * 90.0 / 8.0).to_radians();
            pts.push(vec2(cx + a.cos() * r, cy + a.sin() * r));
        }
    }
    let centre = vec2(x + w / 2.0, y + h / 2.0);
    for i in 0..pts.len() {
        draw_triangle(centre, pts[i], pts[(i + 1) % pts.len()], c);
    }
}

/// A pill with a soft drop shadow, for controls that float over the map.
fn float_pill(x: f32, y: f32, w: f32, h: f32, c: Color) {
    pill(x + 1.0, y + 3.0, w, h, Color::new(0.0, 0.0, 0.0, 0.10));
    pill(x, y, w, h, c);
}

fn pill(x: f32, y: f32, w: f32, h: f32, c: Color) {
    rrect(x, y, w, h, h / 2.0, c);
}

/// A raised card with a soft shadow.
fn card(r: Rect, fill: Color) {
    rrect(r.x + 2.0, r.y + 6.0, r.w, r.h, 18.0, Color::new(0.0, 0.0, 0.0, 0.10));
    rrect(r.x, r.y, r.w, r.h, 18.0, fill);
}

// ---- Icons ------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Icon {
    Road,
    Erase,
    Roundabout,
    Light,
    Highway,
    Bridge,
    Tunnel,
    MoreRoads,
}

fn upgrade_icon(u: Upgrade) -> Icon {
    match u {
        Upgrade::Bridge => Icon::Bridge,
        Upgrade::Tunnel => Icon::Tunnel,
        Upgrade::Roundabout => Icon::Roundabout,
        Upgrade::TrafficLight => Icon::Light,
        Upgrade::Highway => Icon::Highway,
        Upgrade::ExtraRoads => Icon::MoreRoads,
    }
}

/// Draw an icon centred on (cx, cy). `s` = 1 fits a 32 px box.
fn draw_icon(icon: Icon, cx: f32, cy: f32, s: f32, dim: bool) {
    let a = if dim { 0.35 } else { 1.0 };
    let c = |col: Color| with_alpha(col, a);
    let white = c(WHITE);
    match icon {
        Icon::Road | Icon::MoreRoads => {
            rrect(cx - 15.0 * s, cy - 8.0 * s, 30.0 * s, 16.0 * s, 4.0 * s, c(ROAD_GREY));
            for k in 0..3 {
                draw_rectangle(cx - 11.0 * s + k as f32 * 9.0 * s, cy - 1.0 * s, 5.0 * s, 2.0 * s, white);
            }
            if matches!(icon, Icon::MoreRoads) {
                draw_circle(cx + 13.0 * s, cy - 9.0 * s, 8.0 * s, c(GOLD));
                draw_rectangle(cx + 9.0 * s, cy - 10.0 * s, 8.0 * s, 2.0 * s, white);
                draw_rectangle(cx + 12.0 * s, cy - 13.0 * s, 2.0 * s, 8.0 * s, white);
            }
        }
        Icon::Erase => {
            rrect(cx - 13.0 * s, cy - 7.0 * s, 26.0 * s, 14.0 * s, 4.0 * s, c(Color::new(0.95, 0.62, 0.66, 1.0)));
            rrect(cx - 13.0 * s, cy - 7.0 * s, 10.0 * s, 14.0 * s, 4.0 * s, c(Color::new(0.45, 0.55, 0.78, 1.0)));
            draw_line(cx - 12.0 * s, cy + 12.0 * s, cx + 12.0 * s, cy + 12.0 * s, 2.0 * s, c(INK));
        }
        Icon::Roundabout => {
            draw_circle(cx, cy, 14.0 * s, c(ROAD_GREY));
            draw_circle_lines(cx, cy, 9.5 * s, 1.5 * s, white);
            draw_circle(cx, cy, 6.0 * s, c(Color::new(0.45, 0.72, 0.42, 1.0)));
        }
        Icon::Light => {
            rrect(cx - 6.0 * s, cy - 15.0 * s, 12.0 * s, 30.0 * s, 4.0 * s, c(INK));
            draw_circle(cx, cy - 8.0 * s, 3.4 * s, c(Color::new(0.95, 0.35, 0.3, 1.0)));
            draw_circle(cx, cy, 3.4 * s, c(GOLD));
            draw_circle(cx, cy + 8.0 * s, 3.4 * s, c(Color::new(0.35, 0.8, 0.45, 1.0)));
        }
        Icon::Highway => {
            for dx in [-9.0, 9.0] {
                draw_rectangle(cx + dx * s - 2.5 * s, cy - 2.0 * s, 5.0 * s, 14.0 * s, c(Color::new(0.62, 0.62, 0.66, 1.0)));
            }
            rrect(cx - 16.0 * s, cy - 10.0 * s, 32.0 * s, 10.0 * s, 3.0 * s, c(ROAD_GREY));
            draw_rectangle(cx - 16.0 * s, cy - 10.0 * s, 32.0 * s, 2.0 * s, c(GOLD));
        }
        Icon::Bridge => {
            rrect(cx - 16.0 * s, cy + 2.0 * s, 32.0 * s, 12.0 * s, 4.0 * s, c(Color::new(0.45, 0.70, 0.88, 1.0)));
            for dx in [-8.0, 8.0] {
                draw_rectangle(cx + dx * s - 2.0 * s, cy - 4.0 * s, 4.0 * s, 14.0 * s, c(Color::new(0.85, 0.82, 0.76, 1.0)));
            }
            rrect(cx - 16.0 * s, cy - 8.0 * s, 32.0 * s, 7.0 * s, 2.0 * s, c(ROAD_GREY));
        }
        Icon::Tunnel => {
            draw_circle(cx, cy + 4.0 * s, 14.0 * s, c(Color::new(0.62, 0.70, 0.50, 1.0)));
            draw_rectangle(cx - 16.0 * s, cy + 9.0 * s, 32.0 * s, 8.0 * s, c(PAPER));
            draw_circle(cx, cy + 5.0 * s, 6.0 * s, c(INK));
            draw_rectangle(cx - 6.0 * s, cy + 5.0 * s, 12.0 * s, 4.0 * s, c(INK));
            rrect(cx - 16.0 * s, cy + 9.0 * s, 32.0 * s, 5.0 * s, 2.0 * s, c(ROAD_GREY));
        }
    }
}

// ---- Game state -------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tool {
    Road,
    Erase,
    Roundabout,
    Light,
    Highway,
}

const TOOLS: [Tool; 5] = [Tool::Road, Tool::Erase, Tool::Roundabout, Tool::Light, Tool::Highway];

impl Tool {
    fn icon(self) -> Icon {
        match self {
            Tool::Road => Icon::Road,
            Tool::Erase => Icon::Erase,
            Tool::Roundabout => Icon::Roundabout,
            Tool::Light => Icon::Light,
            Tool::Highway => Icon::Highway,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Tool::Road => "Road (R)",
            Tool::Erase => "Erase (E)",
            Tool::Roundabout => "Roundabout (O)",
            Tool::Light => "Traffic light (L)",
            Tool::Highway => "Highway (H)",
        }
    }
    /// How many of this tool's pieces are left, if it uses them up.
    fn count(self, s: &Sim) -> Option<u32> {
        match self {
            Tool::Road => Some(s.roads_left),
            Tool::Erase => None,
            Tool::Roundabout => Some(s.inventory.roundabouts),
            Tool::Light => Some(s.inventory.lights),
            Tool::Highway => Some(s.inventory.highways),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Action {
    Speed(u32),
    Tool(Tool),
    Mute,
    Menu,
    Choose(usize),
    Start(usize),
    Resume,
    Restart,
}

struct Btn {
    rect: Rect,
    action: Action,
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Title,
    Play,
}

struct Game {
    screen: Screen,
    map: MapKind,
    sim: Sim,
    /// A game exists that can be resumed from the title screen.
    in_game: bool,
    /// Map shown behind the title screen.
    title_sim: Sim,
    title_map: MapKind,
    tool: Tool,
    speed: u32, // 0 = paused, 1, 2
    acc: f32,
    last_tile: Option<Pos>,
    drag_erase: bool,
    flash: Option<(Pos, f32)>,
    highway_from: Option<Pos>,
    zoom: f32,
    pan: Vec2,
    mid_drag: Option<Vec2>,
    pinch: Option<(f32, Vec2)>,
    touch_lock: bool,
    new_best: bool,
    trip_cooldown: f32,
    /// Car poses before the latest simulation step, so drawing can blend between steps.
    prev_poses: Vec<Option<(Vec2, Vec2)>>,
    world_cache: render3d::WorldCache,
    title_cache: render3d::WorldCache,
}

fn seed() -> u64 {
    // A fixed seed can be baked in for testing: LL_SEED=42 ./build_web.sh
    if let Some(s) = option_env!("LL_SEED").and_then(|s| s.parse().ok()) {
        return s;
    }
    (macroquad::miniquad::date::now() * 1000.0) as u64
}

impl Game {
    fn new() -> Self {
        Game {
            screen: Screen::Title,
            map: MapKind::Bayside,
            sim: Sim::new_on(MapKind::Bayside, seed()),
            in_game: false,
            title_sim: Sim::new_on(MapKind::Bayside, seed()),
            title_map: MapKind::Bayside,
            tool: Tool::Road,
            speed: 1,
            acc: 0.0,
            last_tile: None,
            drag_erase: false,
            flash: None,
            highway_from: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            mid_drag: None,
            pinch: None,
            touch_lock: false,
            new_best: false,
            trip_cooldown: 0.0,
            prev_poses: Vec::new(),
            world_cache: Default::default(),
            title_cache: Default::default(),
        }
    }

    fn start(&mut self, map: MapKind) {
        self.map = map;
        self.sim = Sim::new_on(map, seed());
        self.in_game = true;
        self.screen = Screen::Play;
        self.tool = Tool::Road;
        self.speed = 1;
        self.acc = 0.0;
        self.highway_from = None;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.new_best = false;
    }

    fn scene(&self) -> render3d::Scene {
        render3d::Scene::new(HUD_H * ui_scale(), BAR_H * ui_scale(), self.zoom, self.pan)
    }

    fn clamp_pan(&mut self) {
        let lim = vec2(W as f32 * 0.5, H as f32 * 0.5) * (1.0 - 1.0 / self.zoom);
        self.pan = self.pan.clamp(-lim, lim);
    }

    /// Zoom by `factor` keeping the ground under screen point `at` in place.
    fn zoom_at(&mut self, factor: f32, at: Vec2) {
        let before = self.scene().ground_at(at.x, at.y, 0.0);
        let nz = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        if (nz - self.zoom).abs() < 1e-4 {
            return;
        }
        self.zoom = nz;
        self.clamp_pan();
        if let (Some(b), Some(a)) = (before, self.scene().ground_at(at.x, at.y, 0.0)) {
            self.pan += b - a;
            self.clamp_pan();
        }
    }

    /// Move the view so the ground under `from` ends up under `to`.
    fn drag_view(&mut self, from: Vec2, to: Vec2) {
        let sc = self.scene();
        if let (Some(a), Some(b)) = (sc.ground_at(from.x, from.y, 0.0), sc.ground_at(to.x, to.y, 0.0)) {
            self.pan -= b - a;
            self.clamp_pan();
        }
    }

    fn select_tool(&mut self, t: Tool) {
        self.tool = t;
        self.highway_from = None;
        if t.count(&self.sim) == Some(0) && t != Tool::Road {
            let msg = match t {
                Tool::Roundabout => "No roundabouts yet. Pick one at the end of a week",
                Tool::Light => "No traffic lights yet. Pick one at the end of a week",
                _ => "No highways yet. Pick one at the end of a week",
            };
            self.sim.toast = Some((msg.into(), 3.0));
        }
    }

    fn act(&mut self, a: Action) {
        match a {
            Action::Speed(s) => self.speed = s,
            Action::Tool(t) => self.select_tool(t),
            Action::Mute => platform::set_muted(!platform::muted()),
            Action::Menu => {
                self.screen = Screen::Title;
                self.highway_from = None;
            }
            Action::Choose(i) => {
                self.sim.choose(i);
                platform::play(Sfx::Pick);
            }
            Action::Start(i) => self.start(MapKind::ALL[i.min(2)]),
            Action::Resume => self.screen = Screen::Play,
            Action::Restart => self.start(self.map),
        }
    }
}

// ---- Layout -------------------------------------------------------------------------

fn top_buttons(with_game: bool) -> Vec<Btn> {
    let mut items: Vec<(Action, f32)> = Vec::new();
    if with_game {
        let w = if narrow() { 36.0 } else { 44.0 };
        items.extend([(Action::Speed(0), w), (Action::Speed(1), w), (Action::Speed(2), w)]);
    }
    items.push((Action::Mute, if narrow() { 36.0 } else { 44.0 }));
    if with_game {
        items.push((Action::Menu, if narrow() { 36.0 } else { 72.0 }));
    }
    let mut x = uw() - 12.0;
    let mut out = Vec::new();
    for (a, w) in items.into_iter().rev() {
        x -= w;
        out.push(Btn { rect: Rect::new(x, 8.0, w, 32.0), action: a });
        x -= 8.0;
    }
    out.reverse();
    out
}

const TOOL_W: f32 = 64.0;
const TOOL_H: f32 = 56.0;
const TOOL_GAP: f32 = 8.0;

fn bar_rect() -> Rect {
    let w = TOOL_W * 5.0 + TOOL_GAP * 4.0 + 20.0;
    Rect::new((uw() - w) / 2.0, uh() - TOOL_H - 18.0, w, TOOL_H + 12.0)
}

fn tool_buttons() -> Vec<Btn> {
    let bar = bar_rect();
    TOOLS
        .iter()
        .enumerate()
        .map(|(i, t)| Btn {
            rect: Rect::new(bar.x + 10.0 + i as f32 * (TOOL_W + TOOL_GAP), bar.y + 6.0, TOOL_W, TOOL_H),
            action: Action::Tool(*t),
        })
        .collect()
}

fn offer_buttons() -> Vec<Btn> {
    let (sw, sh) = (uw(), uh());
    if sw >= 560.0 {
        let (w, h) = (230.0, 250.0);
        let y = sh / 2.0 - h / 2.0 + 30.0;
        (0..2).map(|i| Btn { rect: Rect::new(sw / 2.0 - w - 12.0 + i as f32 * (w + 24.0), y, w, h), action: Action::Choose(i) }).collect()
    } else {
        let (w, h) = (sw - 40.0, 150.0);
        (0..2).map(|i| Btn { rect: Rect::new(20.0, sh / 2.0 - h - 6.0 + 30.0 + i as f32 * (h + 12.0), w, h), action: Action::Choose(i) }).collect()
    }
}

fn title_buttons(in_game: bool) -> Vec<Btn> {
    let (sw, sh) = (uw(), uh());
    let mut out = Vec::new();
    let wide = sw >= 820.0;
    let bottom;
    if wide {
        let (w, h, gap) = (240.0, 170.0, 20.0);
        let x0 = (sw - (3.0 * w + 2.0 * gap)) / 2.0;
        let y = sh * 0.42;
        for i in 0..3 {
            out.push(Btn { rect: Rect::new(x0 + i as f32 * (w + gap), y, w, h), action: Action::Start(i) });
        }
        bottom = y + h;
    } else {
        let (w, h, gap) = ((sw - 40.0).min(380.0), 104.0, 12.0);
        let y0 = sh * 0.30;
        for i in 0..3 {
            out.push(Btn { rect: Rect::new((sw - w) / 2.0, y0 + i as f32 * (h + gap), w, h), action: Action::Start(i) });
        }
        bottom = y0 + 3.0 * h + 2.0 * gap;
    }
    if in_game {
        out.push(Btn { rect: Rect::new(sw / 2.0 - 90.0, bottom + 24.0, 180.0, 46.0), action: Action::Resume });
    }
    out.extend(top_buttons(false));
    out
}

fn game_over_buttons() -> Vec<Btn> {
    let (cx, cy) = (uw() / 2.0, uh() / 2.0);
    vec![
        Btn { rect: Rect::new(cx - 166.0, cy + 54.0, 156.0, 46.0), action: Action::Restart },
        Btn { rect: Rect::new(cx + 10.0, cy + 54.0, 156.0, 46.0), action: Action::Menu },
    ]
}

fn buttons_for(g: &Game) -> Vec<Btn> {
    match g.screen {
        Screen::Title => title_buttons(g.in_game),
        Screen::Play if g.sim.game_over => game_over_buttons(),
        Screen::Play if g.sim.offer.is_some() => {
            let mut b = offer_buttons();
            b.extend(top_buttons(false));
            b
        }
        Screen::Play => {
            let mut b = top_buttons(true);
            b.extend(tool_buttons());
            b
        }
    }
}

// ---- Main loop ------------------------------------------------------------------

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

#[macroquad::main(window_conf)]
async fn main() {
    if let Ok(f) = load_ttf_font_from_bytes(include_bytes!("../assets/Inter-SemiBold.otf")) {
        FONT.with(|cell| *cell.borrow_mut() = Some(f));
    }
    platform::init();
    let mut g = Game::new();

    loop {
        // `mp` is in screen pixels (for the 3D world), `m` in UI units (for buttons and panels).
        let mp: Vec2 = mouse_position().into();
        let m = mp / ui_scale();
        let btns = buttons_for(&g);

        // Test builds print where tiles and buttons are on screen so automated tests can click them.
        if option_env!("LL_SEED").is_some() && is_key_pressed(KeyCode::F9) {
            let scene = g.scene();
            let mut out = String::new();
            for y in 0..H {
                for x in 0..W {
                    let p = scene.to_screen(vec3(x as f32 + 0.5, 0.0, y as f32 + 0.5));
                    let t = match g.sim.terrain[idx((x, y))] {
                        Terrain::Grass => 'g',
                        Terrain::Water => 'w',
                        Terrain::Hill => 'h',
                    };
                    let c = match g.sim.cell((x, y)) {
                        Cell::Empty => '.',
                        Cell::Road => 'r',
                        Cell::Home(_) => 'H',
                        Cell::Dest(_) => 'D',
                        Cell::Door(_) => 'd',
                    };
                    out.push_str(&format!("{},{},{:.1},{:.1},{}{};", x, y, p.x, p.y, t, c));
                }
            }
            macroquad::miniquad::info!("TILES {}", out);
            let b: Vec<String> = btns.iter().map(|b| format!("{:?}@{:.0},{:.0}", b.action, b.rect.center().x, b.rect.center().y)).collect();
            macroquad::miniquad::info!("BUTTONS {}", b.join(";"));
        }

        // Test builds: F8 hands out a stack of every upgrade.
        if option_env!("LL_SEED").is_some() && is_key_pressed(KeyCode::F8) {
            let inv = &mut g.sim.inventory;
            inv.bridge_tiles += 8;
            inv.tunnel_tiles += 8;
            inv.roundabouts += 3;
            inv.lights += 3;
            inv.highways += 2;
            g.sim.roads_left += 100;
        }
        // Test builds: F7 ends the game so the game-over card can be checked.
        if option_env!("LL_SEED").is_some() && is_key_pressed(KeyCode::F7) && g.screen == Screen::Play {
            g.sim.game_over = true;
            g.sim.events.push(Event::GameOver);
        }

        // ---- Buttons --------------------------------------------------------
        let mut consumed = false;
        if is_mouse_button_pressed(MouseButton::Left) {
            if let Some(b) = btns.iter().find(|b| b.rect.contains(m)) {
                consumed = true;
                platform::play(Sfx::Click);
                g.act(b.action);
            }
        }

        // ---- Keys -----------------------------------------------------------
        let offer_open = g.screen == Screen::Play && g.sim.offer.is_some();
        if is_key_pressed(KeyCode::M) {
            g.act(Action::Mute);
        }
        match g.screen {
            Screen::Title => {
                for (i, k) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3].iter().enumerate() {
                    if is_key_pressed(*k) {
                        g.act(Action::Start(i));
                    }
                }
                if g.in_game && is_key_pressed(KeyCode::Escape) {
                    g.act(Action::Resume);
                }
            }
            Screen::Play if g.sim.game_over => {
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    g.act(Action::Restart);
                }
            }
            Screen::Play if offer_open => {
                if is_key_pressed(KeyCode::Key1) || is_key_pressed(KeyCode::Left) {
                    g.act(Action::Choose(0));
                }
                if is_key_pressed(KeyCode::Key2) || is_key_pressed(KeyCode::Right) {
                    g.act(Action::Choose(1));
                }
            }
            Screen::Play => {
                if is_key_pressed(KeyCode::Space) {
                    g.speed = if g.speed == 0 { 1 } else { 0 };
                }
                if is_key_pressed(KeyCode::Key1) {
                    g.speed = 1;
                }
                if is_key_pressed(KeyCode::Key2) {
                    g.speed = 2;
                }
                let keys = [(KeyCode::R, Tool::Road), (KeyCode::E, Tool::Erase), (KeyCode::O, Tool::Roundabout), (KeyCode::L, Tool::Light), (KeyCode::H, Tool::Highway)];
                for (k, t) in keys {
                    if is_key_pressed(k) {
                        g.select_tool(t);
                    }
                }
                if is_key_pressed(KeyCode::Escape) {
                    if g.highway_from.is_some() {
                        g.highway_from = None;
                    } else {
                        g.act(Action::Menu);
                    }
                }
            }
        }

        // ---- World input and simulation ------------------------------------------
        if g.screen == Screen::Play {
            play_input(&mut g, consumed, m, mp);
            step_sim(&mut g);
        } else {
            // Hovering a map card previews it behind the title.
            if let Some(Btn { action: Action::Start(i), .. }) = btns.iter().find(|b| b.rect.contains(m)) {
                let k = MapKind::ALL[*i];
                if k != g.title_map {
                    g.title_map = k;
                    g.title_sim = Sim::new_on(k, seed());
                }
            }
        }

        // ---- Drawing ----------------------------------------------------------
        clear_background(render3d::SKY);
        let time = get_time() as f32;
        match g.screen {
            Screen::Title => {
                let scene = render3d::Scene::new(0.0, 0.0, 1.0, Vec2::ZERO);
                set_camera(&scene.cam);
                render3d::draw_world(&g.title_sim, time, &render3d::Highlight { tile: None, tint: WHITE, flash: None, highway_from: None }, &mut g.title_cache, &[]);
                set_camera(&ui_camera());
                draw_title(&g, &btns, m);
            }
            Screen::Play => {
                let scene = g.scene();
                let over_ui = m.y < HUD_H || bar_rect().contains(m) || g.sim.offer.is_some() || g.sim.game_over;
                let hover = if over_ui { None } else { scene.pick_on(&g.sim, mp.x, mp.y) };
                let tint = hover_tint(&g, hover);
                let poses = smooth_poses(&g);
                set_camera(&scene.cam);
                render3d::draw_world(
                    &g.sim,
                    time,
                    &render3d::Highlight {
                        tile: hover,
                        tint,
                        flash: g.flash,
                        highway_from: g.highway_from.map(|f| (f, hover.is_some_and(|h| g.sim.highway_ok(f, h)))),
                    },
                    &mut g.world_cache,
                    &poses,
                );
                set_default_camera();
                render3d::draw_overlays(&g.sim, &scene, time);
                set_camera(&ui_camera());
                draw_hud(&g, &btns);
                if !g.sim.game_over && g.sim.offer.is_none() {
                    draw_toolbar(&g, &btns, m);
                }
                draw_toast(&g);
                if g.sim.offer.is_some() {
                    draw_offer(&g, &btns, m);
                }
                if g.sim.game_over {
                    draw_game_over(&g, &btns, m);
                }
            }
        }

        set_default_camera();
        next_frame().await
    }
}

fn hover_tint(g: &Game, hover: Option<Pos>) -> Color {
    let erase = g.tool == Tool::Erase || is_mouse_button_down(MouseButton::Right);
    match (g.tool, hover) {
        _ if erase => with_alpha(WARN, 0.45),
        (Tool::Roundabout | Tool::Light, Some(p)) => {
            if g.sim.cell(p) == Cell::Road {
                with_alpha(GOLD, 0.6)
            } else {
                with_alpha(WARN, 0.4)
            }
        }
        (Tool::Highway, _) => with_alpha(GOLD, 0.55),
        _ => Color::new(1.0, 1.0, 1.0, 0.38),
    }
}

fn play_input(g: &mut Game, consumed: bool, m: Vec2, mp: Vec2) {
    let dt = get_frame_time().min(0.1);

    // Keyboard panning
    let mut k = Vec2::ZERO;
    if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
        k.y -= 1.0;
    }
    if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
        k.y += 1.0;
    }
    if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
        k.x -= 1.0;
    }
    if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
        k.x += 1.0;
    }
    if k != Vec2::ZERO && g.zoom > 1.0 {
        g.pan += k * 16.0 * dt / g.zoom;
        g.clamp_pan();
    }
    if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::KpAdd) {
        g.zoom_at(1.25, vec2(screen_width() / 2.0, screen_height() / 2.0));
    }
    if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::KpSubtract) {
        g.zoom_at(1.0 / 1.25, vec2(screen_width() / 2.0, screen_height() / 2.0));
    }

    let blocked = g.sim.offer.is_some() || g.sim.game_over;
    let over_ui = consumed || m.y < HUD_H || bar_rect().contains(m) || blocked;

    // Mouse wheel zoom around the cursor
    let wheel = mouse_wheel().1;
    if wheel != 0.0 && !blocked {
        g.zoom_at(if wheel > 0.0 { 1.15 } else { 1.0 / 1.15 }, mp);
    }

    // Middle-drag panning
    if is_mouse_button_down(MouseButton::Middle) {
        if let Some(prev) = g.mid_drag {
            g.drag_view(prev, mp);
        }
        g.mid_drag = Some(mp);
    } else {
        g.mid_drag = None;
    }

    // Two-finger pinch and pan
    let ts = touches();
    if ts.len() >= 2 {
        g.touch_lock = true;
        let (a, b) = (ts[0].position, ts[1].position);
        let (d, c) = ((a - b).length(), (a + b) * 0.5);
        if let Some((d0, c0)) = g.pinch {
            if d0 > 1.0 {
                g.zoom_at(d / d0, c);
            }
            g.drag_view(c0, c);
        }
        g.pinch = Some((d, c));
    } else {
        g.pinch = None;
        if ts.is_empty() {
            g.touch_lock = false;
        }
    }
    if g.touch_lock || blocked {
        g.last_tile = None;
        return;
    }

    let scene = g.scene();
    let pressed_l = is_mouse_button_pressed(MouseButton::Left) && !over_ui;
    let pressed_r = is_mouse_button_pressed(MouseButton::Right) && !over_ui;
    let down_l = is_mouse_button_down(MouseButton::Left);
    let down_r = is_mouse_button_down(MouseButton::Right);
    let tile = scene.pick_on(&g.sim, mp.x, mp.y);

    // Right click cancels a half-built highway before anything else.
    if pressed_r && g.highway_from.is_some() {
        g.highway_from = None;
        return;
    }

    let drawing = matches!(g.tool, Tool::Road | Tool::Erase);
    if pressed_l && !drawing {
        if let Some(t) = tile {
            tool_click(g, t);
        }
    }

    // Dragging roads (left with Road/Erase, right always erases)
    if (pressed_l && drawing) || pressed_r {
        g.last_tile = None;
        g.drag_erase = pressed_r || g.tool == Tool::Erase;
    }
    let dragging = (down_l && drawing) || down_r;
    if dragging && !over_ui {
        if let Some(t) = tile {
            let tiles = match g.last_tile {
                Some(l) if l != t => tiles_between(l, t),
                Some(_) => vec![],
                None => vec![t],
            };
            let mut placed = false;
            for p in tiles {
                let r = if g.drag_erase { g.sim.erase_road(p) } else { g.sim.place_road(p) };
                match r {
                    EditResult::Ok => placed = true,
                    EditResult::NoRoadsLeft => toast(g, "Out of road tiles. More arrive each week"),
                    EditResult::NeedBridge => toast(g, "Roads on water need a bridge upgrade"),
                    EditResult::NeedTunnel => toast(g, "Roads through hills need a tunnel upgrade"),
                    _ => {}
                }
                if matches!(r, EditResult::NoRoadsLeft | EditResult::NeedBridge | EditResult::NeedTunnel) {
                    g.flash = Some((p, 0.3));
                }
            }
            if placed {
                platform::play(Sfx::Road);
            }
            g.last_tile = Some(t);
        }
    } else {
        g.last_tile = None;
    }
}

fn toast(g: &mut Game, msg: &str) {
    if g.sim.toast.as_ref().map(|t| t.0.as_str()) != Some(msg) {
        platform::play(Sfx::Warning);
    }
    g.sim.toast = Some((msg.into(), 2.5));
}

/// A click with one of the junction or highway tools.
fn tool_click(g: &mut Game, t: Pos) {
    match g.tool {
        Tool::Roundabout | Tool::Light => {
            let kind = if g.tool == Tool::Roundabout { Junction::Roundabout } else { Junction::Light };
            match g.sim.place_junction(t, kind) {
                EditResult::Ok => platform::play(Sfx::Pop),
                EditResult::NoneLeft => g.select_tool(g.tool),
                EditResult::Blocked => {
                    toast(g, "Place it on a road tile");
                    g.flash = Some((t, 0.3));
                }
                _ => {}
            }
        }
        Tool::Highway => match g.highway_from {
            None => {
                if g.sim.inventory.highways == 0 {
                    g.select_tool(Tool::Highway);
                } else if g.sim.terrain[idx(t)] == Terrain::Grass && matches!(g.sim.cell(t), Cell::Empty | Cell::Road) {
                    g.highway_from = Some(t);
                    platform::play(Sfx::Click);
                } else {
                    toast(g, "Highways start and end on open land or road");
                    g.flash = Some((t, 0.3));
                }
            }
            Some(f) if f == t => g.highway_from = None,
            Some(f) => match g.sim.place_highway(f, t) {
                EditResult::Ok => {
                    platform::play(Sfx::Pop);
                    g.highway_from = None;
                }
                EditResult::NoneLeft => g.select_tool(Tool::Highway),
                _ => {
                    toast(g, "Highways span 2 to 14 tiles and end on open land or road");
                    g.flash = Some((t, 0.3));
                }
            },
        },
        _ => {}
    }
}

fn car_poses(s: &Sim) -> Vec<Option<(Vec2, Vec2)>> {
    s.cars.iter().map(|c| (c.state != little_lanes::sim::CarState::Idle).then(|| render3d::car_pose(c))).collect()
}

/// Car poses blended between the last two simulation steps, so cars glide at any frame rate.
fn smooth_poses(g: &Game) -> Vec<Option<(Vec2, Vec2)>> {
    let t = if g.speed == 0 || g.sim.offer.is_some() || g.sim.game_over { 1.0 } else { (g.acc / DT).clamp(0.0, 1.0) };
    car_poses(&g.sim)
        .into_iter()
        .enumerate()
        .map(|(i, cur)| match (g.prev_poses.get(i).copied().flatten(), cur) {
            // Only blend short moves; a jump means the car turned around or was re-routed.
            (Some((p0, d0)), Some((p1, d1))) if (p1 - p0).length() < 0.5 => {
                let d = d0.lerp(d1, t).normalize_or(d1);
                Some((p0.lerp(p1, t), d))
            }
            (_, cur) => cur,
        })
        .collect()
}

fn step_sim(g: &mut Game) {
    let dt = get_frame_time().min(0.1);
    g.acc += dt * g.speed as f32;
    let mut steps = 0;
    while g.acc >= DT && steps < 16 {
        g.prev_poses = car_poses(&g.sim);
        g.sim.step();
        g.acc -= DT;
        steps += 1;
    }
    // Toasts keep fading while the game is paused.
    if steps == 0 {
        if let Some((_, t)) = &mut g.sim.toast {
            *t -= dt;
            if *t <= 0.0 {
                g.sim.toast = None;
            }
        }
    }
    if let Some((_, t)) = &mut g.flash {
        *t -= dt;
        if *t <= 0.0 {
            g.flash = None;
        }
    }
    g.trip_cooldown -= dt;
    for e in std::mem::take(&mut g.sim.events) {
        match e {
            Event::TripDone => {
                if g.trip_cooldown <= 0.0 {
                    platform::play(Sfx::Trip);
                    g.trip_cooldown = 0.08;
                }
            }
            Event::NewHome | Event::NewDest => platform::play(Sfx::Pop),
            Event::Warning => platform::play(Sfx::Warning),
            Event::WeekEnd => platform::play(Sfx::Pick),
            Event::GameOver => {
                g.new_best = platform::record(g.map.index(), g.sim.score);
                platform::play(Sfx::GameOver);
            }
        }
    }
}

// ---- Screens and panels -------------------------------------------------------------

fn draw_top_button(b: &Btn, active: bool, hovered: bool) {
    let r = b.rect;
    let fill = if active {
        INK
    } else if hovered {
        Color::new(0.93, 0.92, 0.88, 1.0)
    } else {
        PAPER
    };
    let ink = if active { WHITE } else { INK };
    float_pill(r.x, r.y, r.w, r.h, fill);
    let c = r.center();
    match b.action {
        Action::Speed(0) => {
            draw_rectangle(c.x - 6.0, c.y - 7.0, 4.0, 14.0, ink);
            draw_rectangle(c.x + 2.0, c.y - 7.0, 4.0, 14.0, ink);
        }
        Action::Speed(n) => {
            for k in 0..n {
                let x = c.x - 4.0 * n as f32 + k as f32 * 9.0;
                draw_triangle(vec2(x - 3.0, c.y - 7.0), vec2(x - 3.0, c.y + 7.0), vec2(x + 7.0, c.y), ink);
            }
        }
        Action::Mute => {
            draw_rectangle(c.x - 9.0, c.y - 4.0, 5.0, 8.0, ink);
            draw_triangle(vec2(c.x - 5.0, c.y - 4.0), vec2(c.x - 5.0, c.y + 4.0), vec2(c.x + 2.0, c.y - 9.0), ink);
            draw_triangle(vec2(c.x - 5.0, c.y + 4.0), vec2(c.x + 2.0, c.y + 9.0), vec2(c.x + 2.0, c.y - 9.0), ink);
            if platform::muted() {
                draw_line(c.x + 5.0, c.y - 5.0, c.x + 11.0, c.y + 5.0, 2.0, ink);
                draw_line(c.x + 11.0, c.y - 5.0, c.x + 5.0, c.y + 5.0, 2.0, ink);
            } else {
                draw_arc(c.x + 2.0, c.y, 12, 6.0, -45.0, 2.0, 90.0, ink);
                draw_arc(c.x + 2.0, c.y, 12, 10.0, -45.0, 2.0, 90.0, ink);
            }
        }
        _ if r.w < 50.0 => {
            for k in -1..=1 {
                draw_rectangle(c.x - 8.0, c.y - 1.0 + k as f32 * 5.0, 16.0, 2.0, ink);
            }
        }
        _ => text_centered("Menu", c.x, c.y + 7.0, 20.0, ink),
    }
}

fn draw_hud(g: &Game, btns: &[Btn]) {
    let s = &g.sim;
    let sw = uw();
    // No solid bar: the counters float over the sky as small cards.

    let mut x = 12.0;
    // Trips: a little car icon
    let label = format!("{}", s.score);
    let w = 54.0 + measure(&label, 24.0).width;
    float_pill(x, 8.0, w, 30.0, PAPER);
    rrect(x + 10.0, 16.0, 26.0, 14.0, 5.0, PALETTE[0]);
    rrect(x + 18.0, 18.0, 10.0, 10.0, 2.0, Color::new(1.0, 1.0, 1.0, 0.7));
    text(&label, x + 44.0, 31.0, 24.0, INK);
    x += w + 10.0;
    // Week with progress ring
    let label = if narrow() { format!("{}", s.week) } else { format!("Week {}", s.week) };
    let w = 50.0 + measure(&label, 24.0).width;
    float_pill(x, 8.0, w, 30.0, PAPER);
    draw_circle_lines(x + 22.0, 23.0, 9.0, 3.0, Color::new(0.0, 0.0, 0.0, 0.12));
    draw_arc(x + 22.0, 23.0, 32, 7.5, -90.0, 3.0, 360.0 * s.week_progress(), INK);
    text(&label, x + 40.0, 31.0, 24.0, INK);
    x += w + 10.0;
    // Bridge and tunnel allowances, when the player has some and there is room.
    if sw > 760.0 {
        for (icon, n) in [(Icon::Bridge, s.inventory.bridge_tiles), (Icon::Tunnel, s.inventory.tunnel_tiles)] {
            if n == 0 {
                continue;
            }
            let label = format!("{}", n);
            let w = 50.0 + measure(&label, 24.0).width;
            float_pill(x, 8.0, w, 30.0, PAPER);
            draw_icon(icon, x + 22.0, 23.0, 0.62, false);
            text(&label, x + 40.0, 31.0, 24.0, INK);
            x += w + 10.0;
        }
    }

    for b in btns.iter().filter(|b| b.rect.y < HUD_H) {
        let active = match b.action {
            Action::Speed(n) => g.speed == n,
            _ => false,
        };
        draw_top_button(b, active, b.rect.contains(Vec2::from(mouse_position()) / ui_scale()));
    }
}

fn draw_toolbar(g: &Game, btns: &[Btn], m: Vec2) {
    let bar = bar_rect();
    card(bar, PAPER);
    for b in btns {
        let Action::Tool(t) = b.action else { continue };
        let r = b.rect;
        let active = g.tool == t;
        let hovered = r.contains(m);
        if active {
            rrect(r.x, r.y, r.w, r.h, 14.0, if t == Tool::Erase { with_alpha(WARN, 0.18) } else { Color::new(0.0, 0.0, 0.0, 0.10) });
            rrect(r.x + 18.0, r.y + r.h - 5.0, r.w - 36.0, 3.0, 1.5, if t == Tool::Erase { WARN } else { INK });
        } else if hovered {
            rrect(r.x, r.y, r.w, r.h, 14.0, CHIP);
        }
        let count = t.count(&g.sim);
        let dim = count == Some(0) && t != Tool::Road;
        draw_icon(t.icon(), r.center().x, r.center().y - 2.0, 1.0, dim);
        if let Some(n) = count {
            let label = format!("{}", n);
            let w = (measure(&label, 16.0).width + 12.0).max(22.0);
            let low = t == Tool::Road && n < 5;
            let fill = if low { WARN } else if n == 0 { Color::new(0.75, 0.75, 0.75, 1.0) } else { INK };
            pill(r.x + r.w - w + 2.0, r.y - 6.0, w, 20.0, fill);
            text_centered(&label, r.x + r.w - w / 2.0 + 2.0, r.y + 9.0, 16.0, WHITE);
        }
        if hovered {
            let label = t.label();
            let w = measure(label, 18.0).width + 24.0;
            let x = (r.center().x - w / 2.0).clamp(8.0, uw() - w - 8.0);
            pill(x, bar.y - 40.0, w, 30.0, INK);
            text_centered(label, x + w / 2.0, bar.y - 19.0, 18.0, WHITE);
        }
    }
    if let Some(f) = g.highway_from {
        let _ = f;
        let label = "Click where the highway ends. Right click cancels";
        let w = measure(label, 18.0).width + 28.0;
        pill((uw() - w) / 2.0, bar.y - 44.0, w, 32.0, Color::new(0.30, 0.26, 0.12, 1.0));
        text_centered(label, uw() / 2.0, bar.y - 22.0, 18.0, WHITE);
    }
}

fn draw_toast(g: &Game) {
    let Some((msg, t)) = &g.sim.toast else { return };
    let a = t.min(1.0);
    let m = measure(msg, toast_size());
    let y = if g.screen == Screen::Play && !g.sim.game_over && g.sim.offer.is_none() { bar_rect().y - 54.0 } else { uh() - 60.0 };
    let w = m.width + 40.0;
    let x = (uw() - w) / 2.0;
    // Opaque fill blended toward the sky colour so pill ends never overlap visibly.
    let sky = render3d::SKY;
    let fill = Color::new(INK.r + (sky.r - INK.r) * (1.0 - a), INK.g + (sky.g - INK.g) * (1.0 - a), INK.b + (sky.b - INK.b) * (1.0 - a), 1.0);
    if g.highway_from.is_none() {
        pill(x, y, w, 38.0, fill);
        text_centered(msg, uw() / 2.0, y + 25.0, toast_size(), Color::new(1.0, 1.0, 1.0, a));
    }
}

fn draw_offer(g: &Game, btns: &[Btn], m: Vec2) {
    let Some(offer) = g.sim.offer else { return };
    let (sw, sh) = (uw(), uh());
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.95, 0.94, 0.90, 0.55));
    let cards: Vec<&Btn> = btns.iter().filter(|b| matches!(b.action, Action::Choose(_))).collect();
    let top = cards.iter().map(|b| b.rect.y).fold(f32::MAX, f32::min);
    text_centered(&format!("Week {} begins", g.sim.week), sw / 2.0, top - 58.0, 34.0, INK);
    text_centered("Pick one upgrade", sw / 2.0, top - 24.0, 22.0, with_alpha(INK, 0.7));
    for b in cards {
        let Action::Choose(i) = b.action else { continue };
        let u = offer[i];
        let mut r = b.rect;
        let hovered = r.contains(m);
        if hovered {
            r.y -= 4.0;
        }
        card(r, PAPER);
        if hovered {
            rrect(r.x + 16.0, r.y, r.w - 32.0, 5.0, 2.5, GOLD);
        }
        let tall = r.h > 200.0;
        let (ix, iy) = if tall { (r.center().x, r.y + 70.0) } else { (r.x + 60.0, r.center().y) };
        draw_circle(ix, iy, 44.0, Color::new(0.0, 0.0, 0.0, 0.05));
        draw_icon(upgrade_icon(u), ix, iy, 1.9, false);
        let (tx, tw, ty) = if tall { (r.center().x, r.w - 36.0, r.y + 150.0) } else { (r.x + 120.0 + (r.w - 140.0) / 2.0, r.w - 140.0, r.y + 52.0) };
        text_centered(u.name(), tx, ty, 26.0, INK);
        for (k, line) in wrap(u.blurb(), 18.0, tw).iter().enumerate() {
            text_centered(line, tx, ty + 30.0 + k as f32 * 22.0, 18.0, with_alpha(INK, 0.7));
        }
        if tall {
            text_centered(&format!("Press {}", i + 1), r.center().x, r.y + r.h - 16.0, 15.0, with_alpha(INK, 0.4));
        }
    }
}

fn draw_game_over(g: &Game, btns: &[Btn], m: Vec2) {
    let (sw, sh) = (uw(), uh());
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.95, 0.94, 0.9, 0.6));
    let (cx, cy) = (sw / 2.0, sh / 2.0);
    let cw = (uw() - 24.0).min(400.0);
    card(Rect::new(cx - cw / 2.0, cy - 130.0, cw, 250.0), PAPER);
    text_centered("Gridlock!", cx, cy - 76.0, 40.0, INK);
    let weeks = if g.sim.week == 1 { "1 week".to_string() } else { format!("{} weeks", g.sim.week) };
    text_centered(&format!("{} trips in {} on {}", g.sim.score, weeks, g.map.name()), cx, cy - 34.0, 22.0, INK);
    if g.new_best {
        pill(cx - 90.0, cy - 16.0, 180.0, 32.0, GOLD);
        text_centered("New best score!", cx, cy + 6.0, 20.0, INK);
    } else {
        text_centered(&format!("Best: {}", platform::best(g.map.index())), cx, cy + 6.0, 20.0, with_alpha(INK, 0.6));
    }
    for b in btns {
        let r = b.rect;
        let primary = b.action == Action::Restart;
        let hovered = r.contains(m);
        let fill = if primary { if hovered { Color::new(0.3, 0.3, 0.36, 1.0) } else { INK } } else if hovered { Color::new(0.0, 0.0, 0.0, 0.12) } else { CHIP };
        pill(r.x, r.y, r.w, r.h, fill);
        text_centered(if primary { "Play again" } else { "Maps" }, r.center().x, r.y + 30.0, 22.0, if primary { WHITE } else { INK });
    }
}

fn draw_title(_g: &Game, btns: &[Btn], m: Vec2) {
    let (sw, sh) = (uw(), uh());
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.97, 0.96, 0.92, 0.45));
    let wide = sw >= 820.0;
    let ty = if wide { sh * 0.22 } else { sh * 0.14 };
    let big = if wide { 76.0 } else { 52.0 };
    text_centered("Little Lanes", sw / 2.0 + 2.0, ty + 3.0, big, Color::new(0.0, 0.0, 0.0, 0.12));
    text_centered("Little Lanes", sw / 2.0, ty, big, INK);
    text_centered("Draw roads. Keep the town moving.", sw / 2.0, ty + 44.0, if wide { 24.0 } else { 18.0 }, with_alpha(INK, 0.75));

    for b in btns {
        let r = b.rect;
        let hovered = r.contains(m);
        match b.action {
            Action::Start(i) => {
                let k = MapKind::ALL[i];
                let mut r = r;
                if hovered {
                    r.y -= 4.0;
                }
                card(r, PAPER);
                let accent = PALETTE[[4, 1, 2][i]];
                rrect(r.x + 16.0, r.y, r.w - 32.0, 5.0, 2.5, accent);
                draw_map_swatch(k, r.x + 18.0, r.y + 24.0, if wide { r.w - 36.0 } else { 90.0 }, if wide { 60.0 } else { 60.0 });
                let (tx, ty, tw) = if wide { (r.x + 18.0, r.y + 112.0, r.w - 36.0) } else { (r.x + 124.0, r.y + 44.0, r.w - 140.0) };
                text(k.name(), tx, ty, 24.0, INK);
                for (n, line) in wrap(k.blurb(), 16.0, tw).iter().take(2).enumerate() {
                    text(line, tx, ty + 22.0 + n as f32 * 19.0, 16.0, with_alpha(INK, 0.65));
                }
                let best = platform::best(i);
                let label = if best > 0 { format!("Best {}", best) } else { "New".to_string() };
                let w = measure(&label, 15.0).width + 18.0;
                pill(r.x + r.w - w - 12.0, r.y + 18.0, w, 24.0, if best > 0 { GOLD } else { CHIP });
                text(&label, r.x + r.w - w - 3.0, r.y + 35.0, 15.0, INK);
            }
            Action::Resume => {
                pill(r.x, r.y, r.w, r.h, if hovered { Color::new(0.3, 0.3, 0.36, 1.0) } else { INK });
                text_centered("Resume game", r.center().x, r.y + 30.0, 22.0, WHITE);
            }
            _ => draw_top_button(b, false, hovered),
        }
    }
    let hint = if narrow() { "Drag to build roads. Pinch to zoom." } else { "Drag to build roads. Right drag erases. Scroll to zoom." };
    text_centered(hint, sw / 2.0, sh - 24.0, if narrow() { 15.0 } else { 18.0 }, with_alpha(INK, 0.6));
}

/// A tiny top-down picture of a map's terrain for the title cards.
fn draw_map_swatch(k: MapKind, x: f32, y: f32, w: f32, h: f32) {
    thread_local! {
        static CACHE: RefCell<Vec<Vec<Terrain>>> = const { RefCell::new(Vec::new()) };
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.is_empty() {
            for kind in MapKind::ALL {
                c.push(little_lanes::map::generate_kind(kind, 7));
            }
        }
        let t = &c[k.index()];
        let (cw, ch) = (w / W as f32, h / H as f32);
        rrect(x, y, w, h, 8.0, Color::new(0.72, 0.84, 0.56, 1.0));
        for yy in 0..H {
            for xx in 0..W {
                let col = match t[idx((xx, yy))] {
                    Terrain::Grass => continue,
                    Terrain::Water => Color::new(0.45, 0.70, 0.86, 1.0),
                    Terrain::Hill => Color::new(0.56, 0.66, 0.44, 1.0),
                };
                draw_circle(x + (xx as f32 + 0.5) * cw, y + (yy as f32 + 0.5) * ch, cw * 0.85, col);
            }
        }
    });
}

fn toast_size() -> f32 {
    if narrow() {
        15.0
    } else {
        22.0
    }
}
