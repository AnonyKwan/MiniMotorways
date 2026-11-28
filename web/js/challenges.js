// Daily, weekly and city challenges: sets of rule changes (see doc 09.3), written here with our own names.
import { hashString } from './model.js';

// Modifier keys understood by the core: [key, a, b]. See src/ffi.rs `ll_mod`.
const K = { START: 0, PREVENT: 1, AWARD: 3, CHOICES: 4, FREE: 5, DEMAND: 7, NEVER_UP: 9, ALL_UP: 10, UNLIMITED: 17, TREES: 18, SPEED: 21, STRAIGHT: 22, DIAGONAL: 23, UPCOST: 24 };
const BRIDGE = 1, MOTORWAY = 2, LIGHT = 3, ROUND = 4, TUNNEL = 5;
const allGroups = (b) => [0, 1, 2, 3, 4, 5].map((g) => [K.DEMAND, g, b]);

/** family keeps two presets that fight each other out of the same day */
export const PRESETS = [
  { id: 'pricey-straights', title: 'Pricey Pavement', desc: 'Straight roads cost 2 tiles.', family: 'cost', mods: [[K.STRAIGHT, 0, 2]] },
  { id: 'pricey-diagonals', title: 'Corner Tax', desc: 'Diagonal roads cost 2 tiles.', family: 'cost', mods: [[K.DIAGONAL, 0, 2]] },
  { id: 'toll-bridges', title: 'Toll Bridges', desc: 'Road on bridges costs double.', family: 'cost', mods: [[K.UPCOST, BRIDGE, 2]] },
  { id: 'free-bridges', title: 'Free Crossings', desc: 'Roads on bridges are free.', family: 'cost', mods: [[K.UPCOST, BRIDGE, 0]] },
  { id: 'free-tunnels', title: 'Free Burrows', desc: 'Roads in tunnels are free.', family: 'cost', mods: [[K.UPCOST, TUNNEL, 0]] },
  { id: 'big-start', title: 'All Up Front', desc: 'Start with 150 tiles and never get more.', family: 'tiles', mods: [[K.START, 0, 150], [K.PREVENT, 0, 0], [K.FREE, 0, 0]] },
  { id: 'endless-tiles', title: 'Bottomless Pit', desc: 'Unlimited road tiles, none offered.', family: 'tiles', mods: [[K.UNLIMITED, 0, 0], [K.PREVENT, 0, 0], [K.FREE, 0, 0]] },
  { id: 'double-tiles', title: 'Double Paving', desc: 'Road tile packs are twice as big.', family: 'tiles', mods: [[K.AWARD, 0, 200]] },
  { id: 'half-tiles', title: 'Lean Weeks', desc: 'Road tile packs are half as big.', family: 'tiles', mods: [[K.AWARD, 0, 50]] },
  { id: 'no-choice', title: 'Just the Road', desc: 'No weekly upgrades at all.', family: 'choices', mods: [[K.CHOICES, 0, 0]] },
  { id: 'one-choice', title: 'Take It or Leave It', desc: 'Only one upgrade offered each week.', family: 'choices', mods: [[K.CHOICES, 0, 1]] },
  { id: 'no-bridges', title: 'Dry Feet', desc: 'No bridges to start, none offered.', family: 'bridge', mods: [[K.START, BRIDGE, 0], [K.PREVENT, BRIDGE, 0]] },
  { id: 'many-bridges', title: 'Bridge Builders', desc: 'Start with 4 bridges.', family: 'bridge', mods: [[K.START, BRIDGE, 4]] },
  { id: 'free-bridge-supply', title: 'Endless Spans', desc: 'Unlimited bridges.', family: 'bridge', mods: [[K.UNLIMITED, BRIDGE, 0]] },
  { id: 'many-tunnels', title: 'Mole People', desc: 'Start with 4 tunnels.', family: 'tunnel', mods: [[K.START, TUNNEL, 4]] },
  { id: 'endless-tunnels', title: 'Swiss Cheese', desc: 'Unlimited tunnels.', family: 'tunnel', mods: [[K.UNLIMITED, TUNNEL, 0]] },
  { id: 'motorway-fan', title: 'Fast Lane Fan', desc: 'Start with 3 motorways.', family: 'motorway', mods: [[K.START, MOTORWAY, 3]] },
  { id: 'motorway-only', title: 'Spaghetti Week', desc: 'Start with 5 motorways, none offered.', family: 'motorway', mods: [[K.START, MOTORWAY, 5], [K.PREVENT, MOTORWAY, 0]] },
  { id: 'many-lights', title: 'Light Show', desc: 'Start with 4 traffic lights.', family: 'light', mods: [[K.START, LIGHT, 4]] },
  { id: 'endless-lights', title: 'Stop and Go', desc: 'Unlimited traffic lights.', family: 'light', mods: [[K.UNLIMITED, LIGHT, 0]] },
  { id: 'many-rounds', title: 'Merry-Go-Round', desc: 'Start with 4 roundabouts.', family: 'round', mods: [[K.START, ROUND, 4]] },
  { id: 'endless-rounds', title: 'Round and Round', desc: 'Unlimited roundabouts.', family: 'round', mods: [[K.UNLIMITED, ROUND, 0]] },
  { id: 'one-of-each', title: 'One of Everything', desc: 'Start with one of every tool.', family: 'tools', mods: [[K.START, BRIDGE, 1], [K.START, MOTORWAY, 1], [K.START, LIGHT, 1], [K.START, ROUND, 1], [K.START, TUNNEL, 1]] },
  { id: 'solid-trees', title: 'Tree Huggers', desc: 'Trees cannot be cut down.', family: 'env', mods: [[K.TREES, 0, 0]] },
  { id: 'skyscrapers', title: 'Tall Order', desc: 'Every destination starts upgraded.', family: 'dest', mods: [[K.ALL_UP, 0, 0]] },
  { id: 'small-town', title: 'Small Town Charm', desc: 'Destinations never upgrade.', family: 'dest', mods: [[K.NEVER_UP, 0, 0]] },
  { id: 'rush-hour', title: 'Rush Hour', desc: 'Demand is 1.5 times higher everywhere.', family: 'demand', mods: allGroups(150) },
  { id: 'fast-bridges', title: 'Speedway Spans', desc: 'Cars fly over bridges.', family: 'speed', mods: [[K.SPEED, BRIDGE, 400]] },
  { id: 'slow-bridges', title: 'Crawl Across', desc: 'Cars crawl over bridges.', family: 'speed', mods: [[K.SPEED, BRIDGE, 40]] },
  { id: 'fast-tunnels', title: 'Go-Kart Tunnels', desc: 'Cars race through tunnels.', family: 'speed2', mods: [[K.SPEED, TUNNEL, 400]] },
  { id: 'slow-tunnels', title: 'Tunnel Traffic', desc: 'Cars crawl through tunnels.', family: 'speed2', mods: [[K.SPEED, TUNNEL, 40]] },
];

