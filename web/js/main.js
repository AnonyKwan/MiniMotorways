import { Core, ERR, KIND } from './core.js';
import { Sound } from './audio.js';
import {
  GROUP_COLORS, GROUP_SHAPES, isUnlocked, lockText, packText, errorText, stepsBetween, clockText, MODES, dailyChallenge,
} from './model.js';
import { THEMES, buildTerrainLayer, drawScene, glyph } from './render.js';
import { loadBest, saveBest, loadPref, savePref } from './storage.js';
import { initFeatures } from './features.js';
import { Renderer3D, webglAvailable } from './render3d.js';

const $ = (id) => document.getElementById(id);
const canvas = $('stage');
const ctx = canvas.getContext('2d');
const params = new URLSearchParams(location.search);

const app = {
  core: null,
  screen: 'loading', // loading | menu | game
  cityIndex: 0,
  seed: 1,
  mode: Number(loadPref('mode', 0)) || 0,
  sound: new Sound(),
  born: new Map(),
  lastScore: 0,
  speed: 1,
  tool: 'road',
  themeName: 'light',
  best: loadBest(),
  unlockAll: params.get('unlock') === 'all' || loadPref('unlockAll', false),
  snap: null,
  statics: null,
  terrain: null,
  layer: null,
  staticsVer: -1,
  over: false,
  helpSeen: loadPref('helpSeen', false),
  view: { zoom: 1, panX: 0, panY: 0 },
  hover: null,
  cam: null,
  r3: null,
  use3d: false,
  photo: false,
  canvas,
  W: 0, H: 0, dpr: 1,
};
window.__ll = app; // handy for tests and debugging

// ---------- start-up ----------
async function boot() {
  const pref = loadPref('theme', null);
  const dark = pref ? pref === 'dark' : window.matchMedia && matchMedia('(prefers-color-scheme: dark)').matches;
  setTheme(params.get('theme') || (dark ? 'dark' : 'light'));
  resize();
  try {
    app.core = await Core.load(fetch('little_lanes_core.wasm'));
  } catch (e) {
    document.body.insertAdjacentHTML('beforeend', `<p class="nojs">Could not start the game engine: ${String(e.message || e)}</p>`);
    throw e;
  }
  initRenderer();
  buildTools();
  buildModes();
  buildMenu();
  features = initFeatures(app, { startGame, toast, setSpeed, afterEdit });
  app.screen = 'menu';
  requestAnimationFrame(frame);
  const want = params.get('city');
  const i = want ? app.core.cities.findIndex((c) => c.id === want) : -1;
  if (i >= 0) startGame(i);
  else showMenu();
}

function initRenderer() {
  const want = params.get('render') ? params.get('render') === '3d' : loadPref('render3d', true);
  app.use3d = false;
  if (want && webglAvailable()) {
    try { app.r3 = app.r3 || new Renderer3D($('stage3d')); app.r3.resize(app.W, app.H, app.dpr); app.use3d = true; }
    catch (e) { console.warn('3D view unavailable, using the flat view', e); app.r3 = null; }
  }
  $('stage3d').hidden = !app.use3d;
  $('btn-render').textContent = app.use3d ? '3D' : '2D';
  $('btn-render').disabled = !webglAvailable();
}

/** (Re)builds everything the 3D view needs from the current game. */
function sync3d() {
  if (!app.use3d || !app.statics || !app.terrain) return;
  const layer = buildTerrainLayer(app.statics.w, app.statics.h, app.terrain, THEMES[app.themeName], (w, h) => {
    const cv = document.createElement('canvas'); cv.width = w; cv.height = h; return cv;
  }, { flat: true });
  app.r3.themeName = app.themeName;
  app.r3.setTerrain(app.statics.w, app.statics.h, app.terrain, layer, app.themeName);
  app.r3.setStatics(app.statics);
}

function setTheme(name) {
  app.themeName = name;
  document.documentElement.dataset.theme = name;
  document.querySelector('meta[name="theme-color"]').content = THEMES[name].page;
  savePref('theme', name);
  app.layer = null;
  if (app.screen === 'game') sync3d();
}

