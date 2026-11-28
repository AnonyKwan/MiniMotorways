// Pure helpers shared by the page and the Node tests: unlocking, grid maths, messages.
import { ERR, KIND, KIND_NAMES } from './core.js';

export const GROUP_NAMES = ['Coral', 'Teal', 'Amber', 'Violet', 'Sky', 'Rose'];
export const GROUP_COLORS = {
  light: ['#ef6a58', '#1fa89b', '#f0b13a', '#8566d6', '#3f93d8', '#e5659f'],
  dark: ['#ff7f6b', '#3dd1c1', '#ffc857', '#a58bf0', '#63b1f5', '#ff82b8'],
};
/** Distinct marks so colours never carry the meaning alone. */
export const GROUP_SHAPES = ['circle', 'square', 'triangle', 'diamond', 'pentagon', 'star'];

// ---- unlocking (doc 01.6) ----
const STARTERS = ['losangeles', 'beijing', 'tokyo', 'daressalaam', 'london'];
const TIERS = [
  { need: 200, ids: ['riodejaneiro', 'manila', 'moscow', 'zurich', 'munich'] },
  { need: 250, ids: ['chiangmai', 'dubai', 'warsaw', 'mexicocity', 'wellington'] },
  { need: 300, ids: ['cairns', 'lisbon', 'reykjavik', 'busan', 'vancouver'] },
  { need: 350, ids: ['hongkong', 'singapore', 'istanbul', 'copenhagen', 'capetown'] },
];

/** @returns {{need:number, from:string[]} | null} null when always open */
export function unlockRule(id) {
  if (STARTERS.includes(id)) return null;
  if (id === 'osaka') return { need: Infinity, from: [] }; // hidden
  if (id === 'mumbai') return { need: 300, from: ['london'] };
  if (id === 'newyorkcity') return { need: 400, from: ['mumbai'] };
  for (let i = 0; i < TIERS.length; i++) {
    if (TIERS[i].ids.includes(id)) return { need: TIERS[i].need, from: i === 0 ? STARTERS : TIERS[i - 1].ids };
  }
  return null;
}

/** @param {Record<string, number>} best best trips per city id */
export function isUnlocked(id, best, unlockAll = false) {
  if (unlockAll) return true;
  const rule = unlockRule(id);
  if (!rule) return true;
  return rule.from.some((f) => (best[f] || 0) >= rule.need);
}

export function lockText(id, cities) {
  const rule = unlockRule(id);
  if (!rule || !isFinite(rule.need)) return '';
  const name = (i) => (cities.find((c) => c.id === i) || { name: i }).name;
  return `Deliver ${rule.need} trips in ${rule.from.map(name).join(', ')}`;
}

// ---- weekly packs ----
export function packText(pack) {
  const [kind, amount, extra] = pack;
  const parts = [];
  if (kind === KIND.CONCRETE) parts.push(`${amount} road tiles`);
  else {
    parts.push(`${amount} ${KIND_NAMES[kind].toLowerCase()}${amount > 1 && kind !== KIND.CONCRETE ? 's' : ''}`);
    if (extra) parts.push(`${extra} road tiles`);
  }
  return parts.join(' + ');
}

// ---- messages for build errors ----
export function errorText(code) {
  switch (code) {
    case ERR.NO_CONCRETE: return 'Out of road tiles';
    case ERR.NO_BRIDGE: return 'You need a bridge to cross water';
    case ERR.NO_TUNNEL: return 'You need a tunnel to go through hills';
    case ERR.BLOCKED: return "Roads can't join two buildings";
    case ERR.CROSSING: return "Diagonal roads can't cross";
    case ERR.NO_UPGRADE: return 'None of those left';
    case ERR.BAD_PLACEMENT: return "Can't place that here";
    case ERR.TOO_SHORT: return 'A motorway needs at least 2 tiles';
    case ERR.PERMANENT: return "That road has set and can't be removed";
    default: return '';
  }
}

// ---- grid maths ----
/** Cells visited going from a to b, one 8-direction step at a time (excluding a). */
export function stepsBetween(w, a, b) {
  let x = a % w, y = Math.floor(a / w);
  const bx = b % w, by = Math.floor(b / w);
  const out = [];
  let guard = 0;
  while ((x !== bx || y !== by) && guard++ < 512) {
    const dx = bx - x, dy = by - y;
    // go diagonal only while both axes still need to move by a similar amount
    const ax = Math.abs(dx), ay = Math.abs(dy);
    let sx = Math.sign(dx), sy = Math.sign(dy);
    if (ax > 2 * ay) sy = 0;
    else if (ay > 2 * ax) sx = 0;
    x += sx; y += sy;
    out.push(y * w + x);
  }
  return out;
}

export function clockText(day) {
  const d = Math.floor(day);
  const weekNo = Math.floor(d / 7) + 1;
  const names = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  return `Week ${weekNo} · ${names[d % 7]}`;
}

export function pinSlots(count, perSide = 4) {
  // positions (0..1 around the building) for pin markers
  const out = [];
  for (let i = 0; i < count; i++) out.push(i);
  return out.map((i) => ({ i, side: Math.floor(i / perSide) % 4, k: i % perSide }));
}

// ---- game modes ----
export const MODES = [
  { id: 0, name: 'Classic', blurb: 'The standard game. Roads can be changed any time.' },
  { id: 1, name: 'Expert', blurb: 'Roads set after 40 seconds and then stay for good.' },
  { id: 2, name: 'Creative', blurb: 'Every tool, no limits, no game over. Just build.' },
  { id: 3, name: 'Endless', blurb: 'No game over. Deliver far to earn efficiency and a new tool at each milestone.' },
];

/** Stable 32-bit hash of a string (FNV-1a). */
export function hashString(str) {
  let h = 0x811c9dc5;
  for (let i = 0; i < str.length; i++) { h ^= str.charCodeAt(i); h = Math.imul(h, 0x01000193); }
  return h >>> 0;
}

/** The same map and seed for everybody on a given UTC day. */
export function dailyChallenge(date, cities) {
  const d = typeof date === 'string' ? date : date.toISOString().slice(0, 10);
  const pool = cities.map((c, i) => ({ c, i })).filter((x) => x.c.id !== 'osaka');
  const h = hashString('little-lanes-' + d);
  const pick = pool[h % pool.length];
  return { date: d, index: pick.i, city: pick.c, seed: (hashString('seed-' + d) & 0x7fffffff) || 1 };
}
