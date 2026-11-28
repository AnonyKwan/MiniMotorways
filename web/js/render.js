// Canvas drawing of the world. All artwork here is drawn in code: an original flat,
// map-like style (soft paper land, ink-blue water, painted roofs). No outside images.
import { GROUP_COLORS, GROUP_SHAPES } from './model.js';

export const THEMES = {
  light: {
    page: '#ece5d6', land: '#f3eee0', landTint: '#eee8d6', grass: '#cfdcae', grid: 'rgba(80,66,40,0.05)',
    water: '#9fcfe3', waterDeep: '#86bfd8', wave: 'rgba(255,255,255,0.45)', foam: '#e9f6fb', shore: '#6fa9c4',
    mountain: '#d3c7ae', mountainLight: '#e6dcc6', mountainDark: '#a79b82', snow: '#fbf8f0',
    tree: '#84b58d', treeDark: '#5d9470', treeLight: '#a6cfa9', trunk: '#8a6b4a',
    asphalt: '#59616d', asphaltEdge: '#3d444e', marking: 'rgba(255,255,255,0.55)',
    deck: '#f6f1e4', deckEdge: '#b9b09b', rail: '#d9d1be', tunnel: '#2e3138', tunnelLight: '#4a4f58',
    slab: '#c9c5b6', slabLine: 'rgba(255,255,255,0.8)',
    ink: '#2b2f36', pinFill: '#ffffff', alarm: '#e5483b', shadow: 'rgba(48,38,16,0.20)', motorway: '#3b4350', dry: 'rgba(255,255,255,0.35)',
  },
  dark: {
    page: '#12161c', land: '#1d242c', landTint: '#232b34', grass: '#27382f', grid: 'rgba(255,255,255,0.035)',
    water: '#163a52', waterDeep: '#112f44', wave: 'rgba(120,190,230,0.25)', foam: '#2a5d7c', shore: '#3d83a8',
    mountain: '#383a40', mountainLight: '#4a4d55', mountainDark: '#2a2c31', snow: '#aeb7c4',
    tree: '#2c5040', treeDark: '#20392f', treeLight: '#3b6a55', trunk: '#4b3b2c',
    asphalt: '#59626f', asphaltEdge: '#0c0f13', marking: 'rgba(255,255,255,0.28)',
    deck: '#8b95a3', deckEdge: '#2b3139', rail: '#6c7684', tunnel: '#0a0b0d', tunnelLight: '#252830',
    slab: '#323a44', slabLine: 'rgba(255,255,255,0.35)',
    ink: '#e9edf2', pinFill: '#1b2027', alarm: '#ff6a5c', shadow: 'rgba(0,0,0,0.5)', motorway: '#aeb8c6', dry: 'rgba(255,255,255,0.18)',
  },
};

export function glyph(ctx, shape, x, y, r) {
  ctx.beginPath();
  switch (shape) {
    case 'circle': ctx.arc(x, y, r, 0, Math.PI * 2); break;
    case 'square': ctx.rect(x - r * 0.85, y - r * 0.85, r * 1.7, r * 1.7); break;
    case 'triangle':
      ctx.moveTo(x, y - r); ctx.lineTo(x + r * 0.95, y + r * 0.7); ctx.lineTo(x - r * 0.95, y + r * 0.7); ctx.closePath(); break;
    case 'diamond':
      ctx.moveTo(x, y - r * 1.05); ctx.lineTo(x + r * 0.85, y); ctx.lineTo(x, y + r * 1.05); ctx.lineTo(x - r * 0.85, y); ctx.closePath(); break;
    case 'pentagon':
      for (let i = 0; i < 5; i++) {
        const a = -Math.PI / 2 + (i * 2 * Math.PI) / 5;
        ctx[i ? 'lineTo' : 'moveTo'](x + Math.cos(a) * r, y + Math.sin(a) * r);
      }
      ctx.closePath(); break;
    default:
      for (let i = 0; i < 10; i++) {
        const a = -Math.PI / 2 + (i * Math.PI) / 5;
        const rr = i % 2 ? r * 0.48 : r * 1.05;
        ctx[i ? 'lineTo' : 'moveTo'](x + Math.cos(a) * rr, y + Math.sin(a) * rr);
      }
      ctx.closePath();
  }
}