// ---------- menu ----------
function buildModes() {
  const box = $('mode-select');
  box.textContent = '';
  for (const m of MODES) {
    const b = document.createElement('button');
    b.type = 'button'; b.setAttribute('role', 'radio'); b.dataset.mode = String(m.id);
    b.setAttribute('aria-checked', String(app.mode === m.id)); b.textContent = m.name;
    b.addEventListener('click', () => {
      app.mode = m.id; savePref('mode', m.id);
      box.querySelectorAll('button').forEach((x) => x.setAttribute('aria-checked', String(x === b)));
      $('mode-blurb').textContent = m.blurb;
      buildMenu();
    });
    box.appendChild(b);
  }
  $('mode-blurb').textContent = MODES[app.mode].blurb;
}

/** Expert opens on a map once one of its own challenges reaches 600 trips (doc 01.6). */
function expertOpen(id) {
  return app.unlockAll || [0, 1, 2].some((i) => ((app.challengeBest || {})[`city:${id}:${i}`] || 0) >= 600);
}

function buildMenu() {
  const grid = $('city-grid');
  grid.textContent = '';
  const cities = app.core.cities;
  const colors = GROUP_COLORS[app.themeName];
  cities.forEach((c, i) => {
    if (c.id === 'osaka' && !app.unlockAll && params.get('osaka') == null) return;
    const needExpert = app.mode === 1 && !expertOpen(c.id);
    const open = isUnlocked(c.id, app.best, app.unlockAll) && !needExpert;
    const b = document.createElement('button');
    b.className = 'card';
    b.disabled = !open;
    b.dataset.city = c.id;
    const start = c.start.map(([k, n]) => `${n} ${['road tiles', 'bridge', 'motorway', 'light', 'roundabout', 'tunnel'][k]}`).join(' · ');
    const best = app.best[c.id] || 0;
    const sw = Array.from({ length: c.groups }, (_, g) => `<i style="background:${colors[g]}"></i>`).join('');
    b.innerHTML = `<span class="name"></span><span class="meta">${c.w}×${c.h} · ${start}</span>` +
      `<span class="swatch">${sw}</span>` +
      (open ? `<span class="best">${best ? `Best ${best} trips` : 'Not played yet'}</span>` : `<span class="lock">🔒 ${needExpert ? 'Expert opens after 600 trips in one of this map’s challenges' : lockText(c.id, cities)}</span>`);
    b.querySelector('.name').textContent = c.name;
    b.addEventListener('click', () => startGame(i));
    grid.appendChild(b);
  });
  $('unlock-all').checked = app.unlockAll;
}

function showMenu() {
  if (app.screen === 'game' && features) features.onLeave();
  app.screen = 'menu';
  $('menu').hidden = false;
  $('hud').hidden = true;
  $('tools').hidden = true;
  $('offer').hidden = true;
  $('over').hidden = true;
  buildMenu();
}

// ---------- game ----------
let features = null;
function startGame(cityIndex, seed, mode = app.mode, opts = {}) {
  if (app.screen === 'game' && features) features.onLeave();
  app.core.setMods(opts.mods || []);
  app.cityIndex = cityIndex;
  app.playMode = mode;
  app.born = new Map();
  app.lastScore = 0;
  app.seed = seed != null ? seed : (params.get('seed') ? Number(params.get('seed')) : (Date.now() & 0x7fffffff));
  app.core.newGame(cityIndex, app.seed, mode);
  const c = app.core.cities[cityIndex];
  app.terrain = app.core.terrain();
  app.layer = null;
  app.snap = app.core.snapshot();
  app.statics = app.core.statics();
  app.staticsVer = app.snap.ver;
  sync3d();
  app.over = false;
  app.view = { zoom: 1, panX: 0, panY: 0 };
  app.cam = null;
  app.screen = 'game';
  setSpeed(1);
  setTool('road');
  $('menu').hidden = true;
  $('hud').hidden = false;
  $('tools').hidden = false;
  $('offer').hidden = true;
  $('over').hidden = true;
  $('hud-city').textContent = (opts.challenge ? `${opts.challenge.title} · ` : '') + (mode ? `${c.name} · ${MODES[mode].name}` : c.name);
  if (features) features.onStart(opts);
  if (!app.helpSeen && !opts.tutorial) { $('help').hidden = false; app.helpSeen = true; savePref('helpSeen', true); setSpeed(0); }
  updateTools(true);
}

