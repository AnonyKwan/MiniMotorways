use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::map::{self, idx, in_bounds, MapKind, Pos, Terrain, DIRS, H, W};
use crate::rng::Rng;

/// Simulation step length in seconds.
pub const DT: f32 = 1.0 / 60.0;
/// Car speed in tiles per second.
pub const CAR_SPEED: f32 = 2.6;
/// Seconds a car spends at a destination before heading home.
pub const UNLOAD_TIME: f32 = 0.6;
/// Pins a small shop can hold before its overflow timer starts (big shops hold half again).
pub const PIN_LIMIT: u32 = 6;
/// Seconds of continuous overflow before the game ends.
pub const OVERFLOW_TIME: f32 = 45.0;
pub const WEEK_LENGTH: f32 = 90.0;
pub const START_ROADS: u32 = 36;
pub const ROADS_PER_WEEK: u32 = 16;
pub const MAX_COLORS: usize = 6;

// ----- Difficulty curve ---------------------------------------------------------------
// Pressure climbs for the first eleven weeks, then levels off. After that, variety comes
// from new colours, scheduled shops and rush waves rather than an ever faster baseline.

/// Average seconds between pins at a small shop (big shops are 1.6x busier).
pub fn pin_interval(week: u32) -> f32 {
    match week {
        0..=15 => (11.0 - 0.7 * (week.max(1) - 1) as f32).max(4.0),
        16..=23 => 3.5,
        _ => 3.0,
    }
}

/// Multiplier on how fast new homes arrive. Capped at week 11.
pub fn pace(week: u32) -> f32 {
    (1.0 + 0.1 * (week.max(1) - 1) as f32).min(2.0)
}

/// Colours in play during a week.
pub fn colors_for_week(week: u32) -> usize {
    match week {
        0..=2 => 2,
        3..=4 => 3,
        5..=7 => 4,
        8..=11 => 5,
        _ => 6,
    }
    .min(MAX_COLORS)
}

/// How many shops the town should have by the end of a week.
pub fn target_dests(week: u32) -> usize {
    let w = week.max(1) as usize;
    match w {
        0..=12 => w,
        13..=16 => w.min(15),
        17..=24 => 15 + (w - 16) * 3 / 8,
        _ => (18 + (w - 24) / 2).min(20),
    }
}

/// Chance that a new shop is a big one.
pub fn big_shop_chance(week: u32) -> f32 {
    match week {
        0..=3 => 0.0,
        4..=6 => 0.2,
        7..=11 => 0.3,
        _ => 0.4,
    }
}

/// Rush waves start in week 9 and come every four weeks.
pub fn rush_week(week: u32) -> bool {
    week >= 9 && (week - 9) % 4 == 0
}
/// When a rush is announced, and how long it lasts, in seconds into its week.
pub const RUSH_WARN_AT: f32 = 30.0;
pub const RUSH_LEAD: f32 = 10.0;
pub const RUSH_LENGTH: f32 = 20.0;
/// Pin interval multiplier for the rushing colour.
pub const RUSH_FACTOR: f32 = 0.6;

/// Names for the destination colours, in palette order.
pub const COLOR_NAMES: [&str; MAX_COLORS] = ["coral", "teal", "yellow", "violet", "blue", "pink"];

/// Upgrade offers come every week until week 12, then every other week.
pub fn offer_week(week: u32) -> bool {
    week <= 12 || (week - 13) % 2 == 0
}

/// Footprint of a small shop (one driveway) and a big shop (two driveways).
pub const SMALL_SHOP: (i32, i32) = (3, 2);
/// A big shop is two small shops side by side, each with its own driveway.
pub const BIG_SHOP: (i32, i32) = (6, 2);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    Empty,
    Road,
    Home(usize),
    Dest(usize),
    Door(usize),
    /// Car park in front of a shop, between or beside its driveways. On a big shop cars can
    /// drive through it from one driveway to the other.
    Park(usize),
}

#[derive(Clone, Debug)]
pub struct Home {
    pub pos: Pos,
    pub color: usize,
    /// Sim time when it appeared, for the arrival animation.
    pub born: f32,
    /// Index into DIRS of the side the driveway is on. Cars can only use that side.
    pub facing: usize,
}

#[derive(Clone, Debug)]
pub struct Dest {
    /// Top-left tile of the building.
    pub origin: Pos,
    /// Building size in tiles (x, y), already turned to the way the shop faces.
    pub size: (i32, i32),
    /// Index into DIRS of the way the shop front faces.
    pub facing: usize,
    /// The car-park tile at the front-left of the shop, seen from the front (u = 0, v = 0).
    pub anchor: Pos,
    /// Driveway tiles, one tile out from the car park: one for a small shop, two for a big
    /// one. A road may join a driveway from its front or either side.
    pub doors: Vec<Pos>,
    /// Sim time when it appeared, for the arrival animation.
    pub born: f32,
    pub color: usize,
    /// Trips wanted, including ones a car is already on its way to serve.
    pub pins: u32,
    /// Pins that already have a car assigned.
    pub assigned: u32,
    /// 0..1, game over at 1.
    pub overflow: f32,
    pub pin_timer: f32,
}

/// DIRS index for the main direction of travel from a to b (works for highway hops too).
pub fn dir_between(a: Pos, b: Pos) -> usize {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if dx.abs() >= dy.abs() {
        if dx >= 0 { 0 } else { 2 }
    } else if dy >= 0 {
        1
    } else {
        3
    }
}

/// A tile of a shop in its own frame: `u` runs along the front, `v` outward from the car
/// park row (v = 0), so the building is at v = -2..=-1 and driveways at v = 1.
pub fn shop_tile(anchor: Pos, facing: usize, u: i32, v: i32) -> Pos {
    let f = DIRS[facing];
    let a = (f.1, -f.0);
    (anchor.0 + u * a.0 + v * f.0, anchor.1 + u * a.1 + v * f.1)
}

/// Driveway positions along the front (u) for a shop of width `w`.
fn door_us(big: bool) -> &'static [i32] {
    if big { &[1, 4] } else { &[1] }
}

impl Dest {
    pub fn big(&self) -> bool {
        self.doors.len() > 1
    }
    /// Width of the shop front in tiles.
    pub fn width(&self) -> i32 {
        if self.big() { BIG_SHOP.0 } else { SMALL_SHOP.0 }
    }
    pub fn tile(&self, u: i32, v: i32) -> Pos {
        shop_tile(self.anchor, self.facing, u, v)
    }
    /// The tile straight out in front of a driveway.
    pub fn entry(&self, door: Pos) -> Pos {
        let f = DIRS[self.facing];
        (door.0 + f.0, door.1 + f.1)
    }
    /// Bounding box (min, max inclusive) of the building, car park and driveways.
    pub fn bounds(&self) -> (Pos, Pos) {
        let mut lo = (i32::MAX, i32::MAX);
        let mut hi = (i32::MIN, i32::MIN);
        for p in [self.tile(0, -2), self.tile(self.width() - 1, -2), self.tile(0, 1), self.tile(self.width() - 1, 1)] {
            lo = (lo.0.min(p.0), lo.1.min(p.1));
            hi = (hi.0.max(p.0), hi.1.max(p.1));
        }
        (lo, hi)
    }
    /// Pins a destination can hold before its overflow timer starts.
    pub fn pin_limit(&self) -> u32 {
        if self.big() {
            PIN_LIMIT * 3 / 2
        } else {
            PIN_LIMIT
        }
    }
}

/// Things the player can pick at the end of each week.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Upgrade {
    Bridge,
    Tunnel,
    Roundabout,
    TrafficLight,
    Highway,
    ExtraRoads,
}

impl Upgrade {
    pub fn name(self) -> &'static str {
        match self {
            Upgrade::Bridge => "Bridge",
            Upgrade::Tunnel => "Tunnel",
            Upgrade::Roundabout => "Roundabout",
            Upgrade::TrafficLight => "Traffic light",
            Upgrade::Highway => "Highway",
            Upgrade::ExtraRoads => "Road tiles",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Upgrade::Bridge => "Roads across 4 more water tiles",
            Upgrade::Tunnel => "Roads through 4 more hill tiles",
            Upgrade::Roundabout => "Two junctions where traffic keeps flowing",
            Upgrade::TrafficLight => "Three lights; facing roads go together",
            Upgrade::Highway => "A fast link over anything",
            Upgrade::ExtraRoads => "A big stack of road tiles",
        }
    }
    /// Road tiles that come with the card. Better tools bring fewer.
    pub fn bonus_roads(self) -> u32 {
        match self {
            Upgrade::ExtraRoads => 30,
            Upgrade::Roundabout | Upgrade::TrafficLight => 14,
            Upgrade::Bridge | Upgrade::Tunnel => 10,
            Upgrade::Highway => 6,
        }
    }
    fn weight(self) -> u32 {
        if self == Upgrade::ExtraRoads { 3 } else { 2 }
    }
    /// First week (the week that is starting) in which the card can be offered.
    fn unlock_week(self) -> u32 {
        match self {
            Upgrade::Highway => 3,
            _ => 2,
        }
    }
}

/// A short burst of demand for one colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rush {
    pub color: usize,
    /// Sim times when the rush starts and ends (it is announced before `start`).
    pub start: f32,
    pub end: f32,
    announced: bool,
}

impl Rush {
    pub fn active(&self, time: f32) -> bool {
        time >= self.start && time < self.end
    }
}

/// Upgrades the player has but hasn't used yet.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Inventory {
    /// Water tiles that can still take a road.
    pub bridge_tiles: u32,
    /// Hill tiles that can still take a road.
    pub tunnel_tiles: u32,
    pub roundabouts: u32,
    pub lights: u32,
    pub highways: u32,
}

