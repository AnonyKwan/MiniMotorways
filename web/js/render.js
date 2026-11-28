// Canvas drawing of the world. Original flat "paper map" look; no outside artwork.
import { GROUP_COLORS, GROUP_SHAPES } from './model.js';

export const THEMES = {
  light: {
    page: '#efe9dc', land: '#f4efe3', grid: 'rgba(60,50,30,0.06)', water: '#a9d3e4', waterEdge: '#8cbfd4',
    mountain: '#cbbfa8', mountainInk: '#a99d86', tree: '#7fb08a', treeInk: '#5f9470',
    roadEdge: '#4d5560', road: '#6b7480', roadLine: 'rgba(255,255,255,0.35)', bridge: '#f7f3ea', tunnel: '#5a5347',
    ink: '#2b2f36', panel: '#fffdf8', pinFill: '#ffffff', alarm: '#e5483b', shadow: 'rgba(40,30,10,0.18)', motorway: '#3d4450',
  },
  dark: {
    page: '#161a20', land: '#1f252d', grid: 'rgba(255,255,255,0.04)', water: '#173a52', waterEdge: '#1e4a66',
    mountain: '#38383a', mountainInk: '#4b4b4e', tree: '#27493a', treeInk: '#33604b',
    roadEdge: '#0e1114', road: '#59626e', roadLine: 'rgba(255,255,255,0.18)', bridge: '#8b95a3', tunnel: '#0c0d0f',
    ink: '#e9edf2', panel: '#232a33', pinFill: '#1b2027', alarm: '#ff6a5c', shadow: 'rgba(0,0,0,0.45)', motorway: '#aeb8c6',
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
    default: // star
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

/** Terrain drawn once into an offscreen canvas (one pixel block per cell). */
export function buildTerrainLayer(w, h, bytes, theme, makeCanvas) {
  const S = 24;
  const cv = makeCanvas(w * S, h * S);
  const c = cv.getContext('2d');
  c.fillStyle = theme.land;
  c.fillRect(0, 0, w * S, h * S);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const b = bytes[y * w + x];
      const kind = b & 3;
      if (kind === 1) {
        c.fillStyle = theme.water;
        c.fillRect(x * S, y * S, S + 0.5, S + 0.5);
      } else if (kind === 2) {
        c.fillStyle = theme.mountain;
        c.fillRect(x * S, y * S, S + 0.5, S + 0.5);
      }
    }
  }
  // shoreline: a darker edge where water meets land
  c.strokeStyle = theme.waterEdge;
  c.lineWidth = 2;
  const at = (x, y) => (x < 0 || y < 0 || x >= w || y >= h ? 1 : bytes[y * w + x] & 3);
  c.beginPath();
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if ((bytes[y * w + x] & 3) !== 1) continue;
      if (at(x + 1, y) !== 1) { c.moveTo((x + 1) * S, y * S); c.lineTo((x + 1) * S, (y + 1) * S); }
      if (at(x - 1, y) !== 1) { c.moveTo(x * S, y * S); c.lineTo(x * S, (y + 1) * S); }
      if (at(x, y + 1) !== 1) { c.moveTo(x * S, (y + 1) * S); c.lineTo((x + 1) * S, (y + 1) * S); }
      if (at(x, y - 1) !== 1) { c.moveTo(x * S, y * S); c.lineTo((x + 1) * S, y * S); }
    }
  }
  c.stroke();
  // hills and trees
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const b = bytes[y * w + x];
      const cx = x * S + S / 2, cy = y * S + S / 2;
      if ((b & 3) === 2) {
        c.fillStyle = theme.mountainInk;
        c.beginPath();
        c.moveTo(cx, cy - S * 0.3); c.lineTo(cx + S * 0.34, cy + S * 0.26); c.lineTo(cx - S * 0.34, cy + S * 0.26);
        c.closePath(); c.fill();
      } else if (b & 4) {
        c.fillStyle = theme.tree;
        c.beginPath(); c.arc(cx, cy, S * 0.3, 0, Math.PI * 2); c.fill();
        c.strokeStyle = theme.treeInk; c.lineWidth = 1.5; c.stroke();
      }
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