function setSpeed(s) {
  app.speed = s;
  for (let i = 0; i < 3; i++) $('speed-' + i).classList.toggle('on', i === s);
}

const SPEEDS = [0, 1, 2];
let last = performance.now();
let acc = 0;
function frame(ts) {
  const dt = Math.min(0.1, Math.max(0, (ts - last) / 1000));
  last = ts;
  if (app.screen === 'game') {
    const blocked = !$('help').hidden;
    if (!app.over && !blocked && SPEEDS[app.speed] > 0) app.core.step(dt * SPEEDS[app.speed]);
    app.snap = app.core.snapshot();
    if (app.snap.ver !== app.staticsVer) refreshStatics();
    soundCues();
    if (features) features.tick(app.snap);
    syncUi();
    render(ts / 1000);
    acc += dt;
  }
  requestAnimationFrame(frame);
}

function soundCues() {
  const s = app.snap;
  if (s.score > app.lastScore) {
    const groups = app.core.cities[app.cityIndex].groups;
    app.sound.deliver((s.score + app.cityIndex) % groups);
  }
  app.lastScore = s.score;
  const pressure = Math.max(0, ...(s.dests || []).map((d) => d[2] / 50));
  app.sound.bed(pressure, s.t);
  if (pressure > 0.6) app.sound.warn(s.t, (pressure - 0.6) / 0.4);
}

function refreshStatics() {
  app.statics = app.core.statics();
  app.staticsVer = app.snap.ver;
  if (app.use3d && app.r3) app.r3.setStatics(app.statics);
}

function render(t) {
  const { W, H } = app;
  const box = regionBox();
  if (app.use3d && app.r3) {
    app.r3.top = app.photo ? 10 : 64;
    app.r3.bottom = app.photo ? 10 : 96;
    app.r3.noFog = app.photo || app.playMode === 3;
    app.r3.setView(box, app.view.zoom, [app.view.panX, app.view.panY]);
    app.r3.draw({ statics: app.statics, snap: app.snap, born: app.born }, performance.now() / 1000);
    drawOverlay3d();
    return;
  }
  if (!app.layer) {
    app.layer = buildTerrainLayer(app.statics.w, app.statics.h, app.terrain, THEMES[app.themeName], (w, h) => {
      const cv = document.createElement('canvas'); cv.width = w; cv.height = h; return cv;
    });
  }
  ctx.setTransform(app.dpr, 0, 0, app.dpr, 0, 0);
  const view = viewTransform();
  drawScene(ctx, W, H, view, {
    w: app.statics.w, h: app.statics.h, terrain: app.terrain, statics: app.statics, snap: app.snap,
    layer: app.layer, hover: app.hover, born: app.born, region: app.photo || app.playMode === 3 ? null : app.snap.region,
  }, THEMES[app.themeName], app.themeName, app.snap.t, performance.now() / 1000);
}

app.cellCenter = (c) => {
  if (app.use3d && app.r3) { const p = app.r3.cellCenterPx(c); const r0 = canvas.getBoundingClientRect(); return [r0.left + p[0], r0.top + p[1]]; }
  const v = viewTransform(), w = app.statics.w, r = canvas.getBoundingClientRect();
  return [r.left + v.ox + ((c % w) + 0.5) * v.scale, r.top + v.oy + (Math.floor(c / w) + 0.5) * v.scale];
};

