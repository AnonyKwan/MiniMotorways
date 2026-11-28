// Achievements: a progress record plus rules that say which ones are earned.
import { ACHIEVEMENTS } from './achievements_data.js';

export const KIND_LABEL = ['road tiles', 'bridges', 'motorways', 'traffic lights', 'roundabouts', 'tunnels'];

export function emptyProgress() {
  return {
    best: {}, // cityId -> { c: classic best trips, e: expert best trips }
    placed: [0, 0, 0, 0, 0, 0], deleted: 0, trees: 0, bigPins: 0,
    concrete: 0, endlessMilestones: 0, daily: 0, weekly: 0, tutorial: false, allTools: false,
    cc: {}, // best score in each map's own challenges
    unlocked: {},
  };
}

export function normalise(p) {
  const e = emptyProgress();
  if (!p || typeof p !== 'object') return e;
  return { ...e, ...p, best: { ...(p.best || {}) }, cc: { ...(p.cc || {}) }, placed: [...(p.placed || e.placed)], unlocked: { ...(p.unlocked || {}) } };
}

/** Adds the numbers of one finished (or abandoned) game to the lifetime record. */
export function recordGame(progress, game) {
  const p = normalise(progress);
  const { cityId, mode, score, stats } = game;
  if (game.challengeKind === 'city') p.cc[cityId] = Math.max(p.cc[cityId] || 0, score);
  if ((mode === 0 || mode === 1) && !game.challengeKind) {
    const b = p.best[cityId] || { c: 0, e: 0 };
    if (mode === 0) b.c = Math.max(b.c, score);
    else b.e = Math.max(b.e, score);
    p.best[cityId] = b;
  }
  if (stats) {
    for (let k = 1; k < 6; k++) p.placed[k] += stats.placed[k] || 0;
    p.deleted += stats.deleted || 0;
    p.trees += stats.trees || 0;
    p.bigPins += stats.bigPins || 0;
    if (mode === 3) p.endlessMilestones = Math.max(p.endlessMilestones, stats.milestones || 0);
    const placed = stats.placed || [];
    if ([1, 2, 3, 4, 5].every((k) => (placed[k] || 0) > 0)) p.allTools = true;
  }
  p.concrete += game.concrete || 0;
  return p;
}

/** Live numbers for the game in progress, added on top of the lifetime record. */
function totals(p, live) {
  const placed = p.placed.map((v, k) => v + ((live && live.stats && live.stats.placed[k]) || 0));
  return {
    placed,
    deleted: p.deleted + ((live && live.stats && live.stats.deleted) || 0),
    trees: p.trees + ((live && live.stats && live.stats.trees) || 0),
    bigPins: p.bigPins + ((live && live.stats && live.stats.bigPins) || 0),
    concrete: p.concrete + ((live && live.concrete) || 0),
    milestones: Math.max(p.endlessMilestones, (live && live.mode === 3 && live.stats && live.stats.milestones) || 0),
    allTools: p.allTools || Boolean(live && live.stats && [1, 2, 3, 4, 5].every((k) => live.stats.placed[k] > 0)),
  };
}

export function isEarned(def, p, live) {
  const [, city, type, value, kind, only] = def;
  const t = totals(p, live);
  switch (type) {
    case 'Score': {
      if (def[0].includes('_Challenge_')) return ((p.cc || {})[city] || 0) >= value;
      const b = p.best[city] || { c: 0, e: 0 };
      let c = b.c, e = b.e;
      if (live && live.cityId === city && !live.challengeKind) { if (live.mode === 0) c = Math.max(c, live.score); if (live.mode === 1) e = Math.max(e, live.score); }
      return (only === 1 ? e : Math.max(c, e)) >= value;
    }
    case 'UpgradesUsed': return kind === 0 ? t.concrete >= value : t.placed[kind] >= value;
    case 'DeletedUpgrades': return t.deleted >= value;
    case 'TreesBulldozed': return t.trees >= value;
    case 'ClearBigPin': return t.bigPins >= value;
    case 'UseAllUpgrades': return t.allTools;
    case 'EndlessMilestones': return t.milestones >= value;
    case 'Tutorial': return Boolean(p.tutorial);
    case 'DailyChallenge': return p.daily >= value;
    case 'WeeklyChallenge': return p.weekly >= value;
    default: return false; // UpgradeLength is not tracked yet
  }
}

/** Marks newly earned achievements and returns them. */
export function sweep(p, live, now = Date.now()) {
  const fresh = [];
  for (const def of ACHIEVEMENTS) {
    if (p.unlocked[def[0]]) continue;
    if (isEarned(def, p, live)) { p.unlocked[def[0]] = now; fresh.push(def); }
  }
  return fresh;
}

export function titleOf(def, cityName) {
  const [, city, type, value, kind, only] = def;
  const tag = only === 1 ? ' (Expert)' : '';
  switch (type) {
    case 'Score': return `${cityName(city)} ${value}${tag}`;
    case 'UpgradesUsed': return `${value} ${KIND_LABEL[kind]}`;
    case 'DeletedUpgrades': return `Second Thoughts ${value}`;
    case 'TreesBulldozed': return 'Timber';
    case 'ClearBigPin': return 'Close Call';
    case 'UseAllUpgrades': return 'Full Toolbox';
    case 'EndlessMilestones': return 'Going Endless';
    case 'Tutorial': return 'Learner Driver';
    case 'DailyChallenge': return `Daily Regular ${value}`;
    case 'WeeklyChallenge': return `Weekly Regular ${value}`;
    default: return `${KIND_LABEL[kind]} length ${value}`;
  }
}

export function describeDef(def, cityName) {
  const [, city, type, value, kind, only] = def;
  switch (type) {
    case 'Score': return `Deliver ${value} trips in ${cityName(city)}${only === 1 ? ' in Expert mode' : ''}.`;
    case 'UpgradesUsed': return `Use ${value} ${KIND_LABEL[kind]} in total.`;
    case 'DeletedUpgrades': return `Take back ${value} tools you placed.`;
    case 'TreesBulldozed': return 'Build a road through a tree.';
    case 'ClearBigPin': return 'Bring an overflowing destination back under control.';
    case 'UseAllUpgrades': return 'Use every kind of tool in one game.';
    case 'EndlessMilestones': return `Reach milestone ${value} in Endless mode.`;
    case 'Tutorial': return 'Finish the tutorial.';
    case 'DailyChallenge': return `Play ${value} daily maps.`;
    case 'WeeklyChallenge': return `Play ${value} weekly maps.`;
    default: return `Build ${value} tiles of ${KIND_LABEL[kind]} (not tracked yet).`;
  }
}

export { ACHIEVEMENTS };