/**
 * @param view {{ox:number, oy:number, scale:number}} pixel offset and pixels per cell
 * @param scene {{w:number, h:number, terrain:Uint8Array, statics:object, snap:object, layer:any, hover?:object}}
 */
export function drawScene(ctx, W, H, view, scene, theme, themeName, time) {
  const { w, h, terrain, statics, snap, layer } = scene;
  const s = view.scale;
  ctx.fillStyle = theme.page;
  ctx.fillRect(0, 0, W, H);
  ctx.save();
  ctx.translate(view.ox, view.oy);
  // map card with soft shadow
  ctx.shadowColor = theme.shadow;
  ctx.shadowBlur = s * 0.8;
  ctx.shadowOffsetY = s * 0.2;
  ctx.fillStyle = theme.land;
  ctx.fillRect(0, 0, w * s, h * s);
  ctx.shadowColor = 'transparent';
  if (layer) ctx.drawImage(layer, 0, 0, w * s, h * s);

  const cxy = (c) => [(c % w) + 0.5, Math.floor(c / w) + 0.5];
  const P = (c) => { const [x, y] = cxy(c); return [x * s, y * s]; };
  const kindAt = (c) => terrain[c] & 3;

  // ---- roads ----
  const roads = statics.roads || [];
  const road = (width, color, filter) => {
    ctx.strokeStyle = color; ctx.lineWidth = width; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    ctx.beginPath();
    for (const [a, b] of roads) {
      if (filter && !filter(a, b)) continue;
      const [ax, ay] = P(a), [bx, by] = P(b);
      ctx.moveTo(ax, ay); ctx.lineTo(bx, by);
    }
    ctx.stroke();
  };
  road(s * 0.5, theme.roadEdge);
  road(s * 0.36, theme.road);
  road(s * 0.5, theme.bridge, (a, b) => kindAt(a) === 1 || kindAt(b) === 1);
  road(s * 0.36, theme.road, (a, b) => kindAt(a) === 1 || kindAt(b) === 1);
  road(s * 0.5, theme.tunnel, (a, b) => kindAt(a) === 2 || kindAt(b) === 2);
  road(s * 0.3, theme.road, (a, b) => kindAt(a) === 2 || kindAt(b) === 2);
  // centre line
  ctx.setLineDash([s * 0.12, s * 0.16]);
  road(Math.max(1, s * 0.03), theme.roadLine);
  ctx.setLineDash([]);

  // ---- motorways (raised, with shadow) ----
  for (const [a, b] of statics.mw || []) {
    const [ax, ay] = P(a), [bx, by] = P(b);
    ctx.lineCap = 'round';
    ctx.strokeStyle = theme.shadow; ctx.lineWidth = s * 0.55;
    ctx.beginPath(); ctx.moveTo(ax + s * 0.12, ay + s * 0.18); ctx.lineTo(bx + s * 0.12, by + s * 0.18); ctx.stroke();
    ctx.strokeStyle = theme.motorway; ctx.lineWidth = s * 0.5;
    ctx.beginPath(); ctx.moveTo(ax, ay); ctx.lineTo(bx, by); ctx.stroke();
    ctx.strokeStyle = theme.bridge; ctx.lineWidth = s * 0.05; ctx.setLineDash([s * 0.25, s * 0.2]);
    ctx.beginPath(); ctx.moveTo(ax, ay); ctx.lineTo(bx, by); ctx.stroke();
    ctx.setLineDash([]);
  }

  // ---- junction furniture ----
  for (const c of statics.rounds || []) {
    const [x, y] = P(c);
    ctx.strokeStyle = theme.roadEdge; ctx.lineWidth = s * 0.5;
    ctx.beginPath(); ctx.arc(x, y, s * 0.55, 0, Math.PI * 2); ctx.stroke();
    ctx.strokeStyle = theme.road; ctx.lineWidth = s * 0.36;
    ctx.beginPath(); ctx.arc(x, y, s * 0.55, 0, Math.PI * 2); ctx.stroke();
    ctx.fillStyle = theme.tree;
    ctx.beginPath(); ctx.arc(x, y, s * 0.3, 0, Math.PI * 2); ctx.fill();
  }
  for (const c of statics.lights || []) {
    const [x, y] = P(c);
    const phase = Math.floor(time / 3.7) % 2;
    ctx.fillStyle = theme.ink;
    ctx.beginPath(); ctx.arc(x + s * 0.28, y - s * 0.28, s * 0.17, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = phase ? '#4bd37b' : '#ff5a4d';
    ctx.beginPath(); ctx.arc(x + s * 0.28, y - s * 0.28, s * 0.1, 0, Math.PI * 2); ctx.fill();
  }

  const colors = GROUP_COLORS[themeName];

  // ---- houses ----
  for (const [c, g] of statics.houses || []) {
    const [x, y] = P(c);
    ctx.fillStyle = theme.shadow;
    roundRect(ctx, x - s * 0.36 + s * 0.05, y - s * 0.36 + s * 0.09, s * 0.72, s * 0.72, s * 0.14); ctx.fill();
    ctx.fillStyle = colors[g];
    roundRect(ctx, x - s * 0.36, y - s * 0.36, s * 0.72, s * 0.72, s * 0.14); ctx.fill();
    ctx.fillStyle = theme.pinFill;
    glyph(ctx, GROUP_SHAPES[g], x, y, s * 0.17); ctx.fill();
  }

  // ---- destinations ----
  const dsnap = snap.dests || [];
  (statics.dests || []).forEach(([c, g, level], i) => {
    const x0 = (c % w) * s, y0 = Math.floor(c / w) * s;
    const info = dsnap[i] || [0, level, 0, 6];
    const [pins, , ot, thr] = info;
    const pad = s * 0.08;
    ctx.fillStyle = theme.shadow;
    if (level > 0) { ctx.beginPath(); ctx.arc(x0 + s + s * 0.06, y0 + s + s * 0.1, s - pad, 0, Math.PI * 2); ctx.fill(); }
    else { roundRect(ctx, x0 + pad + s * 0.06, y0 + pad + s * 0.1, 2 * s - 2 * pad, 2 * s - 2 * pad, s * 0.22); ctx.fill(); }
    ctx.fillStyle = colors[g];
    if (level > 0) { ctx.beginPath(); ctx.arc(x0 + s, y0 + s, s - pad, 0, Math.PI * 2); ctx.fill(); }
    else { roundRect(ctx, x0 + pad, y0 + pad, 2 * s - 2 * pad, 2 * s - 2 * pad, s * 0.22); ctx.fill(); }
    ctx.fillStyle = theme.pinFill;
    glyph(ctx, GROUP_SHAPES[g], x0 + s, y0 + s, s * 0.5); ctx.fill();
    // pins: little markers on the car park edge
    for (let k = 0; k < pins; k++) {
      const col = k % 5, row = Math.floor(k / 5);
      const px = x0 + s * 0.3 + col * s * 0.35, py = y0 + 2 * s + s * 0.16 + row * s * 0.3;
      const over = k >= thr;
      ctx.fillStyle = over ? theme.alarm : colors[g];
      glyph(ctx, GROUP_SHAPES[g], px, py, s * 0.13); ctx.fill();
    }
    if (ot > 0.01) {
      const frac = Math.min(1, ot / 50);
      ctx.strokeStyle = theme.alarm; ctx.lineWidth = Math.max(2, s * 0.12); ctx.lineCap = 'round';
      ctx.beginPath(); ctx.arc(x0 + s, y0 + s, s * 1.32, -Math.PI / 2, -Math.PI / 2 + frac * Math.PI * 2); ctx.stroke();
    }
  });

  // ---- cars ----
  for (const [x, y, dx, dy, g, parked] of snap.cars || []) {
    if (parked) continue;
    ctx.save();
    ctx.translate(x * s, y * s);
    ctx.rotate(Math.atan2(dy, dx));
    ctx.fillStyle = theme.shadow;
    roundRect(ctx, -s * 0.2 + s * 0.03, -s * 0.12 + s * 0.05, s * 0.4, s * 0.24, s * 0.07); ctx.fill();
    ctx.fillStyle = colors[g];
    roundRect(ctx, -s * 0.2, -s * 0.12, s * 0.4, s * 0.24, s * 0.07); ctx.fill();
    ctx.fillStyle = 'rgba(255,255,255,0.75)';
    ctx.fillRect(s * 0.02, -s * 0.09, s * 0.1, s * 0.18);
    ctx.restore();
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
}