function baseScale() {
  const w = app.statics ? app.statics.w : 40, h = app.statics ? app.statics.h : 22;
  const top = 64, bottom = 96;
  return Math.max(8, Math.min((app.W - 16) / w, (app.H - top - bottom) / h));
}
function regionBox() {
  const full = [0, 0, app.statics.w, app.statics.h];
  const r = app.photo || app.playMode === 3 || !app.snap || !app.snap.region ? full : app.snap.region;
  // ease toward the target so the camera pulls out smoothly as the town grows
  if (!app.cam) app.cam = [...r];
  else for (let i = 0; i < 4; i++) app.cam[i] += (r[i] - app.cam[i]) * 0.04;
  return app.cam;
}
/** True once the camera has finished easing toward the open part of the map. */
app.camSettled = () => !app.cam || !app.snap || !app.snap.region || app.cam.every((v, i) => Math.abs(v - app.snap.region[i]) < 0.05);

function viewTransform() {
  const [x0, y0, x1, y1] = app.cam || [0, 0, app.statics.w, app.statics.h];
  const rw = Math.max(4, x1 - x0), rh = Math.max(4, y1 - y0);
  const top = 64, bottom = 96;
  const fit = Math.max(8, Math.min((app.W - 16) / rw, (app.H - top - bottom) / rh));
  const scale = fit * app.view.zoom;
  const ox = (app.W - rw * scale) / 2 - x0 * scale + app.view.panX;
  const oy = top + (app.H - top - bottom - rh * scale) / 2 - y0 * scale + app.view.panY;
  return { ox, oy, scale };
}
function cellAt(px, py) {
  if (app.use3d && app.r3) return app.r3.cellAt(px, py);
  const v = viewTransform();
  const x = Math.floor((px - v.ox) / v.scale), y = Math.floor((py - v.oy) / v.scale);
  if (x < 0 || y < 0 || x >= app.statics.w || y >= app.statics.h) return null;
  return y * app.statics.w + x;
}

// ---------- HUD ----------
let shown = {};
function setText(el, v) { if (shown[el.id] !== v) { shown[el.id] = v; el.textContent = v; } }
function syncUi() {
  const s = app.snap;
  setText($('hud-score'), String(s.score));
  setText($('hud-clock'), clockText(s.day));
  $('hud-week').style.width = `${Math.round(s.wp * 100)}%`;
  updateTools(false);
  const open = s.offerOpen;
  if (open && $('offer').hidden && !app.over) showOffer();
  if (!open && !$('offer').hidden) $('offer').hidden = true;
  if (s.over && !app.over) gameOver();
}

const TOOLS = [
  { id: 'road', label: 'Road', kind: KIND.CONCRETE, icon: '<path d="M4 24 L12 10 L20 18 L28 6"/><circle cx="4" cy="24" r="2"/>' },
  { id: 'erase', label: 'Erase', icon: '<path d="M6 22 L20 8 L26 14 L14 26 H8 Z"/><path d="M12 16 L18 22"/>' },
  { id: 'light', label: 'Light', kind: KIND.LIGHT, icon: '<rect x="11" y="3" width="10" height="20" rx="4"/><circle cx="16" cy="9" r="1.6"/><circle cx="16" cy="17" r="1.6"/><path d="M16 23 V29"/>' },
  { id: 'round', label: 'Roundabout', kind: KIND.ROUNDABOUT, icon: '<circle cx="16" cy="16" r="7"/><path d="M16 3 V9 M16 23 V29 M3 16 H9 M23 16 H29"/>' },
  { id: 'motorway', label: 'Motorway', kind: KIND.MOTORWAY, icon: '<path d="M3 22 C10 22 10 8 16 8 C22 8 22 22 29 22"/><path d="M3 26 H29" stroke-dasharray="3 4"/>' },
];
const STOCK = [
  { label: 'Bridges', kind: KIND.BRIDGE, icon: '<path d="M3 20 H29"/><path d="M6 20 C6 10 26 10 26 20"/><path d="M11 14 V20 M16 12 V20 M21 14 V20"/>' },
  { label: 'Tunnels', kind: KIND.TUNNEL, icon: '<path d="M4 26 V16 C4 4 28 4 28 16 V26"/><path d="M11 26 V18 C11 11 21 11 21 18 V26"/>' },
];