/// How a junction tile manages traffic.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Junction {
    /// Default: only one car in the crossing at a time.
    Plain,
    /// Each direction flows independently.
    Roundabout,
    /// Alternates between east-west and north-south traffic.
    Light,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Highway {
    pub a: Pos,
    pub b: Pos,
}

impl Highway {
    pub fn len(&self) -> f32 {
        (((self.a.0 - self.b.0).pow(2) + (self.a.1 - self.b.1).pow(2)) as f32).sqrt()
    }
}

pub const BRIDGE_TILES: u32 = 4;
pub const TUNNEL_TILES: u32 = 4;
pub const HIGHWAY_MAX_LEN: i32 = 14;
pub const HIGHWAY_SPEED: f32 = 2.2;
/// Seconds each traffic light phase lasts.
pub const LIGHT_PHASE: f32 = 3.0;
/// All-red gap between phases, so the crossing clears before the other way goes.
pub const LIGHT_CLEAR: f32 = 0.6;
/// A car stuck this long at a plain crossing goes anyway, so loops of crossings can't
/// lock up for good. Plain crossings stay slow; they just can't freeze the town.
pub const JAM_BREAK: f32 = 4.0;

/// Things that happened during a step, for sounds and effects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    TripDone,
    NewHome,
    NewDest,
    /// A destination just started overflowing.
    Warning,
    WeekEnd,
    /// A rush wave has been announced.
    RushSoon,
    GameOver,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CarState {
    Idle,
    ToDest(usize),
    Unloading(usize),
    ToHome,
}

/// One car per home; `cars[i]` belongs to `homes[i]`.
#[derive(Clone, Debug)]
pub struct Car {
    pub state: CarState,
    pub path: Vec<Pos>,
    /// Index of `cur` in `path`.
    pub i: usize,
    /// 0..1 progress moving from `prev` into `cur`.
    pub progress: f32,
    pub prev: Pos,
    pub cur: Pos,
    /// Lane slot held on `cur` (road tiles only).
    pub slot: Option<usize>,
    /// Retry countdown when the car has no route.
    pub retry: f32,
    /// Seconds spent waiting to enter a plain crossing.
    pub wait: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditResult {
    Ok,
    NoChange,
    NoRoadsLeft,
    NeedBridge,
    NeedTunnel,
    NoneLeft,
    Blocked,
}

pub struct Sim {
    pub map: MapKind,
    pub terrain: Vec<Terrain>,
    /// Junction control per tile (only meaningful on road tiles with 3+ links).
    pub junctions: Vec<Junction>,
    pub highways: Vec<Highway>,
    pub inventory: Inventory,
    /// Two upgrades offered at the end of a week; the sim waits until one is picked.
    pub offer: Option<[Upgrade; 2]>,
    /// Events since the caller last drained them.
    pub events: Vec<Event>,
    pub cells: Vec<Cell>,
    /// Lane occupancy: one flag per tile per direction of travel.
    pub occ: Vec<bool>,
    pub homes: Vec<Home>,
    pub cars: Vec<Car>,
    pub dests: Vec<Dest>,
    pub roads_left: u32,
    pub score: u32,
    pub time: f32,
    pub week: u32,
    pub game_over: bool,
    /// Destination whose overflow ended the game.
    pub failed_dest: Option<usize>,
    /// Short message to show the player and how long it has left.
    pub toast: Option<(String, f32)>,
    /// The current or next rush wave, if one is scheduled this week.
    pub rush: Option<Rush>,
    /// Last upgrade picked, so the next offer doesn't repeat it.
    last_pick: Option<Upgrade>,
    /// Offers since a highway was last on the table.
    since_highway: u32,
    rng: Rng,
    home_timer: f32,
    dest_timer: f32,
    dispatch_timer: f32,
    roads_dirty: bool,
}

impl Sim {
    pub fn new(seed: u64) -> Self {
        Sim::new_on(MapKind::Bayside, seed)
    }

    pub fn new_on(map: MapKind, seed: u64) -> Self {
        let terrain = map::generate_kind(map, seed);
        let inventory = match map {
            MapKind::Bayside => Inventory { bridge_tiles: BRIDGE_TILES, ..Default::default() },
            MapKind::TwinIslands => Inventory { bridge_tiles: BRIDGE_TILES * 2, ..Default::default() },
            MapKind::Highland => Inventory { tunnel_tiles: TUNNEL_TILES, ..Default::default() },
        };
        let mut s = Sim {
            map,
            junctions: vec![Junction::Plain; (W * H) as usize],
            highways: Vec::new(),
            inventory,
            offer: None,
            events: Vec::new(),
            terrain,
            cells: vec![Cell::Empty; (W * H) as usize],
            occ: vec![false; (W * H * 4) as usize],
            homes: Vec::new(),
            cars: Vec::new(),
            dests: Vec::new(),
            roads_left: START_ROADS,
            score: 0,
            time: 0.0,
            week: 1,
            game_over: false,
            failed_dest: None,
            toast: Some(("Draw roads from homes to matching destinations".into(), 6.0)),
            rush: None,
            last_pick: None,
            since_highway: 0,
            rng: Rng::new(seed ^ 0xA5A5),
            home_timer: 12.0,
            dest_timer: 0.0,
            dispatch_timer: 0.0,
            roads_dirty: false,
        };
        // Opening layout: one destination near the middle with two homes.
        let center = (W / 2 - 6, H / 2);
        if let Some(d) = s.spawn_dest_near(0, center) {
            s.spawn_home(d);
            s.spawn_home(d);
        }
        s
    }

    /// An empty sim with all-grass terrain and nothing spawned, for tests.
    pub fn empty(seed: u64) -> Self {
        let mut s = Sim::new(seed);
        s.terrain = vec![Terrain::Grass; (W * H) as usize];
        s.cells = vec![Cell::Empty; (W * H) as usize];
        s.homes.clear();
        s.cars.clear();
        s.dests.clear();
        s.inventory = Inventory::default();
        s.home_timer = f32::INFINITY;
        s.dest_timer = f32::INFINITY;
        s
    }

    pub fn cell(&self, p: Pos) -> Cell {
        if in_bounds(p) {
            self.cells[idx(p)]
        } else {
            Cell::Dest(usize::MAX)
        }
    }

    pub fn colors_available(&self) -> usize {
        colors_for_week(self.week)
    }

    // ----- Player edits -------------------------------------------------

    /// What `place_road` would do at `p`, without doing it (for the placement preview).
    pub fn road_check(&self, p: Pos) -> EditResult {
        if !in_bounds(p) || self.game_over {
            return EditResult::Blocked;
        }
        match self.cells[idx(p)] {
            Cell::Road => return EditResult::NoChange,
            Cell::Empty => {}
            _ => return EditResult::Blocked,
        }
        match self.terrain[idx(p)] {
            Terrain::Grass if self.roads_left == 0 => EditResult::NoRoadsLeft,
            Terrain::Water if self.inventory.bridge_tiles == 0 => EditResult::NeedBridge,
            Terrain::Hill if self.inventory.tunnel_tiles == 0 => EditResult::NeedTunnel,
            _ => EditResult::Ok,
        }
    }

    /// Whether a junction of `kind` could go on `p` (ignoring whether the player has one).
    pub fn junction_fits(&self, p: Pos, kind: Junction) -> bool {
        if !in_bounds(p) || self.cells[idx(p)] != Cell::Road {
            return false;
        }
        kind != Junction::Roundabout
            || ![(-1, -1), (1, -1), (-1, 1), (1, 1)]
                .iter()
                .any(|d| matches!(self.cell((p.0 + d.0, p.1 + d.1)), Cell::Home(_) | Cell::Dest(_) | Cell::Door(_) | Cell::Park(_)))
    }

    pub fn place_road(&mut self, p: Pos) -> EditResult {
        if !in_bounds(p) || self.game_over {
            return EditResult::Blocked;
        }
        match self.cells[idx(p)] {
            Cell::Road => return EditResult::NoChange,
            Cell::Empty => {}
            _ => return EditResult::Blocked,
        }
        match self.terrain[idx(p)] {
            Terrain::Grass => {
                if self.roads_left == 0 {
                    return EditResult::NoRoadsLeft;
                }
                self.roads_left -= 1;
            }
            Terrain::Water => {
                if self.inventory.bridge_tiles == 0 {
                    return EditResult::NeedBridge;
                }
                self.inventory.bridge_tiles -= 1;
            }
            Terrain::Hill => {
                if self.inventory.tunnel_tiles == 0 {
                    return EditResult::NeedTunnel;
                }
                self.inventory.tunnel_tiles -= 1;
            }
        }
        self.cells[idx(p)] = Cell::Road;
        self.roads_dirty = true;
        self.update_facings_near(p);
        EditResult::Ok
    }

    pub fn erase_road(&mut self, p: Pos) -> EditResult {
        if !in_bounds(p) || self.game_over {
            return EditResult::Blocked;
        }
        if self.cells[idx(p)] != Cell::Road {
            return EditResult::NoChange;
        }
        if self.tile_occupancy(p) > 0 {
            return EditResult::Blocked;
        }
        // Removing a highway end removes that highway and gives it back.
        let before = self.highways.len();
        self.highways.retain(|h| h.a != p && h.b != p);
        self.inventory.highways += (before - self.highways.len()) as u32;
        match self.junctions[idx(p)] {
            Junction::Roundabout => self.inventory.roundabouts += 1,
            Junction::Light => self.inventory.lights += 1,
            Junction::Plain => {}
        }
        self.junctions[idx(p)] = Junction::Plain;
        self.cells[idx(p)] = Cell::Empty;
        match self.terrain[idx(p)] {
            Terrain::Grass => self.roads_left += 1,
            Terrain::Water => self.inventory.bridge_tiles += 1,
            Terrain::Hill => self.inventory.tunnel_tiles += 1,
        }
        self.roads_dirty = true;
        self.update_facings_near(p);
        EditResult::Ok
    }