const byId = Object.fromEntries(PRESETS.map((p) => [p.id, p]));
export const presetById = (id) => byId[id];

/** Picks `n` presets from different families, the same way every time for the same key. */
export function pickPresets(key, n) {
  let h = hashString(key);
  const out = [];
  const families = new Set();
  let guard = 0;
  while (out.length < n && guard++ < 200) {
    h = (Math.imul(h, 1664525) + 1013904223) >>> 0;
    const p = PRESETS[h % PRESETS.length];
    if (families.has(p.family) || out.includes(p)) continue;
    families.add(p.family);
    out.push(p);
  }
  return out;
}

export function isoDay(date) { return (typeof date === 'string' ? date : date.toISOString().slice(0, 10)); }

/** Days since 1970-01-01 (UTC) for an ISO date. */
export function dayNumber(iso) { return Math.floor(Date.parse(iso + 'T00:00:00Z') / 86400000); }

/** One challenge, the same for everybody on a given UTC day. Saturdays are Expert. */
export function dailyChallengeSet(date, cities) {
  const iso = isoDay(date);
  const day = dayNumber(iso);
  const pool = cities.map((c, i) => ({ c, i })).filter((x) => x.c.id !== 'osaka');
  const pick = pool[hashString('daily-city-' + iso) % pool.length];
  const dow = ((day % 7) + 4) % 7; // 1970-01-01 was a Thursday; 0 = Sunday
  return {
    key: 'daily:' + iso, kind: 'daily', date: iso, index: pick.i, city: pick.c,
    seed: (hashString('daily-seed-' + iso) & 0x7fffffff) || 1,
    mode: dow === 6 ? 1 : 0,
    presets: pickPresets('daily-' + iso, 2),
  };
}

export function weeklyChallengeSet(date, cities) {
  const iso = isoDay(date);
  const week = Math.floor(dayNumber(iso) / 7);
  const pool = cities.map((c, i) => ({ c, i })).filter((x) => x.c.id !== 'osaka');
  const pick = pool[hashString('weekly-city-' + week) % pool.length];
  return {
    key: 'weekly:' + week, kind: 'weekly', week, index: pick.i, city: pick.c,
    seed: (hashString('weekly-seed-' + week) & 0x7fffffff) || 1, mode: 0,
    presets: pickPresets('weekly-' + week, 3),
  };
}

/** Three set challenges per city, unlocked by 1000 trips there. */
export function cityChallengeSet(cityId, cities) {
  const index = cities.findIndex((c) => c.id === cityId);
  const sets = [];
  for (let i = 0; i < 3; i++) {
    sets.push({
      key: `city:${cityId}:${i}`, kind: 'city', index, city: cities[index],
      seed: (hashString(`city-${cityId}-${i}`) & 0x7fffffff) || 1, mode: 0, target: 100 + i * 100,
      presets: pickPresets(`city-${cityId}-${i}`, i + 1),
    });
  }
  return sets;
}

/** All modifier triples of a challenge, flattened. */
export function modsOf(set) { return set.presets.flatMap((p) => p.mods); }
export function titleOf(set) { return set.presets.map((p) => p.title).join(' + '); }
export function describe(set) { return set.presets.map((p) => p.desc).join(' '); }

export const ANY_MAP_TRIPS_TO_UNLOCK = 300;
export const CITY_TRIPS_TO_UNLOCK = 1000;