function buildTools() {
  const nav = $('tools');
  nav.textContent = '';
  const mk = (t, clickable) => {
    const el = document.createElement(clickable ? 'button' : 'div');
    el.className = 'tool';
    el.dataset.tool = t.id || t.label.toLowerCase();
    el.dataset.kind = t.kind;
    el.innerHTML = `<svg viewBox="0 0 32 32" aria-hidden="true">${t.icon}</svg><span>${t.label}</span><span class="count">0</span>`;
    if (clickable) el.addEventListener('click', () => setTool(t.id));
    nav.appendChild(el);
  };
  TOOLS.forEach((t) => mk(t, true));
  STOCK.forEach((t) => mk(t, false));
}

function setTool(id) {
  app.tool = id;
  app.pending = null;
  app.hover = null;
  document.querySelectorAll('#tools .tool[data-tool]').forEach((el) => {
    if (el.tagName === 'BUTTON') el.classList.toggle('on', el.dataset.tool === id);
  });
}

function updateTools(force) {
  const up = app.snap && app.snap.up;
  if (!up) return;
  document.querySelectorAll('#tools .tool').forEach((el) => {
    const kind = Number(el.dataset.kind);
    const cnt = el.querySelector('.count');
    if (!cnt || Number.isNaN(kind)) { if (cnt) cnt.hidden = true; return; }
    const [owned, used] = up[kind];
    const free = owned - used;
    const label = owned >= 1000 ? '∞' : String(free);
    if (cnt.textContent !== label || force) cnt.textContent = label;
    el.classList.toggle('empty', free <= 0);
    if (el.tagName === 'DIV') el.hidden = owned === 0;
    if (el.tagName === 'BUTTON' && kind !== KIND.CONCRETE) el.disabled = false;
  });
}

let toastTimer = 0;
function toast(msg, ms = 1600) {
  if (!msg) return;
  const el = $('toast');
  el.textContent = msg;
  el.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), ms);
}

function showOffer() {
  const box = $('offer-cards');
  box.textContent = '';
  const names = ['Road tiles', 'Bridge', 'Motorway', 'Traffic light', 'Roundabout', 'Tunnel'];
  app.snap.offers.forEach((p, i) => {
    const b = document.createElement('button');
    b.dataset.offer = String(i);
    const big = document.createElement('span'); big.className = 'big';
    big.textContent = p[0] === KIND.CONCRETE ? `+${p[1]} road tiles` : `+${p[1]} ${names[p[0]]}${p[1] > 1 ? 's' : ''}`;
    const small = document.createElement('span'); small.className = 'small';
    small.textContent = p[0] === KIND.CONCRETE ? 'Plain and flexible' : (p[2] ? `and ${p[2]} road tiles` : packText(p));
    b.append(big, small);
    b.addEventListener('click', () => chooseOffer(i));
    box.appendChild(b);
  });
  $('offer').hidden = false;
}

function chooseOffer(i) {
  const code = app.core.chooseUpgrade(i);
  if (code === ERR.OK) { $('offer').hidden = true; app.snap = app.core.snapshot(); refreshStatics(); }
}

function gameOver() {
  app.over = true;
  const trips = app.snap.score;
  const c = app.core.cities[app.cityIndex];
  const prev = app.best[c.id] || 0;
  if (app.playMode === 0 && !app.challenge && trips > prev) { app.best[c.id] = trips; saveBest(app.best); }
  $('over-text').textContent = `${c.name}: ${trips} trips delivered` + (trips > prev ? ' — a new best!' : ` (best ${prev}).`);
  $('offer').hidden = true;
  $('over').hidden = false;
  setSpeed(0);
  if (features) features.onGameOver();
}

// ---------- input ----------
const pointers = new Map();
let drawing = null; // {mode:'road'|'erase', lastCell}
let panning = null;
let ctrlDown = false;
let eraseToggle = false;

function local(e) {
  const r = canvas.getBoundingClientRect();
  return [e.clientX - r.left, e.clientY - r.top];
}

function currentMode(e) {
  if (e && e.button === 2) return 'erase';
  if (ctrlDown || eraseToggle) return 'erase';
  return app.tool;
}

