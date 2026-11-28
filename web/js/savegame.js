// Saving a game by recording what the player did. The simulation is deterministic, so
// replaying the same actions at the same simulation step rebuilds the same town.

/** One simulation step as the core sees it (a 32-bit float), so replays tick exactly once per call. */
export const STEP32 = Math.fround(1 / 30);
export const SAVE_VERSION = 1;
const MAX_LOG = 20000;

export function newSave(city, seed, mode, mods, challenge) {
  return { v: SAVE_VERSION, city, seed, mode, mods: mods || [], challenge: challenge || null, log: [], tick: 0, score: 0, savedAt: 0 };
}

/** Performs one recorded action on the core. Returns the core's result code. */
export function applyOp(core, op, a = 0, b = 0) {
  switch (op) {
    case 'r': return core.addRoad(a, b);
    case 'b': core.beginErase(); return 0;
    case 'e': return core.eraseCell(a);
    case 'm': return core.addMotorway(a, b);
    case 'l': return core.toggleLight(a);
    case 'o': return core.toggleRoundabout(a);
    case 'c': return core.chooseUpgrade(a);
    case 'u': return core.undo();
    case 'n': return core.continueEndless();
    default: return 12;
  }
}

/** Runs an action and, when it changed something, writes it into the save log. */
export function perform(core, save, op, a = 0, b = 0) {
  const code = applyOp(core, op, a, b);
  if ((code === 0 || op === 'b') && save && save.log.length < MAX_LOG) save.log.push([core.ticks(), op, a, b]);
  return code;
}

/** Steps the core until it has taken exactly `tick` steps. */
export function advanceTo(core, tick) {
  let guard = 0;
  while (core.ticks() < tick && guard++ < 400000) core.step(STEP32);
}

/** Rebuilds a saved game inside `core`. */
export function replay(core, save) {
  core.setMods(save.mods);
  core.newGame(save.city, save.seed, save.mode);
  for (const [tick, op, a, b] of save.log) {
    advanceTo(core, tick);
    applyOp(core, op, a, b);
  }
  advanceTo(core, save.tick);
}

export function describe(save, cities) {
  const c = cities[save.city];
  const mins = Math.floor(save.tick / 30 / 60);
  return `${c ? c.name : 'Unknown map'} · ${save.score} trips · ${mins} min`;
}

export function validSave(s) {
  return Boolean(s) && s.v === SAVE_VERSION && Number.isInteger(s.city) && Number.isFinite(s.seed) && Array.isArray(s.log) && Number.isFinite(s.tick);
}
