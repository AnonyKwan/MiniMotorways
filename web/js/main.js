import { Core, ERR, KIND } from './core.js';
import {
  GROUP_COLORS, isUnlocked, lockText, packText, errorText, stepsBetween, clockText,
} from './model.js';
import { THEMES, buildTerrainLayer, drawScene } from './render.js';
import { loadBest, saveBest, loadPref, savePref } from './storage.js';

const $ = (id) => document.getElementById(id);
const canvas = $('stage');
const ctx = canvas.getContext('2d');
const params = new URLSearchParams(location.search);

const app = {
  core: null,
  screen: 'loading', // loading | menu | game
  cityIndex: 0,
  seed: 1,
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
  buildTools();
  buildMenu();
  app.screen = 'menu';
  requestAnimationFrame(frame);
  const want = params.get('city');
  const i = want ? app.core.cities.findIndex((c) => c.id === want) : -1;
  if (i >= 0) startGame(i);
  else showMenu();
}

function setTheme(name) {
  app.themeName = name;
  document.documentElement.dataset.theme = name;
  document.querySelector('meta[name="theme-color"]').content = THEMES[name].page;
  savePref('theme', name);
  app.layer = null;
}

// ---------- menu ----------
function buildMenu() {
  const grid = $('city-grid');
  grid.textContent = '';
  const cities = app.core.cities;
  const colors = GROUP_COLORS[app.themeName];
  cities.forEach((c, i) => {
    if (c.id === 'osaka' && !app.unlockAll && params.get('osaka') == null) return;
    const open = isUnlocked(c.id, app.best, app.unlockAll);
    const b = document.createElement('button');
    b.className = 'card';
    b.disabled = !open;
    b.dataset.city = c.id;
    const start = c.start.map(([k, n]) => `${n} ${['road tiles', 'bridge', 'motorway', 'light', 'roundabout', 'tunnel'][k]}`).join(' · ');
    const best = app.best[c.id] || 0;
    const sw = Array.from({ length: c.groups }, (_, g) => `<i style="background:${colors[g]}"></i>`).join('');
    b.innerHTML = `<span class="name"></span><span class="meta">${c.w}×${c.h} · ${start}</span>` +
      `<span class="swatch">${sw}</span>` +
      (open ? `<span class="best">${best ? `Best ${best} trips` : 'Not played yet'}</span>` : `<span class="lock">🔒 ${lockText(c.id, cities)}</span>`);
    b.querySelector('.name').textContent = c.name;
    b.addEventListener('click', () => startGame(i));
    grid.appendChild(b);
  });
  $('unlock-all').checked = app.unlockAll;
}

function showMenu() {
  app.screen = 'menu';
  $('menu').hidden = false;
  $('hud').hidden = true;
  $('tools').hidden = true;
  $('offer').hidden = true;
  $('over').hidden = true;
  buildMenu();
}

// ---------- game ----------
function startGame(cityIndex, seed) {
  app.cityIndex = cityIndex;
  app.seed = seed != null ? seed : (params.get('seed') ? Number(params.get('seed')) : (Date.now() & 0x7fffffff));
  app.core.newGame(cityIndex, app.seed);
  const c = app.core.cities[cityIndex];
  app.terrain = app.core.terrain();
  app.layer = null;
  app.snap = app.core.snapshot();
  app.statics = app.core.statics();
  app.staticsVer = app.snap.ver;
  app.over = false;
  app.view = { zoom: 1, panX: 0, panY: 0 };
  app.screen = 'game';
  setSpeed(1);
  setTool('road');
  $('menu').hidden = true;
  $('hud').hidden = false;
  $('tools').hidden = false;
  $('offer').hidden = true;
  $('over').hidden = true;
  $('hud-city').textContent = c.name;
  if (!app.helpSeen) { $('help').hidden = false; app.helpSeen = true; savePref('helpSeen', true); setSpeed(0); }
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
    syncUi();
    render(ts / 1000);
    acc += dt;
  }
  requestAnimationFrame(frame);
}

function refreshStatics() {
  app.statics = app.core.statics();
  app.staticsVer = app.snap.ver;
}

function render(t) {
  const { W, H } = app;
  if (!app.layer) {
    app.layer = buildTerrainLayer(app.statics.w, app.statics.h, app.terrain, THEMES[app.themeName], (w, h) => {
      const cv = document.createElement('canvas'); cv.width = w; cv.height = h; return cv;
    });
  }
  ctx.setTransform(app.dpr, 0, 0, app.dpr, 0, 0);
  const view = viewTransform();
  drawScene(ctx, W, H, view, {
    w: app.statics.w, h: app.statics.h, terrain: app.terrain, statics: app.statics, snap: app.snap,
    layer: app.layer, hover: app.hover,
  }, THEMES[app.themeName], app.themeName, app.snap.t);
}

app.cellCenter = (c) => {
  const v = viewTransform(), w = app.statics.w, r = canvas.getBoundingClientRect();
  return [r.left + v.ox + ((c % w) + 0.5) * v.scale, r.top + v.oy + (Math.floor(c / w) + 0.5) * v.scale];
};

function baseScale() {
  const w = app.statics ? app.statics.w : 40, h = app.statics ? app.statics.h : 22;
  const top = 64, bottom = 96;
  return Math.max(8, Math.min((app.W - 16) / w, (app.H - top - bottom) / h));
}
function viewTransform() {
  const w = app.statics.w, h = app.statics.h;
  const scale = baseScale() * app.view.zoom;
  const ox = (app.W - w * scale) / 2 + app.view.panX;
  const oy = 64 + (app.H - 64 - 96 - h * scale) / 2 + app.view.panY;
  return { ox, oy, scale };
}
function cellAt(px, py) {
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
    if (Number.isNaN(kind)) { cnt.hidden = true; return; }
    const [owned, used] = up[kind];
    const free = owned - used;
    const label = String(free);
    if (cnt.textContent !== label || force) cnt.textContent = label;
    el.classList.toggle('empty', free <= 0);
    if (el.tagName === 'DIV') el.hidden = owned === 0;
    if (el.tagName === 'BUTTON' && kind !== KIND.CONCRETE) el.disabled = false;
  });
}

let toastTimer = 0;
function toast(msg) {
  if (!msg) return;
  const el = $('toast');
  el.textContent = msg;
  el.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('show'), 1600);
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
  if (trips > prev) { app.best[c.id] = trips; saveBest(app.best); }
  $('over-text').textContent = `${c.name}: ${trips} trips delivered` + (trips > prev ? ' — a new best!' : ` (best ${prev}).`);
  $('offer').hidden = true;
  $('over').hidden = false;
  setSpeed(0);
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
    if (mode === 'erase') afterEdit(app.core.eraseCell(cell));
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
$('btn-retry').addEventListener('click', () => startGame(app.cityIndex));
$('btn-over-menu').addEventListener('click', showMenu);
$('unlock-all').addEventListener('change', (e) => { app.unlockAll = e.target.checked; savePref('unlockAll', app.unlockAll); buildMenu(); });

function resize() {
  app.dpr = Math.min(window.devicePixelRatio || 1, 2.5);
  app.W = window.innerWidth; app.H = window.innerHeight;
  canvas.width = Math.round(app.W * app.dpr);
  canvas.height = Math.round(app.H * app.dpr);
}
window.addEventListener('resize', resize);
document.addEventListener('visibilitychange', () => { last = performance.now(); });

boot();