canvas.addEventListener('contextmenu', (e) => e.preventDefault());
canvas.addEventListener('pointerdown', (e) => {
  if (app.screen !== 'game' || app.over) return;
  canvas.setPointerCapture(e.pointerId);
  const [px, py] = local(e);
  pointers.set(e.pointerId, { x: px, y: py });
  if (pointers.size >= 2 || e.button === 1) {
    drawing = null;
    panning = { ids: [...pointers.keys()], px, py, pz: app.view.zoom, dist: pinchDist() };
    return;
  }
  const mode = currentMode(e);
  const cell = cellAt(px, py);
  if (cell == null) return;
  if (mode === 'road' || mode === 'erase') {
    drawing = { mode, last: cell };
    if (mode === 'erase') { app.core.beginErase(); afterEdit(app.core.eraseCell(cell)); if (features) features.markErased(); }
  } else if (mode === 'light') {
    afterEdit(app.core.toggleLight(cell), true);
  } else if (mode === 'round') {
    afterEdit(app.core.toggleRoundabout(cell), true);
  } else if (mode === 'motorway') {
    if (app.pending == null) { app.pending = cell; app.hover = { cell, color: '#3f93d8' }; }
    else {
      const a = app.pending; app.pending = null; app.hover = null;
      if (a !== cell) afterEdit(app.core.addMotorway(a, cell), true);
    }
  }
});

function pinchDist() {
  const p = [...pointers.values()];
  if (p.length < 2) return 0;
  return Math.hypot(p[0].x - p[1].x, p[0].y - p[1].y);
}

canvas.addEventListener('pointermove', (e) => {
  if (app.screen !== 'game') return;
  const [px, py] = local(e);
  const prev = pointers.get(e.pointerId);
  if (prev) { prev.x = px; prev.y = py; }
  if (panning) {
    if (pointers.size >= 2) {
      const d = pinchDist();
      if (panning.dist > 0 && d > 0) zoomAt(...centroid(), clampZoom(panning.pz * (d / panning.dist)));
      const [cx, cy] = centroid();
      app.view.panX += cx - panning.px; app.view.panY += cy - panning.py;
      panning.px = cx; panning.py = cy;
    } else if (prev) {
      app.view.panX += px - panning.px; app.view.panY += py - panning.py;
      panning.px = px; panning.py = py;
    }
    return;
  }
  const cell = cellAt(px, py);
  if (drawing && cell != null && cell !== drawing.last) {
    if (drawing.mode === 'road') {
      for (const c of stepsBetween(app.statics.w, drawing.last, cell)) {
        const code = app.core.addRoad(drawing.last, c);
        if (code === ERR.OK || code === ERR.EXISTS) { drawing.last = c; continue; }
        toast(errorText(code));
        break;
      }
      afterEdit(0);
    } else {
      for (const c of stepsBetween(app.statics.w, drawing.last, cell)) { app.core.eraseCell(c); drawing.last = c; }
      afterEdit(0);
    }
  }
  if (!drawing) {
    if (app.tool === 'motorway' && app.pending != null && cell != null) app.hover = { cell: app.pending, line: [app.pending, cell], color: '#3f93d8' };
    else app.hover = cell != null && e.pointerType === 'mouse' ? { cell } : null;
  }
});

function centroid() {
  const p = [...pointers.values()];
  return [(p[0].x + p[1].x) / 2, (p[0].y + p[1].y) / 2];
}

function endPointer(e) {
  pointers.delete(e.pointerId);
  if (pointers.size === 0) { drawing = null; panning = null; }
  else if (pointers.size === 1 && panning) {
    const p = [...pointers.values()][0];
    panning.px = p.x; panning.py = p.y; panning.dist = 0;
  }
}
canvas.addEventListener('pointerup', endPointer);
canvas.addEventListener('pointercancel', endPointer);
canvas.addEventListener('pointerleave', () => { if (!drawing) app.hover = null; });

function clampZoom(z) { return Math.max(0.6, Math.min(4, z)); }
function zoomAt(px, py, z) {
  if (app.use3d) { app.view.zoom = z; return; }
  const before = viewTransform();
  const wx = (px - before.ox) / before.scale, wy = (py - before.oy) / before.scale;
  app.view.zoom = z;
  const after = viewTransform();
  app.view.panX += px - (after.ox + wx * after.scale);
  app.view.panY += py - (after.oy + wy * after.scale);
}
canvas.addEventListener('wheel', (e) => {
  if (app.screen !== 'game') return;
  e.preventDefault();
  const [px, py] = local(e);
  zoomAt(px, py, clampZoom(app.view.zoom * Math.exp(-e.deltaY * 0.0015)));
}, { passive: false });