function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

/** Small deterministic hash in [0,1) for per-cell variety. */
export function cellHash(x, y, k = 0) {
  let h = (Math.imul(x + 1, 374761393) + Math.imul(y + 1, 668265263) + Math.imul(k + 7, 2246822519)) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return (h >>> 0) / 4294967296;
}

function shade(hex, amt) {
  const n = parseInt(hex.slice(1), 16);
  const f = (v) => Math.max(0, Math.min(255, Math.round(v + amt * 255)));
  return `rgb(${f(n >> 16)},${f((n >> 8) & 255)},${f(n & 255)})`;
}

/** Terrain painted once into an offscreen canvas (S pixels per cell). */
export function buildTerrainLayer(w, h, bytes, theme, makeCanvas) {
  const S = 32;
  const cv = makeCanvas(w * S, h * S);
  const c = cv.getContext('2d');
  const kindAt = (x, y) => (x < 0 || y < 0 || x >= w || y >= h ? -1 : bytes[y * w + x] & 3);
  c.fillStyle = theme.land;
  c.fillRect(0, 0, w * S, h * S);
  // meadow patches and soft tints
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (kindAt(x, y) !== 0) continue;
      const r = cellHash(x, y, 1);
      if (r < 0.3) {
        c.fillStyle = theme.landTint;
        c.beginPath();
        c.ellipse(x * S + S / 2, y * S + S / 2, S * 0.8, S * 0.7, cellHash(x, y, 30) * 3, 0, Math.PI * 2);
        c.fill();
      }
      if (r > 0.86) {
        c.fillStyle = theme.grass;
        c.globalAlpha = 0.55;
        c.beginPath();
        c.ellipse(x * S + S * (0.3 + 0.4 * cellHash(x, y, 2)), y * S + S * (0.3 + 0.4 * cellHash(x, y, 3)), S * 0.42, S * 0.3, cellHash(x, y, 4) * 3, 0, Math.PI * 2);
        c.fill();
        c.globalAlpha = 1;
      }
    }
  }
  // water with depth and wave strokes
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (kindAt(x, y) !== 1) continue;
      const deep = [kindAt(x + 1, y), kindAt(x - 1, y), kindAt(x, y + 1), kindAt(x, y - 1)].every((k) => k === 1);
      c.fillStyle = deep ? theme.waterDeep : theme.water;
      c.fillRect(x * S, y * S, S + 1, S + 1);
    }
  }
  c.strokeStyle = theme.wave;
  c.lineWidth = 2;
  c.lineCap = 'round';
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (kindAt(x, y) !== 1 || cellHash(x, y, 5) < 0.35) continue;
      const px = x * S + S * (0.2 + 0.3 * cellHash(x, y, 6)), py = y * S + S * (0.3 + 0.4 * cellHash(x, y, 7));
      c.beginPath();
      c.moveTo(px, py);
      c.quadraticCurveTo(px + S * 0.18, py - S * 0.12, px + S * 0.36, py);
      c.stroke();
    }
  }
  // shoreline: foam inside, ink line outside
  const edge = (x, y, dir) => {
    const x0 = x * S, y0 = y * S;
    if (dir === 'r') { c.moveTo(x0 + S, y0); c.lineTo(x0 + S, y0 + S); }
    else if (dir === 'l') { c.moveTo(x0, y0); c.lineTo(x0, y0 + S); }
    else if (dir === 'b') { c.moveTo(x0, y0 + S); c.lineTo(x0 + S, y0 + S); }
    else { c.moveTo(x0, y0); c.lineTo(x0 + S, y0); }
  };
  c.beginPath();
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (kindAt(x, y) !== 1) continue;
      if (kindAt(x + 1, y) !== 1 && kindAt(x + 1, y) !== -1) edge(x, y, 'r');
      if (kindAt(x - 1, y) !== 1 && kindAt(x - 1, y) !== -1) edge(x, y, 'l');
      if (kindAt(x, y + 1) !== 1 && kindAt(x, y + 1) !== -1) edge(x, y, 'b');
      if (kindAt(x, y - 1) !== 1 && kindAt(x, y - 1) !== -1) edge(x, y, 't');
    }
  }
  c.strokeStyle = theme.shore;
  c.lineWidth = 3;
  c.stroke();
  c.strokeStyle = theme.foam;
  c.lineWidth = 1.5;
  c.stroke();
  // railway: sleepers and two steel rails
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (!(bytes[y * w + x] & 8)) continue;
      c.fillStyle = theme.trunk;
      c.fillRect(x * S + S * 0.42, y * S + S * 0.12, S * 0.16, S * 0.76);
      c.fillStyle = theme.deckEdge;
      c.fillRect(x * S, y * S + S * 0.26, S + 1, S * 0.07);
      c.fillRect(x * S, y * S + S * 0.67, S + 1, S * 0.07);
    }
  }
  // hills: layered peaks with a lit face and a shaded face
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (kindAt(x, y) !== 2) continue;
      c.fillStyle = theme.mountain;
      c.fillRect(x * S, y * S, S + 1, S + 1);
    }
  }
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (kindAt(x, y) !== 2) continue;
      const peaks = 1 + Math.floor(cellHash(x, y, 8) * 2.2);
      for (let k = 0; k < peaks; k++) {
        const cx = x * S + S * (0.3 + 0.4 * cellHash(x, y, 9 + k)), base = y * S + S * (0.72 + 0.12 * cellHash(x, y, 12 + k));
        const pw = S * (0.34 + 0.12 * cellHash(x, y, 15 + k)), ph = S * (0.5 + 0.2 * cellHash(x, y, 18 + k));
        c.fillStyle = theme.mountainLight;
        c.beginPath(); c.moveTo(cx - pw, base); c.lineTo(cx, base - ph); c.lineTo(cx + pw * 0.1, base); c.closePath(); c.fill();
        c.fillStyle = theme.mountainDark;
        c.beginPath(); c.moveTo(cx + pw, base); c.lineTo(cx, base - ph); c.lineTo(cx + pw * 0.1, base); c.closePath(); c.fill();
        if (ph > S * 0.62) {
          c.fillStyle = theme.snow;
          c.beginPath(); c.moveTo(cx - pw * 0.28, base - ph * 0.72); c.lineTo(cx, base - ph); c.lineTo(cx + pw * 0.28, base - ph * 0.72);
          c.lineTo(cx + pw * 0.05, base - ph * 0.6); c.closePath(); c.fill();
        }
      }
    }
  }
  // trees: shadow, trunk, two-tone canopy
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (!(bytes[y * w + x] & 4)) continue;
      const cx = x * S + S * (0.45 + 0.1 * cellHash(x, y, 21)), cy = y * S + S * 0.55;
      const r = S * (0.26 + 0.07 * cellHash(x, y, 22));
      c.fillStyle = theme.shadow;
      c.beginPath(); c.ellipse(cx + S * 0.08, cy + r * 0.95, r * 0.95, r * 0.4, 0, 0, Math.PI * 2); c.fill();
      c.fillStyle = theme.trunk;
      c.fillRect(cx - S * 0.03, cy + r * 0.2, S * 0.06, r * 0.7);
      c.fillStyle = theme.treeDark;
      c.beginPath(); c.arc(cx, cy - r * 0.15, r, 0, Math.PI * 2); c.fill();
      c.fillStyle = theme.tree;
      c.beginPath(); c.arc(cx - r * 0.12, cy - r * 0.27, r * 0.82, 0, Math.PI * 2); c.fill();
      c.fillStyle = theme.treeLight;
      c.beginPath(); c.arc(cx - r * 0.3, cy - r * 0.45, r * 0.34, 0, Math.PI * 2); c.fill();
    }
  }
  c.strokeStyle = theme.grid;
  c.lineWidth = 1;
  c.beginPath();
  for (let x = 0; x <= w; x++) { c.moveTo(x * S, 0); c.lineTo(x * S, h * S); }
  for (let y = 0; y <= h; y++) { c.moveTo(0, y * S); c.lineTo(w * S, y * S); }
  c.stroke();
  return cv;
}