    /// Number of neighbours this road tile actually links to (roads, driveways, highways).
    pub fn link_count(&self, p: Pos) -> usize {
        let mut n = DIRS
            .iter()
            .filter(|d| {
                let q = (p.0 + d.0, p.1 + d.1);
                in_bounds(q)
                    && match self.cells[idx(q)] {
                        Cell::Road => true,
                        // A shop driveway joins a road on its front or either side (its back is
                        // the car park, never a road).
                        Cell::Door(_) => true,
                        Cell::Home(h) => self.driveway(h) == p,
                        _ => false,
                    }
            })
            .count();
        n += self.highways.iter().filter(|h| h.a == p || h.b == p).count();
        n
    }

    pub fn is_crossing(&self, p: Pos) -> bool {
        in_bounds(p) && self.cells[idx(p)] == Cell::Road && self.link_count(p) >= 3
    }

    /// Whether a tile is one of the eight around a roundabout, which its ring spills into.
    pub fn near_roundabout(&self, p: Pos) -> bool {
        (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                let q = (p.0 + dx, p.1 + dy);
                (dx, dy) != (0, 0) && in_bounds(q) && self.cells[idx(q)] == Cell::Road && self.junctions[idx(q)] == Junction::Roundabout
            })
        })
    }

    /// Put a roundabout or traffic light on a road tile.
    pub fn place_junction(&mut self, p: Pos, kind: Junction) -> EditResult {
        if !in_bounds(p) || self.game_over || self.cells[idx(p)] != Cell::Road || kind == Junction::Plain {
            return EditResult::Blocked;
        }
        // A roundabout's ring needs the corners around it clear of buildings.
        if !self.junction_fits(p, kind) {
            return EditResult::Blocked;
        }
        if self.junctions[idx(p)] == kind {
            return EditResult::NoChange;
        }
        let have = match kind {
            Junction::Roundabout => &mut self.inventory.roundabouts,
            _ => &mut self.inventory.lights,
        };
        if *have == 0 {
            return EditResult::NoneLeft;
        }
        *have -= 1;
        match self.junctions[idx(p)] {
            Junction::Roundabout => self.inventory.roundabouts += 1,
            Junction::Light => self.inventory.lights += 1,
            Junction::Plain => {}
        }
        self.junctions[idx(p)] = kind;
        EditResult::Ok
    }

    /// Whether a highway may run between a and b.
    pub fn highway_ok(&self, a: Pos, b: Pos) -> bool {
        let ok_end = |p: Pos| {
            in_bounds(p)
                && self.terrain[idx(p)] == Terrain::Grass
                && matches!(self.cells[idx(p)], Cell::Empty | Cell::Road)
        };
        let d = (a.0 - b.0).abs().max((a.1 - b.1).abs());
        a != b && d >= 2 && d <= HIGHWAY_MAX_LEN && ok_end(a) && ok_end(b)
            && !self.highways.iter().any(|h| (h.a == a && h.b == b) || (h.a == b && h.b == a))
    }

    /// Build an elevated highway between two tiles. Its ends become free road tiles.
    pub fn place_highway(&mut self, a: Pos, b: Pos) -> EditResult {
        if self.game_over || !self.highway_ok(a, b) {
            return EditResult::Blocked;
        }
        if self.inventory.highways == 0 {
            return EditResult::NoneLeft;
        }
        self.inventory.highways -= 1;
        for p in [a, b] {
            if self.cells[idx(p)] == Cell::Empty {
                self.cells[idx(p)] = Cell::Road;
                self.update_facings_near(p);
            }
        }
        self.highways.push(Highway { a, b });
        self.roads_dirty = true;
        EditResult::Ok
    }

    /// Choose one of the two offered upgrades.
    pub fn choose(&mut self, i: usize) {
        let Some(offer) = self.offer.take() else { return };
        let u = offer[i.min(1)];
        let inv = &mut self.inventory;
        let what = match u {
            Upgrade::Bridge => {
                inv.bridge_tiles += BRIDGE_TILES;
                format!("+{} bridge tiles", BRIDGE_TILES)
            }
            Upgrade::Tunnel => {
                inv.tunnel_tiles += TUNNEL_TILES;
                format!("+{} tunnel tiles", TUNNEL_TILES)
            }
            Upgrade::Roundabout => {
                inv.roundabouts += 2;
                "+2 roundabouts".to_string()
            }
            Upgrade::TrafficLight => {
                inv.lights += 3;
                "+3 traffic lights".to_string()
            }
            Upgrade::Highway => {
                inv.highways += 1;
                "+1 highway".to_string()
            }
            Upgrade::ExtraRoads => String::new(),
        };
        self.roads_left += u.bonus_roads();
        self.last_pick = Some(u);
        let roads = format!("+{} road tiles", u.bonus_roads());
        let msg = if what.is_empty() { roads } else { format!("{} and {}", what, roads) };
        self.toast = Some((msg, 3.0));
    }

    /// Two different cards, weighted, following these rules: last week's pick isn't offered
    /// again, cards unlock by week, bridges and tunnels only where the map has water or hills,
    /// and a highway is guaranteed if none has been offered for four offers.
    fn make_offer(&mut self) -> [Upgrade; 2] {
        let has = |t: Terrain| self.terrain.iter().any(|x| *x == t);
        let week = self.week;
        let mut pool: Vec<Upgrade> = [
            Upgrade::ExtraRoads,
            Upgrade::Roundabout,
            Upgrade::TrafficLight,
            Upgrade::Highway,
            Upgrade::Bridge,
            Upgrade::Tunnel,
        ]
        .into_iter()
        .filter(|u| week >= u.unlock_week())
        .filter(|u| match u {
            Upgrade::Bridge => has(Terrain::Water),
            Upgrade::Tunnel => has(Terrain::Hill),
            _ => true,
        })
        .filter(|u| Some(*u) != self.last_pick)
        .collect();
        let mut out = Vec::new();
        if week >= Upgrade::Highway.unlock_week() && self.since_highway >= 4 && pool.contains(&Upgrade::Highway) {
            out.push(Upgrade::Highway);
            pool.retain(|u| *u != Upgrade::Highway);
        }
        while out.len() < 2 && !pool.is_empty() {
            let total: u32 = pool.iter().map(|u| u.weight()).sum();
            let mut r = self.rng.below(total as usize) as u32;
            let mut k = 0;
            while r >= pool[k].weight() {
                r -= pool[k].weight();
                k += 1;
            }
            out.push(pool.swap_remove(k));
        }
        while out.len() < 2 {
            // Only possible on a tiny pool early on; fall back to something useful.
            out.push(if out.contains(&Upgrade::ExtraRoads) { Upgrade::Roundabout } else { Upgrade::ExtraRoads });
        }
        if out.contains(&Upgrade::Highway) {
            self.since_highway = 0;
        } else if week >= Upgrade::Highway.unlock_week() {
            self.since_highway += 1;
        }
        [out[0], out[1]]
    }

    /// Which axis a traffic light at `p` lets through right now: 0 = east-west, 1 =
    /// north-south, None during the short all-red gap between phases.
    pub fn light_axis(&self, p: Pos) -> Option<usize> {
        let half = LIGHT_PHASE + LIGHT_CLEAR;
        let t = self.time + (p.0 + p.1) as f32 * half;
        let k = (t / half).floor();
        if t - k * half >= LIGHT_PHASE {
            None
        } else {
            Some((k as i64).rem_euclid(2) as usize)
        }
    }

    /// Whether the light at `p` currently lets traffic heading along `dir` (DIRS index) through.
    pub fn light_green(&self, p: Pos, dir: usize) -> bool {
        self.light_axis(p) == Some(dir % 2)
    }

    fn tile_occupancy(&self, p: Pos) -> u32 {
        let b = idx(p) * 4;
        self.occ[b..b + 4].iter().filter(|o| **o).count() as u32
    }

    // ----- Spawning ----------------------------------------------------

    fn footprint_free(&self, anchor: Pos, facing: usize, w: i32, doors: &[i32]) -> bool {
        // Building, car park and driveways, plus a one-tile margin, must be clear of buildings,
        // and the shop itself must sit on empty grass, off any other shop's driveway. The tile
        // in front of each driveway must be open land or road so a road can always reach it.
        let at = |u, v| shop_tile(anchor, facing, u, v);
        for &u in doors {
            let p = at(u, 2);
            if !in_bounds(p) || !(self.cells[idx(p)] == Cell::Road || (self.cells[idx(p)] == Cell::Empty && self.terrain[idx(p)] == Terrain::Grass)) {
                return false;
            }
        }
        for v in -3..=2 {
            for u in -1..=w {
                let p = at(u, v);
                if !in_bounds(p) {
                    return false;
                }
                let inside = ((-2..=0).contains(&v) && (0..w).contains(&u)) || (v == 1 && doors.contains(&u));
                let c = self.cells[idx(p)];
                if inside {
                    if c != Cell::Empty || self.terrain[idx(p)] != Terrain::Grass || self.near_roundabout(p) || self.blocks_entrance(p) {
                        return false;
                    }
                } else if !matches!(c, Cell::Empty | Cell::Road) {
                    return false;
                }
            }
        }
        true
    }

    fn place_dest(&mut self, color: usize, anchor: Pos, facing: usize, big: bool) -> Option<usize> {
        let w = if big { BIG_SHOP.0 } else { SMALL_SHOP.0 };
        let us = door_us(big);
        if !self.footprint_free(anchor, facing, w, us) {
            return None;
        }
        let at = |u, v| shop_tile(anchor, facing, u, v);
        let id = self.dests.len();
        let mut lo = (i32::MAX, i32::MAX);
        let mut hi = (i32::MIN, i32::MIN);
        for u in 0..w {
            for v in [-2, -1] {
                let p = at(u, v);
                self.cells[idx(p)] = Cell::Dest(id);
                lo = (lo.0.min(p.0), lo.1.min(p.1));
                hi = (hi.0.max(p.0), hi.1.max(p.1));
            }
            self.cells[idx(at(u, 0))] = Cell::Park(id);
        }
        let doors: Vec<Pos> = us.iter().map(|&u| at(u, 1)).collect();
        for p in &doors {
            self.cells[idx(*p)] = Cell::Door(id);
        }
        self.dests.push(Dest {
            born: self.time,
            origin: lo,
            size: (hi.0 - lo.0 + 1, hi.1 - lo.1 + 1),
            facing,
            anchor,
            doors,
            color,
            pins: 0,
            assigned: 0,
            overflow: 0.0,
            pin_timer: self.rng.range_f32(3.0, 6.0),
        });
        self.roads_dirty = true;
        Some(id)
    }

    /// Try to place a destination of `color` close to `near`, searching outward.
    pub fn spawn_dest_near(&mut self, color: usize, near: Pos) -> Option<usize> {
        for r in 0..20 {
            let mut options = Vec::new();
            for y in near.1 - r..=near.1 + r {
                for x in near.0 - r..=near.0 + r {
                    if (x - near.0).abs().max((y - near.1).abs()) == r {
                        options.push((x, y));
                    }
                }
            }
            while !options.is_empty() {
                let o = options.swap_remove(self.rng.below(options.len()));
                let facing = self.rng.below(4);
                if let Some(id) = self.place_dest(color, o, facing, false) {
                    return Some(id);
                }
            }
        }
        None
    }

    fn spawn_random_dest(&mut self) -> Option<usize> {
        // Prefer the available colour with the fewest destinations.
        let n = self.colors_available();
        let mut counts = vec![0usize; n];
        for d in &self.dests {
            if d.color < n {
                counts[d.color] += 1;
            }
        }
        let min = *counts.iter().min().unwrap_or(&0);
        let choices: Vec<usize> = (0..n).filter(|c| counts[*c] == min).collect();
        let color = choices[self.rng.below(choices.len())];
        let big = self.rng.f32() < big_shop_chance(self.week);
        // Keep destinations apart so the map stays readable; if the map is getting full,
        // accept closer spacing rather than silently giving up. Shops face any way.
        for gap in [7, 4] {
            for _ in 0..300 {
                let o = (2 + self.rng.below((W - 4) as usize) as i32, 2 + self.rng.below((H - 4) as usize) as i32);
                let facing = self.rng.below(4);
                let far = self.dests.iter().all(|d| (d.anchor.0 - o.0).abs() + (d.anchor.1 - o.1).abs() >= gap);
                if !far || !self.dest_site_reachable(o, facing, big) {
                    continue;
                }
                if let Some(id) = self.place_dest(color, o, facing, big) {
                    return Some(id);
                }
            }
        }
        None
    }

    /// Crossing budget for spawn checks: what the player holds now plus one more upgrade.
    fn crossing_budget(&self) -> u32 {
        self.inventory.bridge_tiles + self.inventory.tunnel_tiles + BRIDGE_TILES.max(TUNNEL_TILES)
    }

    /// Fewest water or hill tiles a road would need to cross to get from any of `starts` to
    /// each tile, building only on free tiles (a 0-1 search). Buildings are reached but not
    /// passed through. This is the "could the player ever connect this?" check that keeps
    /// spawns from landing somewhere unreachable.
    fn crossing_costs(&self, starts: &[Pos]) -> Vec<u32> {
        let mut cost = vec![u32::MAX; (W * H) as usize];
        let mut dq = std::collections::VecDeque::new();
        for s in starts {
            if in_bounds(*s) {
                cost[idx(*s)] = 0;
                dq.push_back(*s);
            }
        }
        while let Some(p) = dq.pop_front() {
            let c = cost[idx(p)];
            if !starts.contains(&p) && !matches!(self.cells[idx(p)], Cell::Empty | Cell::Road) {
                continue;
            }
            for d in DIRS {
                let q = (p.0 + d.0, p.1 + d.1);
                if !in_bounds(q) {
                    continue;
                }
                let step = match (self.cells[idx(q)], self.terrain[idx(q)]) {
                    (Cell::Road, _) | (_, Terrain::Grass) => 0,
                    _ => 1,
                };
                if c + step < cost[idx(q)] {
                    cost[idx(q)] = c + step;
                    if step == 0 {
                        dq.push_front(q);
                    } else {
                        dq.push_back(q);
                    }
                }
            }
        }
        cost
    }

    /// A shop at `origin` must be connectable to the homes already in town.
    fn dest_site_reachable(&self, anchor: Pos, facing: usize, big: bool) -> bool {
        if self.homes.is_empty() {
            return true;
        }
        let front: Vec<Pos> = door_us(big).iter().map(|&u| shop_tile(anchor, facing, u, 2)).filter(|p| in_bounds(*p)).collect();
        let cost = self.crossing_costs(&front);
        let budget = self.crossing_budget();
        self.homes.iter().any(|h| cost[idx(h.pos)] <= budget)
    }

    /// Place a home of the destination's colour a few tiles away from it.
    pub fn spawn_home(&mut self, dest: usize) -> Option<usize> {
        let d = &self.dests[dest];
        let (door, color) = (d.doors[0], d.color);
        let cost = self.crossing_costs(&d.doors.clone());
        let budget = self.crossing_budget();
        let mut options = Vec::new();
        for y in door.1 - 9..=door.1 + 9 {
            for x in door.0 - 12..=door.0 + 12 {
                let p = (x, y);
                let dist = (x - door.0).abs() + (y - door.1).abs();
                if !(4..=12).contains(&dist) || !in_bounds(p) {
                    continue;
                }
                if self.cells[idx(p)] != Cell::Empty || self.terrain[idx(p)] != Terrain::Grass || self.near_roundabout(p) {
                    continue;
                }
                // Never put a home where no road could reach its shop, or across a shop's entrance.
                if cost[idx(p)] > budget || self.blocks_entrance(p) {
                    continue;
                }
                // Needs at least two open sides and no building directly next to it.
                let mut open = 0;
                let mut crowded = false;
                for dd in DIRS {
                    let n = (p.0 + dd.0, p.1 + dd.1);
                    match self.cell(n) {
                        Cell::Empty | Cell::Road if self.terrain[idx(n)] == Terrain::Grass => open += 1,
                        Cell::Empty | Cell::Road => {}
                        _ => crowded = true,
                    }
                }
                if open >= 2 && !crowded {
                    options.push(p);
                }
            }
        }
        if options.is_empty() {
            return None;
        }
        let p = options[self.rng.below(options.len())];
        Some(self.add_home_facing(p, color, Some(door)))
    }

    /// Undo the most recent `place_dest` (used when no home can be placed for it).
    fn remove_last_dest(&mut self) {
        let Some(d) = self.dests.pop() else { return };
        let id = self.dests.len();
        for c in self.cells.iter_mut() {
            if matches!(*c, Cell::Dest(i) | Cell::Door(i) | Cell::Park(i) if i == id) {
                *c = Cell::Empty;
            }
        }
        let _ = d;
        self.roads_dirty = true;
    }

    /// Whether `p` is beside a shop driveway, where a road has to be able to join it.
    fn blocks_entrance(&self, p: Pos) -> bool {
        DIRS.iter().any(|d| matches!(self.cell((p.0 + d.0, p.1 + d.1)), Cell::Door(_)))
    }

    /// Place a home directly, for tests and scripted setups.
    pub fn add_home(&mut self, p: Pos, color: usize) -> usize {
        self.add_home_facing(p, color, None)
    }

    /// Place a home. Its driveway starts on the open side facing `toward` (if given),
    /// and turns later to meet the first road the player draws beside it.
    fn add_home_facing(&mut self, p: Pos, color: usize, toward: Option<Pos>) -> usize {
        let id = self.homes.len();
        self.cells[idx(p)] = Cell::Home(id);
        let mut facing = 1;
        let mut best = i32::MAX;
        for (k, d) in DIRS.iter().enumerate() {
            let n = (p.0 + d.0, p.1 + d.1);
            let open = in_bounds(n) && matches!(self.cells[idx(n)], Cell::Empty | Cell::Road) && self.terrain[idx(n)] == Terrain::Grass;
            if !open {
                continue;
            }
            let score = match toward {
                Some(t) => (t.0 - n.0).abs() + (t.1 - n.1).abs(),
                None => if k == 1 { 0 } else { 1 },
            };
            if score < best {
                best = score;
                facing = k;
            }
        }
        self.homes.push(Home { pos: p, color, born: self.time, facing });
        self.update_facing(id);
        self.cars.push(Car {
            state: CarState::Idle,
            path: vec![p],
            i: 0,
            progress: 1.0,
            prev: p,
            cur: p,
            slot: None,
            retry: 0.0,
            wait: 0.0,
        });
        id
    }

    /// Place a shop facing down, for tests and scripted setups. Its building covers rows
    /// `origin.1 - 1` and `origin.1`, its car park row `origin.1 + 1`, and its driveway is
    /// at `(origin.0 + 1, origin.1 + 2)`.
    pub fn add_dest(&mut self, origin: Pos, color: usize) -> Option<usize> {
        self.place_dest(color, (origin.0, origin.1 + 1), 1, false)
    }

    /// Place a big two-driveway shop facing down, laid out like `add_dest`, with driveways
    /// at `(origin.0 + 1, origin.1 + 2)` and `(origin.0 + 4, origin.1 + 2)`.
    pub fn add_big_dest(&mut self, origin: Pos, color: usize) -> Option<usize> {
        self.place_dest(color, (origin.0, origin.1 + 1), 1, true)
    }

    /// Place a shop facing `facing` (a DIRS index) with its car park's front-left tile at
    /// `anchor`, for tests and scripted setups.
    pub fn add_dest_facing(&mut self, anchor: Pos, facing: usize, color: usize, big: bool) -> Option<usize> {
        self.place_dest(color, anchor, facing, big)
    }

    /// The tile a home's driveway leads to.
    pub fn driveway(&self, h: usize) -> Pos {
        let home = &self.homes[h];
        let d = DIRS[home.facing];
        (home.pos.0 + d.0, home.pos.1 + d.1)
    }

    /// Keep a home's driveway if a road meets it; otherwise turn it toward a road beside the house.
    fn update_facing(&mut self, h: usize) {
        let p = self.homes[h].pos;
        let joined = |s: &Self, n: Pos| in_bounds(n) && matches!(s.cells[idx(n)], Cell::Road);
        if joined(self, self.driveway(h)) {
            return;
        }
        // Don't swing the driveway around while the family car is out.
        if self.cars.get(h).is_some_and(|c| c.state != CarState::Idle) {
            return;
        }
        for k in 0..4 {
            let d = DIRS[k];
            if joined(self, (p.0 + d.0, p.1 + d.1)) {
                self.homes[h].facing = k;
                return;
            }
        }
    }

    fn update_facings_near(&mut self, p: Pos) {
        for d in DIRS {
            if let Cell::Home(h) = self.cell((p.0 + d.0, p.1 + d.1)) {
                self.update_facing(h);
            }
        }
    }

    /// Whether a car may move between neighbouring tiles a and b (houses only via their driveway).
    fn link_ok(&self, a: Pos, b: Pos) -> bool {
        let ok = |h: Pos, other: Pos| match self.cells[idx(h)] {
            Cell::Home(id) => self.driveway(id) == other,
            // A car park only joins its own shop's car park and driveways.
            Cell::Park(id) => matches!(self.cells[idx(other)], Cell::Park(j) | Cell::Door(j) if j == id),
            // A shop driveway joins its own car park behind it, and roads on its front or
            // either side; never another shop or a building.
            Cell::Door(id) => match self.cells[idx(other)] {
                Cell::Park(j) | Cell::Door(j) => j == id,
                Cell::Dest(_) => false,
                _ => true,
            },
            _ => true,
        };
        ok(a, b) && ok(b, a)
    }

    /// Tiles a car may drive across on its way somewhere else: roads, and the car park and
    /// driveways of a big shop (which link its two driveways).
    pub fn passable(&self, p: Pos) -> bool {
        match self.cells[idx(p)] {
            Cell::Road => true,
            Cell::Park(d) | Cell::Door(d) => self.dests[d].big(),
            _ => false,
        }
    }

    // ----- Pathfinding -------------------------------------------------

    /// Cheapest route from `start` to the first tile satisfying `is_goal`.
    /// Only roads can be passed through; homes and doors can only be the start or the goal.
    /// Busy tiles cost more, so cars spread over alternative routes.
    pub fn find_path(&self, start: Pos, is_goal: &dyn Fn(Pos) -> bool) -> Option<Vec<Pos>> {
        let n = (W * H) as usize;
        let mut dist = vec![u32::MAX; n];
        let mut from = vec![usize::MAX; n];
        let mut heap = BinaryHeap::new();
        dist[idx(start)] = 0;
        heap.push(Reverse((0u32, idx(start))));
        while let Some(Reverse((d, i))) = heap.pop() {
            if d > dist[i] {
                continue;
            }
            let p = ((i as i32) % W, (i as i32) / W);
            if p != start && is_goal(p) {
                let mut path = vec![p];
                let mut c = i;
                while from[c] != usize::MAX {
                    c = from[c];
                    path.push(((c as i32) % W, (c as i32) / W));
                }
                path.reverse();
                return Some(path);
            }
            if p != start && !self.passable(p) {
                continue;
            }
            let mut nbrs: Vec<(Pos, u32)> = DIRS.iter().map(|dd| ((p.0 + dd.0, p.1 + dd.1), 10)).collect();
            for h in &self.highways {
                let len = (h.len() * 10.0 / HIGHWAY_SPEED) as u32;
                if h.a == p {
                    nbrs.push((h.b, len));
                } else if h.b == p {
                    nbrs.push((h.a, len));
                }
            }
            for (q, step) in nbrs {
                if !in_bounds(q) {
                    continue;
                }
                let qi = idx(q);
                let enterable = self.passable(q) || is_goal(q);
                if !enterable || !self.link_ok(p, q) {
                    continue;
                }
                let crossing = if self.junctions[qi] == Junction::Plain && self.is_crossing(q) { 4 } else { 0 };
                let cost = d + step + crossing + 6 * self.tile_occupancy(q);
                if cost < dist[qi] {
                    dist[qi] = cost;
                    from[qi] = i;
                    heap.push(Reverse((cost, qi)));
                }
            }
        }
        None
    }

    fn route_car(&mut self, c: usize) {
        let car = &self.cars[c];
        let start = car.cur;
        let path = match car.state {
            CarState::ToDest(d) => {
                let doors = self.dests[d].doors.clone();
                self.find_path(start, &|p| doors.contains(&p))
            }
            CarState::ToHome => {
                let home = self.homes[c].pos;
                if start == home {
                    Some(vec![home])
                } else {
                    self.find_path(start, &|p| p == home)
                }
            }
            _ => return,
        };
        match path {
            Some(p) => {
                let car = &mut self.cars[c];
                car.path = p;
                car.i = 0;
            }
            None => {
                if let CarState::ToDest(d) = self.cars[c].state {
                    // Can't reach the destination any more: release the trip and head home.
                    self.dests[d].assigned -= 1;
                    self.cars[c].state = CarState::ToHome;
                    self.route_car(c);
                } else {
                    let car = &mut self.cars[c];
                    car.path = vec![car.cur];
                    car.i = 0;
                    car.retry = 1.0;
                }
            }
        }
    }

    // ----- Update ------------------------------------------------------

    pub fn step(&mut self) {
        if self.game_over || self.offer.is_some() {
            return;
        }
        let dt = DT;
        self.time += dt;

        if let Some((_, t)) = &mut self.toast {
            *t -= dt;
            if *t <= 0.0 {
                self.toast = None;
            }
        }

        if self.time >= self.week as f32 * WEEK_LENGTH {
            self.week += 1;
            self.roads_left += ROADS_PER_WEEK;
            self.toast = Some((format!("Week {}: +{} road tiles", self.week, ROADS_PER_WEEK), 4.0));
            if offer_week(self.week) {
                self.offer = Some(self.make_offer());
            }
            self.events.push(Event::WeekEnd);
            self.schedule_rush();
        }
        self.update_rush();

        if self.roads_dirty {
            self.roads_dirty = false;
            for c in 0..self.cars.len() {
                if matches!(self.cars[c].state, CarState::ToDest(_) | CarState::ToHome) {
                    self.route_car(c);
                }
            }
        }

        self.update_spawning(dt);
        self.update_pins(dt);

        self.dispatch_timer -= dt;
        if self.dispatch_timer <= 0.0 {
            self.dispatch_timer = 0.25;
            self.dispatch();
        }

        for c in 0..self.cars.len() {
            self.update_car(c, dt);
        }
    }

    /// Plan this week's rush wave, if it is a rush week and there is a colour to rush.
    fn schedule_rush(&mut self) {
        self.rush = None;
        if !rush_week(self.week) || self.dests.is_empty() {
            return;
        }
        let colors: Vec<usize> = (0..MAX_COLORS).filter(|c| self.dests.iter().any(|d| d.color == *c)).collect();
        let color = colors[self.rng.below(colors.len())];
        let start = (self.week - 1) as f32 * WEEK_LENGTH + RUSH_WARN_AT + RUSH_LEAD;
        self.rush = Some(Rush { color, start, end: start + RUSH_LENGTH, announced: false });
    }

    fn update_rush(&mut self) {
        let Some(mut r) = self.rush else { return };
        if !r.announced && self.time >= r.start - RUSH_LEAD {
            r.announced = true;
            self.events.push(Event::RushSoon);
            self.toast = Some((format!("Rush hour soon at the {} shops", COLOR_NAMES[r.color]), 4.0));
        }
        self.rush = (self.time < r.end).then_some(r);
    }

    /// Seconds until the rush starts (negative once running), if one is announced.
    pub fn rush_countdown(&self) -> Option<(usize, f32)> {
        let r = self.rush?;
        (self.time >= r.start - RUSH_LEAD).then(|| (r.color, r.start - self.time))
    }

    fn update_spawning(&mut self, dt: f32) {
        let pace = pace(self.week);
        self.home_timer -= dt * pace;
        // Homes keep pace with shops: about five per shop at most, so the town grows
        // denser without burying the map.
        let room = self.homes.len() < 5 * self.dests.len() + 2;
        if self.home_timer <= 0.0 && !self.dests.is_empty() && room {
            self.home_timer = self.rng.range_f32(12.0, 18.0);
            // Favour destinations with few homes nearby.
            let mut best = 0;
            let mut best_score = f32::MAX;
            for (i, d) in self.dests.iter().enumerate() {
                let homes = self.homes.iter().filter(|h| h.color == d.color).count() as f32;
                let dests = self.dests.iter().filter(|e| e.color == d.color).count() as f32;
                let s = homes / dests + self.rng.f32();
                if s < best_score {
                    best_score = s;
                    best = i;
                }
            }
            if self.spawn_home(best).is_some() {
                self.events.push(Event::NewHome);
            }
        }
        // Shops follow a schedule: each week brings the town up to its target, a little
        // after the week starts. A failed placement retries five seconds later.
        let due = self.dests.len() < target_dests(self.week) && self.week_progress() >= 0.3;
        if due {
            self.dest_timer -= dt;
            if self.dest_timer <= 0.0 {
                let placed = self.spawn_random_dest().filter(|d| {
                    if self.spawn_home(*d).is_some() {
                        return true;
                    }
                    // No room for a home that could reach it: take the shop back.
                    self.remove_last_dest();
                    false
                });
                if placed.is_some() {
                    self.toast = Some(("A new destination has opened".into(), 3.0));
                    self.events.push(Event::NewDest);
                    self.dest_timer = 8.0;
                } else {
                    self.dest_timer = 5.0;
                }
            }
        }
    }

    fn update_pins(&mut self, dt: f32) {
        let interval = pin_interval(self.week) * self.map.difficulty();
        let rush = self.rush.filter(|r| r.active(self.time)).map(|r| r.color);
        for i in 0..self.dests.len() {
            // A shop whose colour has no homes yet waits for one before asking for trips.
            let color = self.dests[i].color;
            if !self.homes.iter().any(|h| h.color == color) {
                continue;
            }
            let d = &mut self.dests[i];
            d.pin_timer -= dt;
            if d.pin_timer <= 0.0 {
                let jitter = self.rng.range_f32(0.7, 1.3);
                let busy = if d.big() { 1.6 } else { 1.0 };
                let rushing = if rush == Some(d.color) { RUSH_FACTOR } else { 1.0 };
                d.pin_timer = interval * jitter * rushing / busy;
                d.pins += 1;
            }
            if d.pins > d.pin_limit() {
                if d.overflow == 0.0 {
                    self.events.push(Event::Warning);
                }
                d.overflow += dt / OVERFLOW_TIME;
            } else {
                d.overflow = (d.overflow - dt / (OVERFLOW_TIME * 0.5)).max(0.0);
            }
            if d.overflow >= 1.0 && !self.game_over {
                self.events.push(Event::GameOver);
                self.game_over = true;
                self.failed_dest = Some(i);
            }
        }
    }

    fn dispatch(&mut self) {
        for d in 0..self.dests.len() {
            while self.dests[d].pins > self.dests[d].assigned {
                let color = self.dests[d].color;
                // Search from each driveway and keep the nearest idle matching car.
                let mut found: Option<Vec<Pos>> = None;
                for door in self.dests[d].doors.clone() {
                    let cells = &self.cells;
                    let homes = &self.homes;
                    let cars = &self.cars;
                    let p = self.find_path(door, &|p| match cells[idx(p)] {
                        Cell::Home(h) => homes[h].color == color && cars[h].state == CarState::Idle,
                        _ => false,
                    });
                    if let Some(p) = p {
                        if found.as_ref().is_none_or(|f| p.len() < f.len()) {
                            found = Some(p);
                        }
                    }
                }
                let Some(mut path) = found else { break };
                path.reverse();
                let h = match self.cells[idx(path[0])] {
                    Cell::Home(h) => h,
                    _ => unreachable!(),
                };
                let car = &mut self.cars[h];
                car.state = CarState::ToDest(d);
                car.path = path;
                car.i = 0;
                car.progress = 1.0;
                car.prev = car.cur;
                self.dests[d].assigned += 1;
            }
        }
    }

    pub(crate) fn update_car(&mut self, c: usize, dt: f32) {
        match self.cars[c].state {
            CarState::Idle => {}
            CarState::Unloading(d) => {
                self.cars[c].retry -= dt;
                if self.cars[c].retry <= 0.0 {
                    let dest = &mut self.dests[d];
                    debug_assert!(dest.pins > 0 && dest.assigned > 0);
                    dest.pins = dest.pins.saturating_sub(1);
                    dest.assigned = dest.assigned.saturating_sub(1);
                    self.score += 1;
                    self.events.push(Event::TripDone);
                    self.cars[c].state = CarState::ToHome;
                    self.route_car(c);
                }
            }
            CarState::ToDest(_) | CarState::ToHome => {
                let car = &mut self.cars[c];
                if car.path.len() <= 1 && car.cur != self.homes[c].pos && car.retry > 0.0 {
                    car.retry -= dt;
                    if car.retry <= 0.0 {
                        self.route_car(c);
                    }
                    return;
                }
                let seg = (((car.cur.0 - car.prev.0).pow(2) + (car.cur.1 - car.prev.1).pow(2)) as f32).sqrt().max(1.0);
                let speed = if seg > 1.5 { CAR_SPEED * HIGHWAY_SPEED / seg } else { CAR_SPEED };
                car.progress = (car.progress + speed * dt).min(1.0);
                if car.progress < 1.0 {
                    return;
                }
                if car.i + 1 >= car.path.len() {
                    self.arrive(c);
                    return;
                }
                let next = car.path[car.i + 1];
                let dir = dir_between(car.cur, next);
                let next_slot = if self.cells[idx(next)] == Cell::Road {
                    Some(idx(next) * 4 + dir)
                } else {
                    None
                };
                if let Some(s) = next_slot {
                    if self.occ[s] {
                        return; // queue behind the car ahead
                    }
                    if self.is_crossing(next) {
                        let wait = match self.junctions[idx(next)] {
                            Junction::Plain => self.crossing_busy(c, next) && self.cars[c].wait < JAM_BREAK,
                            Junction::Roundabout => false,
                            Junction::Light => !self.light_green(next, dir),
                        };
                        if wait {
                            self.cars[c].wait += dt;
                            return; // wait at the junction
                        }
                    }
                    self.cars[c].wait = 0.0;
                    self.occ[s] = true;
                }
                let car = &mut self.cars[c];
                if let Some(s) = car.slot {
                    self.occ[s] = false;
                }
                car.slot = next_slot;
                car.prev = car.cur;
                car.cur = next;
                car.i += 1;
                car.progress = 0.0;
            }
        }
    }

    /// Whether a plain crossing is taken by another car. A car that is only passing the other
    /// way between this crossing and the one `c` is on doesn't count: they use opposite lanes,
    /// and counting them let two neighbouring crossings block each other forever.
    fn crossing_busy(&self, c: usize, next: Pos) -> bool {
        let here = self.cars[c].cur;
        self.cars.iter().enumerate().any(|(k, o)| {
            k != c && o.slot.is_some() && o.cur == next && o.path.get(o.i + 1) != Some(&here)
        })
    }

    fn arrive(&mut self, c: usize) {
        let car = &mut self.cars[c];
        if let Some(s) = car.slot.take() {
            self.occ[s] = false;
        }
        match car.state {
            CarState::ToDest(d) => {
                car.state = CarState::Unloading(d);
                car.retry = UNLOAD_TIME;
            }
            CarState::ToHome => {
                if car.cur == self.homes[c].pos {
                    car.state = CarState::Idle;
                    car.prev = car.cur;
                    car.path = vec![car.cur];
                    car.i = 0;
                } else {
                    car.retry = 1.0;
                }
            }
            _ => {}
        }
    }

    /// Fraction of the current week that has passed.
    pub fn week_progress(&self) -> f32 {
        let start = (self.week - 1) as f32 * WEEK_LENGTH;
        ((self.time - start) / WEEK_LENGTH).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &mut Sim, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            s.step();
        }
    }

    #[test]
    fn new_game_has_a_destination_and_two_homes() {
        let s = Sim::new(7);
        assert_eq!(s.dests.len(), 1);
        assert_eq!(s.homes.len(), 2);
        assert_eq!(s.roads_left, START_ROADS);
    }

    #[test]
    fn roads_cost_tiles_and_refund_on_erase() {
        let mut s = Sim::empty(1);
        assert_eq!(s.place_road((5, 5)), EditResult::Ok);
        assert_eq!(s.place_road((5, 5)), EditResult::NoChange);
        assert_eq!(s.roads_left, START_ROADS - 1);
        assert_eq!(s.erase_road((5, 5)), EditResult::Ok);
        assert_eq!(s.roads_left, START_ROADS);
    }

    #[test]
    fn cannot_build_on_water() {
        let mut s = Sim::empty(1);
        s.terrain[idx((3, 3))] = Terrain::Water;
        assert_eq!(s.place_road((3, 3)), EditResult::NeedBridge);
    }

    #[test]
    fn running_out_of_roads() {
        let mut s = Sim::empty(1);
        s.roads_left = 1;
        assert_eq!(s.place_road((1, 1)), EditResult::Ok);
        assert_eq!(s.place_road((2, 1)), EditResult::NoRoadsLeft);
    }

    /// Destination at (10,5) has its door at (11,7). Home at (11,12). Road (11,8)..(11,11).
    fn connected_setup() -> Sim {
        let mut s = Sim::empty(3);
        let d = s.add_dest((10, 5), 0).unwrap();
        assert_eq!(s.dests[d].doors, vec![(11, 7)]);
        s.add_home((11, 12), 0);
        for y in 8..=11 {
            assert_eq!(s.place_road((11, y)), EditResult::Ok);
        }
        s
    }

    #[test]
    fn connected_home_serves_destination() {
        let mut s = connected_setup();
        s.dests[0].pins = 2;
        run(&mut s, 12.0);
        assert!(s.score >= 2, "score was {}", s.score);
    }

    #[test]
    fn unconnected_home_does_not_serve() {
        let mut s = Sim::empty(3);
        s.add_dest((10, 5), 0).unwrap();
        s.add_home((11, 12), 0);
        s.dests[0].pins = 2;
        run(&mut s, 10.0);
        assert_eq!(s.score, 0);
        assert_eq!(s.cars[0].state, CarState::Idle);
    }

    #[test]
    fn wrong_colour_home_does_not_serve() {
        let mut s = Sim::empty(3);
        s.add_dest((10, 5), 0).unwrap();
        s.add_home((11, 12), 1);
        for y in 8..=11 {
            s.place_road((11, y));
        }
        s.dests[0].pins = 1;
        run(&mut s, 10.0);
        assert_eq!(s.score, 0);
    }

    #[test]
    fn overflow_ends_the_game() {
        let mut s = Sim::empty(3);
        s.add_dest((10, 5), 0).unwrap();
        s.add_home((25, 15), 0); // unconnected, so nobody serves the shop
        s.dests[0].pins = PIN_LIMIT + 3;
        run(&mut s, OVERFLOW_TIME + 2.0);
        assert!(s.game_over);
        assert_eq!(s.failed_dest, Some(0));
    }

    #[test]
    fn cutting_a_road_mid_trip_sends_car_home() {
        let mut s = connected_setup();
        s.dests[0].pins = 1;
        run(&mut s, 0.6);
        assert!(matches!(s.cars[0].state, CarState::ToDest(_)));
        assert_ne!(s.cars[0].cur, (11, 8));
        assert_eq!(s.erase_road((11, 8)), EditResult::Ok);
        run(&mut s, 0.1);
        assert_eq!(s.dests[0].assigned, 0);
        assert_eq!(s.cars[0].state, CarState::ToHome);
        run(&mut s, 5.0);
        assert_eq!(s.cars[0].state, CarState::Idle);
        assert_eq!(s.score, 0);
    }

    #[test]
    fn cars_queue_instead_of_overlapping() {
        let mut s = Sim::empty(4);
        s.add_dest((10, 2), 0).unwrap(); // door (11,4)
        for y in 5..=14 {
            s.place_road((11, y));
        }
        for x in 12..=16 {
            s.place_road((x, 14));
        }
        for x in 12..=16 {
            s.add_home((x, 15), 0);
        }
        s.dests[0].pins = 5;
        for _ in 0..(8.0 / DT) as usize {
            s.step();
            // No two cars ever hold the same lane slot.
            let mut seen = std::collections::HashSet::new();
            for c in &s.cars {
                if let Some(sl) = c.slot {
                    assert!(seen.insert(sl), "two cars share a lane slot");
                }
            }
        }
        run(&mut s, 20.0);
        assert!(s.score >= 5, "score was {}", s.score);
    }

    #[test]
    fn house_turns_to_face_the_first_road() {
        let mut s = Sim::empty(5);
        let h = s.add_home((10, 10), 0);
        assert_eq!(s.homes[h].facing, 1); // starts facing down
        s.place_road((9, 10)); // road on the left
        assert_eq!(s.driveway(h), (9, 10));
        // A second road elsewhere doesn't steal the driveway.
        s.place_road((11, 10));
        assert_eq!(s.driveway(h), (9, 10));
        // Removing the joined road lets it turn to the remaining one.
        s.erase_road((9, 10));
        assert_eq!(s.driveway(h), (11, 10));
    }

    #[test]
    fn cars_only_use_the_driveway() {
        // Road passes beside the house on two sides but only one is the driveway.
        let mut s = Sim::empty(6);
        s.add_dest((10, 5), 0).unwrap(); // driveway (11,7)
        let h = s.add_home((12, 10), 0);
        for y in 8..=11 {
            s.place_road((11, y));
        }
        assert_eq!(s.driveway(h), (11, 10));
        // A road on the house's other side, not connected to the shop, is ignored.
        s.place_road((13, 10));
        assert_eq!(s.driveway(h), (11, 10));
        s.dests[0].pins = 1;
        run(&mut s, 8.0);
        assert!(s.score >= 1);
        // Cars never stepped onto the non-driveway road.
        assert_eq!(s.occ[idx((13, 10)) * 4..idx((13, 10)) * 4 + 4], [false; 4]);
    }

    #[test]
    fn a_path_cannot_cut_through_a_house() {
        let mut s = Sim::empty(7);
        let h = s.add_home((10, 10), 0);
        s.place_road((9, 10));
        s.place_road((11, 10));
        assert_eq!(s.driveway(h), (9, 10));
        assert!(s.find_path((9, 10), &|p| p == (11, 10)).is_none());
    }

    #[test]
    fn big_shop_has_two_driveways_and_either_works() {
        let mut s = Sim::empty(8);
        let d = s.add_big_dest((10, 5), 0).unwrap();
        assert_eq!(s.dests[d].doors, vec![(11, 7), (14, 7)]);
        assert!(s.dests[d].big());
        // Home connected only to the right-hand driveway.
        s.add_home((14, 11), 0);
        for y in 8..=10 {
            s.place_road((14, y));
        }
        s.dests[d].pins = 2;
        run(&mut s, 10.0);
        assert!(s.score >= 2, "score was {}", s.score);
    }

    #[test]
    fn shops_have_a_car_park_and_big_ones_can_be_driven_through() {
        let mut s = Sim::empty(8);
        let small = s.add_dest((3, 3), 1).unwrap();
        assert_eq!(s.cell((3, 3)), Cell::Dest(small));
        assert_eq!(s.cell((3, 4)), Cell::Park(small));
        assert_eq!(s.cell((4, 4)), Cell::Park(small));
        assert_eq!(s.cell((4, 5)), Cell::Door(small));
        assert_eq!(s.cell((3, 5)), Cell::Empty);
        let big = s.add_big_dest((10, 5), 2).unwrap();
        assert_eq!(s.cell((12, 6)), Cell::Park(big));
        assert_eq!(s.cell((11, 7)), Cell::Door(big));
        assert_eq!(s.cell((14, 7)), Cell::Door(big));
        // A teal home east of the big shop; its own shop lies west, reachable only by
        // driving in one driveway, across the car park and out the other.
        s.add_home((18, 9), 0);
        for p in [(17, 9), (16, 9), (15, 9), (14, 9), (14, 8)] {
            s.place_road(p);
        }
        let target = s.add_dest((3, 9), 0).unwrap();
        for p in [(11, 8), (11, 9), (10, 9), (9, 9), (8, 9), (7, 9), (7, 10), (7, 11), (7, 12), (6, 12), (5, 12), (4, 12)] {
            s.place_road(p);
        }
        assert_eq!(s.cell((4, 11)), Cell::Door(target));
        let path = s.find_path((18, 9), &|p| p == (4, 11)).expect("route through the car park");
        assert!(path.contains(&(13, 6)) && path.contains(&(12, 6)), "{:?}", path);
        // A road beside the car park or building doesn't connect into it.
        s.place_road((9, 6));
        assert_eq!(s.link_count((9, 6)), 0);
        s.place_road((16, 5));
        assert_eq!(s.link_count((16, 5)), 0);
        s.dests[target].pins = 2;
        s.dests[big].pins = 0;
        run(&mut s, 20.0);
        assert!(s.score >= 1, "score was {}", s.score);
    }

    #[test]
    fn a_driveway_can_be_joined_from_its_side() {
        let mut s = Sim::empty(8);
        s.add_dest((10, 5), 0).unwrap(); // driveway (11,7)
        s.add_home((6, 7), 0);
        for x in 7..=10 {
            s.place_road((x, 7));
        }
        assert_eq!(s.link_count((10, 7)), 2);
        s.dests[0].pins = 1;
        run(&mut s, 12.0);
        assert!(s.score >= 1, "score was {}", s.score);
    }

    #[test]
    fn shops_can_face_any_way() {
        for facing in 0..4 {
            let mut s = Sim::empty(8);
            let d = s.add_dest_facing((20, 12), facing, 0, false).unwrap();
            let door = s.dests[d].doors[0];
            let f = DIRS[facing];
            // The car park is straight behind the driveway, the building behind that.
            assert_eq!(s.cell((door.0 - f.0, door.1 - f.1)), Cell::Park(d));
            assert_eq!(s.cell((door.0 - 2 * f.0, door.1 - 2 * f.1)), Cell::Dest(d));
            let entry = s.dests[d].entry(door);
            let home = (entry.0 + 3 * f.0, entry.1 + 3 * f.1);
            s.add_home(home, 0);
            for k in 0..3 {
                s.place_road((entry.0 + k * f.0, entry.1 + k * f.1));
            }
            s.dests[d].pins = 1;
            run(&mut s, 12.0);
            assert!(s.score >= 1, "facing {facing}: score was {}", s.score);
            let (lo, hi) = s.dests[d].bounds();
            assert_eq!((hi.0 - lo.0 + 1) * (hi.1 - lo.1 + 1), if facing % 2 == 0 { 4 * 3 } else { 3 * 4 });
        }
    }

    #[test]
    fn roundabouts_need_clear_corners_and_keep_them_clear() {
        let mut s = Sim::empty(14);
        for x in 4..=8 {
            s.place_road((x, 6));
        }
        s.inventory.roundabouts = 2;
        s.add_home((5, 5), 0);
        assert_eq!(s.place_junction((6, 6), Junction::Roundabout), EditResult::Blocked);
        assert_eq!(s.place_junction((8, 6), Junction::Roundabout), EditResult::Ok);
        assert!(s.near_roundabout((9, 7)));
        // Shops won't open in its corners either.
        assert_eq!(s.add_dest((8, 7), 1), None);
    }

    #[test]
    fn bridges_and_tunnels_use_their_own_allowance() {
        let mut s = Sim::empty(9);
        s.terrain[idx((5, 5))] = Terrain::Water;
        s.terrain[idx((6, 5))] = Terrain::Hill;
        s.inventory.bridge_tiles = 1;
        assert_eq!(s.place_road((5, 5)), EditResult::Ok);
        assert_eq!(s.place_road((6, 5)), EditResult::NeedTunnel);
        assert_eq!(s.roads_left, START_ROADS);
        assert_eq!(s.inventory.bridge_tiles, 0);
        assert_eq!(s.erase_road((5, 5)), EditResult::Ok);
        assert_eq!(s.inventory.bridge_tiles, 1);
    }

    #[test]
    fn week_end_offers_two_upgrades_and_waits() {
        let mut s = Sim::new(12);
        for d in &mut s.dests {
            d.pins = 0;
        }
        s.time = WEEK_LENGTH - DT * 0.5;
        s.step();
        let offer = s.offer.expect("an offer at week end");
        assert_ne!(offer[0], offer[1]);
        let t = s.time;
        s.step();
        assert_eq!(s.time, t, "sim pauses until a choice is made");
        let before = (s.inventory, s.roads_left);
        s.choose(1);
        assert!(s.offer.is_none());
        assert_ne!((s.inventory, s.roads_left), before);
    }

    /// A plus-shaped crossing at (10,10) with roads one tile long on each arm.
    fn crossing() -> Sim {
        let mut s = Sim::empty(13);
        for p in [(10, 10), (9, 10), (11, 10), (10, 9), (10, 11)] {
            s.place_road(p);
        }
        s
    }

    #[test]
    fn plain_crossing_lets_one_car_through() {
        let mut s = crossing();
        assert!(s.is_crossing((10, 10)));
        // A car already crossing eastbound blocks a northbound one.
        s.add_home((10, 12), 0);
        s.homes[0].facing = 3;
        s.add_home((7, 10), 0);
        let slot = idx((10, 10)) * 4;
        s.occ[slot] = true;
        let o = &mut s.cars[1];
        o.state = CarState::ToHome;
        o.path = vec![(9, 10), (10, 10), (11, 10)];
        o.i = 1;
        o.cur = (10, 10);
        o.prev = (9, 10);
        o.slot = Some(slot);
        let c = &mut s.cars[0];
        c.state = CarState::ToHome;
        c.path = vec![(10, 11), (10, 10), (10, 9)];
        c.cur = (10, 11);
        c.prev = (10, 12);
        c.i = 0;
        c.progress = 1.0;
        s.update_car(0, DT);
        assert_eq!(s.cars[0].cur, (10, 11), "waits at a busy plain crossing");
        // A car coming the other way out of the crossing toward us doesn't block.
        s.cars[1].path = vec![(10, 9), (10, 10), (10, 11)];
        s.cars[1].i = 1;
        assert!(!s.crossing_busy(0, (10, 10)));
        s.cars[1].path = vec![(9, 10), (10, 10), (11, 10)];
        // With a roundabout it goes straight in.
        s.inventory.roundabouts = 1;
        assert_eq!(s.place_junction((10, 10), Junction::Roundabout), EditResult::Ok);
        s.update_car(0, DT);
        assert_eq!(s.cars[0].cur, (10, 10));
    }

    #[test]
    fn traffic_light_alternates_directions() {
        let mut s = crossing();
        s.inventory.lights = 1;
        assert_eq!(s.place_junction((10, 10), Junction::Light), EditResult::Ok);
        let axis = s.light_axis((10, 10)).expect("a light starts on a phase");
        assert!(s.light_green((10, 10), axis) && !s.light_green((10, 10), axis + 1));
        // A short all-red gap, then the other way goes.
        s.time += LIGHT_PHASE + 0.1;
        assert_eq!(s.light_axis((10, 10)), None);
        s.time += LIGHT_CLEAR;
        assert_eq!(s.light_axis((10, 10)), Some(1 - axis));
        // Erasing the tile gives the light back.
        s.erase_road((10, 10));
        assert_eq!(s.inventory.lights, 1);
    }

    #[test]
    fn highway_links_far_apart_roads_over_water() {
        let mut s = Sim::empty(14);
        for y in 0..H {
            for x in 14..17 {
                s.terrain[idx((x, y))] = Terrain::Water;
            }
        }
        s.add_dest((20, 5), 0).unwrap(); // driveway (21,7)
        s.add_home((10, 8), 0);
        for x in 11..=13 {
            s.place_road((x, 8));
        }
        for x in 17..=21 {
            s.place_road((x, 8));
        }
        assert!(s.find_path((13, 8), &|p| p == (17, 8)).is_none());
        s.inventory.highways = 1;
        assert_eq!(s.place_highway((13, 8), (17, 8)), EditResult::Ok);
        assert!(s.find_path((13, 8), &|p| p == (17, 8)).is_some());
        s.dests[0].pins = 1;
        run(&mut s, 12.0);
        assert!(s.score >= 1, "score was {}", s.score);
    }

    #[test]
    fn maps_have_their_own_character() {
        let count = |k: MapKind, t: Terrain| map::generate_kind(k, 3).iter().filter(|x| **x == t).count();
        assert!(count(MapKind::Highland, Terrain::Hill) > count(MapKind::Bayside, Terrain::Hill));
        assert!(count(MapKind::TwinIslands, Terrain::Water) > count(MapKind::Highland, Terrain::Water));
        for k in MapKind::ALL {
            let s = Sim::new_on(k, 5);
            assert_eq!(s.dests.len(), 1, "{:?} opens with a shop", k);
        }
    }

    #[test]
    fn spawning_continues_over_time() {
        let mut s = Sim::new(11);
        for _ in 0..(2.5 * WEEK_LENGTH / DT) as usize {
            for d in &mut s.dests {
                d.pins = 0;
            }
            if s.offer.is_some() {
                s.choose(0);
            }
            s.step();
        }
        assert!(s.homes.len() > 2);
        assert!(s.dests.len() > 1);
        assert!(s.week >= 3);
    }

    #[test]
    fn difficulty_climbs_then_levels_off() {
        assert_eq!(pin_interval(1), 11.0);
        assert!(pin_interval(5) < pin_interval(4));
        assert_eq!(pin_interval(11), 4.0);
        assert_eq!(pin_interval(14), 4.0, "flat after week 11");
        assert_eq!(pin_interval(16), 3.5);
        assert_eq!(pin_interval(40), 3.0);
        assert_eq!(pace(11), 2.0);
        assert_eq!(pace(30), 2.0, "pace is capped");
        assert_eq!(colors_for_week(1), 2);
        assert_eq!(colors_for_week(12), 6);
        assert!((1..60).all(|w| target_dests(w + 1) >= target_dests(w)));
        assert_eq!(target_dests(60), 20);
        assert!(!rush_week(8) && rush_week(9) && rush_week(13) && !rush_week(14));
        assert!(offer_week(12) && offer_week(13) && !offer_week(14));
    }

    #[test]
    fn offers_follow_the_card_rules() {
        let mut s = Sim::new_on(MapKind::Bayside, 21);
        let mut weeks_without_highway = 0;
        for week in 2..=40 {
            s.week = week;
            if !offer_week(week) {
                continue;
            }
            let last = s.last_pick;
            let o = s.make_offer();
            assert_ne!(o[0], o[1]);
            assert!(!o.contains(&Upgrade::Tunnel) || s.terrain.contains(&Terrain::Hill));
            assert!(last.is_none_or(|l| !o.contains(&l)), "week {} repeats last pick", week);
            if week < 3 {
                assert!(!o.contains(&Upgrade::Highway));
            }
            if o.contains(&Upgrade::Highway) {
                weeks_without_highway = 0;
            } else if week >= 3 {
                weeks_without_highway += 1;
            }
            assert!(weeks_without_highway <= 4, "highway pity timer");
            s.offer = Some(o);
            s.choose(0);
        }
    }

    #[test]
    fn a_rush_wave_is_announced_then_speeds_up_one_colour() {
        let mut s = Sim::new(4);
        s.week = 8;
        s.time = 8.0 * WEEK_LENGTH - DT * 0.5;
        s.step();
        assert_eq!(s.week, 9);
        s.offer = None;
        let r = s.rush.expect("week 9 has a rush");
        assert!(s.rush_countdown().is_none(), "not announced yet");
        s.time = r.start - RUSH_LEAD;
        s.step();
        assert!(s.rush_countdown().is_some());
        assert!(s.events.contains(&Event::RushSoon));
        s.time = r.end;
        s.step();
        assert!(s.rush.is_none());
    }

    #[test]
    fn homes_never_spawn_cut_off_from_their_shop() {
        // A shop on the left, a wide river, nothing to cross it with: every new home
        // must land on the shop's side.
        let mut s = Sim::empty(31);
        for y in 0..H {
            for x in 14..24 {
                s.terrain[idx((x, y))] = Terrain::Water;
            }
        }
        let d = s.add_dest((5, 8), 0).unwrap();
        for _ in 0..30 {
            if let Some(h) = s.spawn_home(d) {
                assert!(s.homes[h].pos.0 < 14, "home at {:?} is across the river", s.homes[h].pos);
            }
        }
        assert!(!s.homes.is_empty());
    }

    #[test]
    fn shops_without_homes_wait_and_entrances_stay_clear() {
        let mut s = Sim::empty(8);
        let d = s.add_dest((10, 5), 2).unwrap();
        run(&mut s, 30.0);
        assert_eq!(s.dests[d].pins, 0, "no homes of its colour yet, so no demand");
        // Homes never land in the row in front of a shop's car park.
        for _ in 0..40 {
            s.spawn_home(d);
        }
        let front = s.dests[d].origin.1 + s.dests[d].size.1 + 1;
        assert!(s.homes.iter().all(|h| !(h.pos.1 == front && (10..13).contains(&h.pos.0))));
    }
}