function afterEdit(code, showError) {
  if (showError && code !== ERR.OK) toast(errorText(code));
  app.snap = app.core.snapshot();
  if (app.snap.ver !== app.staticsVer) refreshStatics();
}

// ---------- keyboard and buttons ----------
window.addEventListener('keydown', (e) => {
  if (e.key === 'Control') ctrlDown = true;
  if (app.screen !== 'game') return;
  const offerOpen = !$('offer').hidden;
  if (offerOpen && (e.key === '1' || e.key === '2')) { chooseOffer(Number(e.key) - 1); return; }
  if (e.key === 'Tab') { e.preventDefault(); eraseToggle = !eraseToggle; toast(eraseToggle ? 'Erase mode' : 'Draw mode'); return; }
  if (e.key === ' ') { e.preventDefault(); setSpeed(app.speed === 0 ? 1 : 0); }
  else if (e.key === '1') setSpeed(0);
  else if (e.key === '2') setSpeed(1);
  else if (e.key === '3') setSpeed(2);
  else if (e.key === 'Escape') { if (!$('help').hidden) closeHelp(); else showMenu(); }
});
window.addEventListener('keyup', (e) => { if (e.key === 'Control') ctrlDown = false; });

for (let i = 0; i < 3; i++) $('speed-' + i).addEventListener('click', () => setSpeed(i));
$('btn-menu').addEventListener('click', showMenu);
$('btn-theme').addEventListener('click', () => setTheme(app.themeName === 'light' ? 'dark' : 'light'));
$('btn-theme-menu').addEventListener('click', () => { setTheme(app.themeName === 'light' ? 'dark' : 'light'); buildMenu(); });
$('btn-help').addEventListener('click', () => { $('help').hidden = false; });
function closeHelp() { $('help').hidden = true; if (app.speed === 0 && !app.over) setSpeed(1); }
$('btn-help-close').addEventListener('click', closeHelp);
$('btn-retry').addEventListener('click', () => startGame(app.cityIndex, undefined, app.playMode));
$('btn-daily').addEventListener('click', () => {
  const d = dailyChallenge(new Date(), app.core.cities);
  startGame(d.index, d.seed, 0);
});
$('btn-sound').addEventListener('click', () => {
  app.sound.unlock();
  app.sound.setMuted(!app.sound.muted);
  $('btn-sound').classList.toggle('off', app.sound.muted);
  savePref('muted', app.sound.muted);
});
app.sound.muted = Boolean(loadPref('muted', false));
$('btn-sound').classList.toggle('off', app.sound.muted);
document.addEventListener('pointerdown', () => app.sound.unlock(), { once: false });
$('btn-over-menu').addEventListener('click', showMenu);
$('unlock-all').addEventListener('change', (e) => { app.unlockAll = e.target.checked; savePref('unlockAll', app.unlockAll); buildMenu(); });

function resize() {
  if (app.r3) app.r3.resize(window.innerWidth, window.innerHeight, Math.min(window.devicePixelRatio || 1, 2.5));
  app.dpr = Math.min(window.devicePixelRatio || 1, 2.5);
  app.W = window.innerWidth; app.H = window.innerHeight;
  canvas.width = Math.round(app.W * app.dpr);
  canvas.height = Math.round(app.H * app.dpr);
}
window.addEventListener('resize', resize);
document.addEventListener('visibilitychange', () => { last = performance.now(); });

app.snapshotCanvas = () => {
  if (!app.use3d) return canvas;
  const cv = document.createElement('canvas');
  cv.width = canvas.width; cv.height = canvas.height;
  const c2 = cv.getContext('2d');
  c2.drawImage($('stage3d'), 0, 0, cv.width, cv.height);
  c2.drawImage(canvas, 0, 0);
  return cv;
};