const easeOutBack = (t) => { const k = 1.70158; const u = t - 1; return 1 + (k + 1) * u * u * u + k * u * u; };

/** Scale for a building that has just appeared (0.4 s pop). */
function popScale(scene, key, now) {
  const born = scene.born;
  if (!born) return 1;
  if (!born.has(key)) born.set(key, now);
  const t = (now - born.get(key)) / 0.45;
  return t >= 1 ? 1 : Math.max(0.01, easeOutBack(Math.max(0, t)));
}

/**
 * @param view {{ox:number, oy:number, scale:number}} pixel offset and pixels per cell
 * @param scene {{w,h,terrain,statics,snap,layer,hover?,born?:Map}}
 */
export function drawScene(ctx, W, H, view, scene, theme, themeName, time, now = 0) {
  const { w, h, terrain, statics, snap, layer } = scene;
  const s = view.scale;
  ctx.fillStyle = theme.page;
  ctx.fillRect(0, 0, W, H);
  ctx.save();
  ctx.translate(view.ox, view.oy);
  ctx.shadowColor = theme.shadow;
  ctx.shadowBlur = s * 0.9;
  ctx.shadowOffsetY = s * 0.25;
  ctx.fillStyle = theme.land;
  roundRect(ctx, 0, 0, w * s, h * s, s * 0.35);
  ctx.fill();
  ctx.shadowColor = 'transparent';
  ctx.save();
  roundRect(ctx, 0, 0, w * s, h * s, s * 0.35);
  ctx.clip();
  if (layer) ctx.drawImage(layer, 0, 0, w * s, h * s);

  const P = (c) => [((c % w) + 0.5) * s, (Math.floor(c / w) + 0.5) * s];
  const kindAt = (c) => terrain[c] & 3;
  const roads = statics.roads || [];
  const stroke = (width, color, filter, dash) => {
    ctx.strokeStyle = color; ctx.lineWidth = width; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    ctx.setLineDash(dash || []);
    ctx.beginPath();
    for (const [a, b] of roads) {
      if (filter && !filter(a, b)) continue;
      const [ax, ay] = P(a), [bx, by] = P(b);
      ctx.moveTo(ax, ay); ctx.lineTo(bx, by);
    }
    ctx.stroke();
    ctx.setLineDash([]);
  };
  const wet = (a, b) => kindAt(a) === 1 || kindAt(b) === 1;
  const hill = (a, b) => kindAt(a) === 2 || kindAt(b) === 2;

  // bridge shadow on the water, then every road
  ctx.translate(s * 0.07, s * 0.14);
  stroke(s * 0.5, theme.shadow, wet);
  ctx.translate(-s * 0.07, -s * 0.14);
  stroke(s * 0.54, theme.asphaltEdge, (a, b) => !wet(a, b) && !hill(a, b));
  stroke(s * 0.42, theme.asphalt, (a, b) => !wet(a, b) && !hill(a, b));
  stroke(s * 0.58, theme.deckEdge, wet);
  stroke(s * 0.5, theme.deck, wet);
  stroke(s * 0.4, theme.asphalt, wet);
  stroke(s * 0.58, theme.tunnel, hill);
  stroke(s * 0.4, theme.tunnelLight, hill);
  stroke(Math.max(1, s * 0.035), theme.marking, null, [s * 0.14, s * 0.18]);
  // Expert mode: fresh asphalt that has not set yet
  for (const [c, f] of snap.dry || []) {
    const [x, y] = P(c);
    ctx.fillStyle = theme.dry;
    ctx.globalAlpha = 1 - f;
    ctx.beginPath(); ctx.arc(x, y, s * 0.26, 0, Math.PI * 2); ctx.fill();
    ctx.globalAlpha = 1;
  }

  // motorways: raised ribbons with shadow, deck and chevrons
  for (const [a, b] of statics.mw || []) {
    const [ax, ay] = P(a), [bx, by] = P(b);
    ctx.lineCap = 'round';
    ctx.strokeStyle = theme.shadow; ctx.lineWidth = s * 0.55;
    ctx.beginPath(); ctx.moveTo(ax + s * 0.14, ay + s * 0.24); ctx.lineTo(bx + s * 0.14, by + s * 0.24); ctx.stroke();
    ctx.strokeStyle = theme.deckEdge; ctx.lineWidth = s * 0.56;
    ctx.beginPath(); ctx.moveTo(ax, ay); ctx.lineTo(bx, by); ctx.stroke();
    ctx.strokeStyle = theme.motorway; ctx.lineWidth = s * 0.46;
    ctx.beginPath(); ctx.moveTo(ax, ay); ctx.lineTo(bx, by); ctx.stroke();
    ctx.strokeStyle = theme.marking; ctx.lineWidth = s * 0.04; ctx.setLineDash([s * 0.3, s * 0.22]);
    ctx.beginPath(); ctx.moveTo(ax, ay); ctx.lineTo(bx, by); ctx.stroke();
    ctx.setLineDash([]);
  }

  for (const c of statics.rounds || []) {
    const [x, y] = P(c);
    ctx.fillStyle = theme.shadow; ctx.beginPath(); ctx.arc(x + s * 0.05, y + s * 0.09, s * 0.7, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = theme.asphaltEdge; ctx.beginPath(); ctx.arc(x, y, s * 0.7, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = theme.asphalt; ctx.beginPath(); ctx.arc(x, y, s * 0.62, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = theme.grass; ctx.beginPath(); ctx.arc(x, y, s * 0.28, 0, Math.PI * 2); ctx.fill();
    ctx.strokeStyle = theme.treeDark; ctx.lineWidth = s * 0.05; ctx.stroke();
  }
  for (const c of statics.lights || []) {
    const [x, y] = P(c);
    const green = Math.floor(time / 3.7) % 2 === 0;
    ctx.fillStyle = theme.ink; roundRect(ctx, x + s * 0.2, y - s * 0.52, s * 0.2, s * 0.4, s * 0.08); ctx.fill();
    ctx.fillStyle = green ? '#ff5a4d' : '#3a2523'; ctx.beginPath(); ctx.arc(x + s * 0.3, y - s * 0.42, s * 0.05, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = green ? '#2c3a2e' : '#4bd37b'; ctx.beginPath(); ctx.arc(x + s * 0.3, y - s * 0.24, s * 0.05, 0, Math.PI * 2); ctx.fill();
  }

  const colors = GROUP_COLORS[themeName];
  const now2 = now || time;

  // ---- houses: painted roofs with a ridge and a driveway nub ----
  (statics.houses || []).forEach(([c, g], i) => {
    const [x, y] = P(c);
    const k = popScale(scene, 'h' + i, now2);
    ctx.save();
    ctx.translate(x, y); ctx.scale(k, k);
    ctx.fillStyle = theme.shadow;
    roundRect(ctx, -s * 0.34 + s * 0.06, -s * 0.34 + s * 0.1, s * 0.68, s * 0.68, s * 0.12); ctx.fill();
    ctx.fillStyle = shade(colors[g], -0.14);
    roundRect(ctx, -s * 0.36, -s * 0.36, s * 0.72, s * 0.72, s * 0.13); ctx.fill();
    ctx.fillStyle = colors[g];
    roundRect(ctx, -s * 0.32, -s * 0.36, s * 0.64, s * 0.62, s * 0.12); ctx.fill();
    ctx.fillStyle = shade(colors[g], 0.1);
    ctx.fillRect(-s * 0.3, -s * 0.34, s * 0.6, s * 0.08);
    ctx.strokeStyle = shade(colors[g], -0.2); ctx.lineWidth = s * 0.03;
    ctx.beginPath(); ctx.moveTo(-s * 0.28, -s * 0.05); ctx.lineTo(s * 0.28, -s * 0.05); ctx.stroke();
    ctx.fillStyle = theme.pinFill;
    glyph(ctx, GROUP_SHAPES[g], 0, s * 0.02, s * 0.13); ctx.fill();
    ctx.restore();
  });

  // ---- destinations: car park slab, building, timer ring, pins ----
  const dsnap = snap.dests || [];
  (statics.dests || []).forEach(([c, g, level, kind], i) => {
    const x0 = (c % w) * s, y0 = Math.floor(c / w) * s;
    const info = dsnap[i] || [0, level, 0, 6];
    const [pins, , ot, thr] = info;
    const cx = x0 + s, cy = y0 + s;
    const k = popScale(scene, 'd' + i, now2);
    ctx.save();
    ctx.translate(cx, cy); ctx.scale(k, k); ctx.translate(-cx, -cy);
    // slab with parking lines
    ctx.fillStyle = theme.shadow;
    roundRect(ctx, x0 + s * 0.1 + s * 0.06, y0 + s * 0.1 + s * 0.1, 2 * s - s * 0.2, 2 * s - s * 0.2, s * 0.2); ctx.fill();
    ctx.fillStyle = theme.slab;
    roundRect(ctx, x0 + s * 0.1, y0 + s * 0.1, 2 * s - s * 0.2, 2 * s - s * 0.2, s * 0.2); ctx.fill();
    ctx.strokeStyle = theme.slabLine; ctx.lineWidth = s * 0.03;
    ctx.beginPath();
    for (let j = 1; j < 6; j++) { ctx.moveTo(x0 + s * 0.1 + j * (s * 1.8 / 6), y0 + s * 1.66); ctx.lineTo(x0 + s * 0.1 + j * (s * 1.8 / 6), y0 + s * 1.9); }
    ctx.stroke();
    // the building itself
    const bs = s * 1.3;
    ctx.fillStyle = shade(colors[g], -0.16);
    if (level > 0) { ctx.beginPath(); ctx.arc(cx, cy - s * 0.12, bs * 0.56, 0, Math.PI * 2); ctx.fill(); }
    else { roundRect(ctx, cx - bs / 2, cy - s * 0.12 - bs / 2, bs, bs, s * 0.2); ctx.fill(); }
    ctx.fillStyle = colors[g];
    if (level > 0) { ctx.beginPath(); ctx.arc(cx, cy - s * 0.2, bs * 0.5, 0, Math.PI * 2); ctx.fill(); }
    else { roundRect(ctx, cx - bs / 2 + s * 0.04, cy - s * 0.2 - bs / 2, bs - s * 0.08, bs - s * 0.1, s * 0.17); ctx.fill(); }
    ctx.fillStyle = shade(colors[g], 0.1);
    if (level <= 0) ctx.fillRect(cx - bs / 2 + s * 0.12, cy - s * 0.2 - bs / 2 + s * 0.05, bs - s * 0.24, s * 0.07);
    ctx.fillStyle = theme.pinFill;
    glyph(ctx, GROUP_SHAPES[g], cx, cy - s * 0.2, s * 0.3); ctx.fill();
    if (kind === 1) { // station: a little train on the roof
      ctx.fillStyle = theme.ink; roundRect(ctx, cx - s * 0.36, cy + s * 0.2, s * 0.72, s * 0.18, s * 0.05); ctx.fill();
      ctx.beginPath(); ctx.arc(cx - s * 0.2, cy + s * 0.4, s * 0.05, 0, Math.PI * 2); ctx.arc(cx + s * 0.2, cy + s * 0.4, s * 0.05, 0, Math.PI * 2); ctx.fill();
    } else if (kind === 2) { // ferry terminal: a boat hull
      ctx.fillStyle = theme.ink; ctx.beginPath(); ctx.moveTo(cx - s * 0.36, cy + s * 0.2); ctx.lineTo(cx + s * 0.36, cy + s * 0.2); ctx.lineTo(cx + s * 0.22, cy + s * 0.4); ctx.lineTo(cx - s * 0.22, cy + s * 0.4); ctx.closePath(); ctx.fill();
    }
    ctx.restore();
    // pins: marker drops lined up above the building
    const perRow = 5;
    for (let p = 0; p < pins; p++) {
      const col = p % perRow, row = Math.floor(p / perRow);
      const px = cx - ((Math.min(pins, perRow) - 1) * s * 0.34) / 2 + col * s * 0.34;
      const py = y0 - s * 0.2 - row * s * 0.36;
      const over = p >= thr;
      ctx.fillStyle = theme.shadow;
      ctx.beginPath(); ctx.arc(px + s * 0.03, py + s * 0.05, s * 0.15, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = theme.pinFill;
      ctx.beginPath(); ctx.arc(px, py, s * 0.16, 0, Math.PI * 2); ctx.fill();
      ctx.beginPath(); ctx.moveTo(px - s * 0.1, py + s * 0.1); ctx.lineTo(px, py + s * 0.26); ctx.lineTo(px + s * 0.1, py + s * 0.1); ctx.fill();
      ctx.fillStyle = over ? theme.alarm : colors[g];
      glyph(ctx, GROUP_SHAPES[g], px, py, s * 0.09); ctx.fill();
    }
    if (ot > 0.01) {
      const frac = Math.min(1, ot / 50);
      ctx.lineCap = 'round';
      ctx.strokeStyle = 'rgba(0,0,0,0.12)'; ctx.lineWidth = Math.max(3, s * 0.14);
      ctx.beginPath(); ctx.arc(cx, cy, s * 1.34, 0, Math.PI * 2); ctx.stroke();
      ctx.strokeStyle = theme.alarm; ctx.lineWidth = Math.max(3, s * 0.12);
      ctx.beginPath(); ctx.arc(cx, cy, s * 1.34, -Math.PI / 2, -Math.PI / 2 + frac * Math.PI * 2); ctx.stroke();
    }
  });

  // ---- ferry route, trains and boats ----
  const bt = statics.boat;
  if (bt && bt.length === 3) {
    ctx.strokeStyle = theme.foam; ctx.lineWidth = s * 0.06; ctx.setLineDash([s * 0.3, s * 0.3]);
    ctx.beginPath(); ctx.moveTo((bt[1] + 0.5) * s, (bt[0] + 0.5) * s); ctx.lineTo((bt[2] + 0.5) * s, (bt[0] + 0.5) * s); ctx.stroke();
    ctx.setLineDash([]);
  }
  for (const [kind, tx, ty, dir] of snap.transit || []) {
    ctx.save();
    ctx.translate(tx * s, ty * s);
    if (kind === 0) {
      for (let k = 0; k < 3; k++) {
        const ox = -dir * k * s * 1.05;
        ctx.fillStyle = theme.shadow; roundRect(ctx, ox - s * 0.5 + s * 0.04, -s * 0.2 + s * 0.08, s * 1.0, s * 0.4, s * 0.1); ctx.fill();
        ctx.fillStyle = k === 0 ? theme.ink : theme.deckEdge; roundRect(ctx, ox - s * 0.5, -s * 0.2, s * 1.0, s * 0.4, s * 0.1); ctx.fill();
        ctx.fillStyle = theme.deck; ctx.fillRect(ox - s * 0.34, -s * 0.1, s * 0.68, s * 0.12);
      }
    } else {
      ctx.fillStyle = theme.shadow; ctx.beginPath(); ctx.ellipse(s * 0.05, s * 0.1, s * 0.62, s * 0.26, 0, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = theme.ink; ctx.beginPath();
      ctx.moveTo(-s * 0.6 * dir, -s * 0.2); ctx.lineTo(s * 0.6 * dir, -s * 0.2); ctx.lineTo(s * 0.45 * dir, s * 0.22); ctx.lineTo(-s * 0.45 * dir, s * 0.22); ctx.closePath(); ctx.fill();
      ctx.fillStyle = theme.deck; ctx.fillRect(-s * 0.2, -s * 0.12, s * 0.4, s * 0.16);
    }
    ctx.restore();
  }

  // ---- cars ----
  for (const [x, y, dx, dy, g, parked] of snap.cars || []) {
    if (parked) continue;
    ctx.save();
    ctx.translate(x * s, y * s);
    ctx.rotate(Math.atan2(dy, dx));
    ctx.fillStyle = theme.shadow;
    roundRect(ctx, -s * 0.2 + s * 0.04, -s * 0.12 + s * 0.06, s * 0.4, s * 0.24, s * 0.08); ctx.fill();
    ctx.fillStyle = shade(colors[g], -0.12);
    roundRect(ctx, -s * 0.21, -s * 0.125, s * 0.42, s * 0.25, s * 0.08); ctx.fill();
    ctx.fillStyle = colors[g];
    roundRect(ctx, -s * 0.2, -s * 0.12, s * 0.4, s * 0.22, s * 0.08); ctx.fill();
    ctx.fillStyle = 'rgba(30,40,55,0.55)';
    roundRect(ctx, -s * 0.02, -s * 0.085, s * 0.13, s * 0.17, s * 0.03); ctx.fill();
    ctx.fillStyle = 'rgba(255,244,190,0.9)';
    ctx.fillRect(s * 0.17, -s * 0.09, s * 0.03, s * 0.05); ctx.fillRect(s * 0.17, s * 0.04, s * 0.03, s * 0.05);
    ctx.restore();
  }

  // the part of the map that has not opened yet fades into the paper
  const rg = scene.region;
  if (rg) {
    ctx.fillStyle = theme.page;
    ctx.globalAlpha = 0.78;
    ctx.fillRect(0, 0, w * s, rg[1] * s);
    ctx.fillRect(0, rg[3] * s, w * s, (h - rg[3]) * s);
    ctx.fillRect(0, rg[1] * s, rg[0] * s, (rg[3] - rg[1]) * s);
    ctx.fillRect(rg[2] * s, rg[1] * s, (w - rg[2]) * s, (rg[3] - rg[1]) * s);
    ctx.globalAlpha = 1;
    ctx.strokeStyle = theme.deckEdge; ctx.lineWidth = 2; ctx.setLineDash([s * 0.3, s * 0.2]);
    ctx.strokeRect(rg[0] * s, rg[1] * s, (rg[2] - rg[0]) * s, (rg[3] - rg[1]) * s);
    ctx.setLineDash([]);
  }

  // ---- interaction hints ----
  const hv = scene.hover;
  if (hv) {
    ctx.strokeStyle = hv.color || theme.ink; ctx.lineWidth = Math.max(2, s * 0.06); ctx.lineCap = 'round';
    if (hv.line) {
      const [ax, ay] = P(hv.line[0]), [bx, by] = P(hv.line[1]);
      ctx.setLineDash([s * 0.2, s * 0.15]);
      ctx.beginPath(); ctx.moveTo(ax, ay); ctx.lineTo(bx, by); ctx.stroke();
      ctx.setLineDash([]);
    }
    if (hv.cell != null) {
      const [x, y] = P(hv.cell);
      ctx.beginPath(); ctx.arc(x, y, s * 0.42, 0, Math.PI * 2); ctx.stroke();
    }
  }
  ctx.restore();
  // thin paper border
  ctx.strokeStyle = theme.deckEdge; ctx.globalAlpha = 0.5; ctx.lineWidth = 2;
  roundRect(ctx, 0, 0, w * s, h * s, s * 0.35); ctx.stroke();
  ctx.globalAlpha = 1;
  ctx.restore();
}