/** Pins, overflow rings and hover hints, drawn flat on top of the 3D view so they always face the camera. */
function drawOverlay3d() {
  const r3 = app.r3, theme = THEMES[app.themeName], colors = GROUP_COLORS[app.themeName];
  ctx.setTransform(app.dpr, 0, 0, app.dpr, 0, 0);
  ctx.clearRect(0, 0, app.W, app.H);
  const w = app.statics.w;
  (app.statics.dests || []).forEach(([c, g, , ], i) => {
    const info = (app.snap.dests || [])[i];
    if (!info) return;
    const [pins, , ot, thr] = info;
    const cx = (c % w) + 1, cz = Math.floor(c / w) + 1;
    const base = r3.project(cx, 1.5, cz - 0.15);
    const ground = r3.project(cx, 0.1, cz);
    if (!base || !ground) return;
    const ppc = r3.pxPerCell(cx, cz);
    if (ot > 0.01) {
      const frac = Math.min(1, ot / 50), rx = ppc * 1.5, ry = rx * 0.78;
      ctx.lineCap = 'round';
      ctx.strokeStyle = 'rgba(0,0,0,0.14)'; ctx.lineWidth = Math.max(3, ppc * 0.16);
      ctx.beginPath(); ctx.ellipse(ground[0], ground[1], rx, ry, 0, 0, Math.PI * 2); ctx.stroke();
      ctx.strokeStyle = theme.alarm; ctx.lineWidth = Math.max(3, ppc * 0.14);
      ctx.beginPath(); ctx.ellipse(ground[0], ground[1], rx, ry, 0, -Math.PI / 2, -Math.PI / 2 + frac * Math.PI * 2); ctx.stroke();
    }
    for (let p = 0; p < pins; p++) {
      const col = p % 5, row = Math.floor(p / 5);
      const px = base[0] + (col - (Math.min(pins, 5) - 1) / 2) * ppc * 0.36;
      const py = base[1] - ppc * 0.45 - row * ppc * 0.38;
      const over = p >= thr;
      ctx.fillStyle = theme.shadow; ctx.beginPath(); ctx.arc(px + 1.5, py + 2.5, ppc * 0.17, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = '#fff'; ctx.beginPath(); ctx.arc(px, py, ppc * 0.17, 0, Math.PI * 2); ctx.fill();
      ctx.beginPath(); ctx.moveTo(px - ppc * 0.1, py + ppc * 0.1); ctx.lineTo(px, py + ppc * 0.27); ctx.lineTo(px + ppc * 0.1, py + ppc * 0.1); ctx.fill();
      ctx.fillStyle = over ? theme.alarm : colors[g];
      glyph(ctx, GROUP_SHAPES[g], px, py, ppc * 0.1); ctx.fill();
    }
  });
  const hv = app.hover;
  if (hv) {
    ctx.strokeStyle = hv.color || theme.ink; ctx.lineWidth = 2.5; ctx.lineCap = 'round';
    const at = (cell) => r3.cellCenterPx(cell);
    if (hv.line) {
      const a = at(hv.line[0]), b = at(hv.line[1]);
      ctx.setLineDash([8, 6]); ctx.beginPath(); ctx.moveTo(a[0], a[1]); ctx.lineTo(b[0], b[1]); ctx.stroke(); ctx.setLineDash([]);
    }
    if (hv.cell != null) {
      const p0 = at(hv.cell), ppc = r3.pxPerCell((hv.cell % w) + 0.5, Math.floor(hv.cell / w) + 0.5);
      ctx.beginPath(); ctx.ellipse(p0[0], p0[1], ppc * 0.45, ppc * 0.36, 0, 0, Math.PI * 2); ctx.stroke();
    }
  }
}

$('btn-render').addEventListener('click', () => {
  const want = !app.use3d;
  savePref('render3d', want);
  if (want) { initRenderer(); } else { app.use3d = false; $('stage3d').hidden = true; $('btn-render').textContent = '2D'; app.layer = null; }
  if (app.screen === 'game') { if (app.use3d) sync3d(); }
});

boot();
