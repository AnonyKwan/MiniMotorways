import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { Core, ERR } from '../web/js/core.js';
import { PRESETS, pickPresets, dailyChallengeSet, weeklyChallengeSet, cityChallengeSet, modsOf } from '../web/js/challenges.js';
import { ACHIEVEMENTS, emptyProgress, recordGame, sweep, isEarned, titleOf, describeDef } from '../web/js/achievements.js';

const wasmPath = new URL('../dist/little_lanes_core.wasm', import.meta.url);
const opts = { skip: existsSync(wasmPath) ? false : 'wasm not built' };
const CITIES = ['beijing', 'london', 'tokyo', 'osaka', 'dubai'].map((id) => ({ id, name: id }));

test('every challenge has a name, a description and valid modifiers', () => {
  assert.ok(PRESETS.length >= 28);
  const ids = new Set();
  for (const p of PRESETS) {
    assert.ok(!ids.has(p.id));
    ids.add(p.id);
    assert.ok(p.title && p.desc && p.mods.length > 0, p.id);
    for (const m of p.mods) assert.ok(m.length === 3 && m.every(Number.isInteger), p.id);
  }
});

test('daily and weekly challenges are the same for everyone', () => {
  const a = dailyChallengeSet('2026-11-28', CITIES);
  const b = dailyChallengeSet('2026-11-28', CITIES);
  assert.deepEqual(a, b);
  assert.equal(a.presets.length, 2);
  assert.notEqual(a.city.id, 'osaka');
  assert.equal(new Set(a.presets.map((p) => p.family)).size, 2, 'two different kinds of change');
  const w = weeklyChallengeSet('2026-11-28', CITIES);
  assert.equal(w.presets.length, 3);
  assert.deepEqual(weeklyChallengeSet('2026-11-29', CITIES).presets.map((p) => p.id), w.presets.map((p) => p.id), 'same week');
});

test('Saturdays are Expert days', () => {
  assert.equal(dailyChallengeSet('2026-11-28', CITIES).mode, 1); // a Saturday
  assert.equal(dailyChallengeSet('2026-11-27', CITIES).mode, 0);
});

test('every city has three challenges of growing size', () => {
  const sets = cityChallengeSet('london', CITIES);
  assert.deepEqual(sets.map((s) => s.presets.length), [1, 2, 3]);
  assert.deepEqual(sets.map((s) => s.target), [100, 200, 300]);
  assert.ok(modsOf(sets[2]).length >= 3);
  assert.equal(pickPresets('x', 3).length, 3);
});

test('achievement table is complete', () => {
  assert.equal(ACHIEVEMENTS.length, 197);
  const ids = new Set(ACHIEVEMENTS.map((a) => a[0]));
  assert.equal(ids.size, 197);
  for (const d of ACHIEVEMENTS) {
    assert.ok(titleOf(d, (c) => c).length > 0);
    assert.ok(describeDef(d, (c) => c).length > 0);
  }
});

test('score achievements unlock from the best trips on a map', () => {
  let p = emptyProgress();
  const stats = { placed: [0, 0, 0, 0, 0, 0], deleted: 0, trees: 0, bigPins: 0, milestones: 0 };
  p = recordGame(p, { cityId: 'beijing', mode: 0, score: 250, stats, concrete: 40 });
  const ids = sweep(p, null).map((d) => d[0]);
  assert.ok(ids.some((i) => /Beijing_Score200$/.test(i)), 'Beijing 200 earned');
  assert.ok(!ids.some((i) => /Beijing_Score1000$/.test(i)));
  assert.equal(sweep(p, null).length, 0, 'earned once only');
  const expertOnly = ACHIEVEMENTS.find((d) => d[2] === 'Score' && d[5] === 1 && d[1] === 'beijing');
  if (expertOnly) assert.equal(isEarned(expertOnly, p, null), false, 'classic trips do not count for Expert-only goals');
});

test('live games count before they end', () => {
  const p = emptyProgress();
  const live = { cityId: 'tokyo', mode: 0, score: 205, concrete: 700, stats: { placed: [0, 1, 1, 1, 1, 1], deleted: 0, trees: 1, bigPins: 0, milestones: 0 } };
  const ids = sweep(p, live).map((d) => d[0]);
  assert.ok(ids.some((i) => /Tokyo_Score200$/.test(i)));
  assert.ok(ids.length > 3, 'trees, all tools and concrete goals too');
});

test('modifiers, undo and Endless work through the real WASM', opts, async () => {
  const c = await Core.load(readFileSync(wasmPath));
  const bj = c.cities.findIndex((x) => x.id === 'beijing');
  c.setMods([[22, 0, 3]]); // straight roads cost 3
  c.newGame(bj, 3, 0);
  const t = c.terrain();
  const w = c.cities[bj].w;
  let a = -1;
  for (let i = 3 * w; i < t.length - 3; i++) {
    if ([i, i + 1, i + 2].every((k) => (t[k] & 3) === 0)) { a = i; break; }
  }
  assert.equal(c.addRoad(a, a + 1), ERR.OK);
  assert.equal(c.snapshot().up[0][1], 3);
  c.beginErase();
  assert.equal(c.eraseCell(a), ERR.OK);
  assert.equal(c.snapshot().up[0][1], 0);
  assert.equal(c.undo(), ERR.OK);
  assert.equal(c.snapshot().up[0][1], 3, 'put back');
  // the next game forgets the modifiers
  c.newGame(bj, 3, 0);
  assert.equal(c.addRoad(a, a + 1), ERR.OK);
  assert.equal(c.snapshot().up[0][1], 1);
  // endless
  c.newGame(bj, 3, 3);
  for (let i = 0; i < 3000; i++) c.step(0.1);
  const s = c.snapshot();
  assert.equal(s.over, false);
  assert.equal(s.eff.length, 3);
  assert.equal(s.eff[1], 60);
  // a lost game carries on
  c.newGame(bj, 3, 0);
  for (let i = 0; i < 6000 && !c.snapshot().over; i++) c.step(0.1);
  assert.equal(c.snapshot().over, true);
  c.continueEndless();
  assert.equal(c.snapshot().over, false);
});

test('the map opens up as the town grows', opts, async () => {
  const c = await Core.load(readFileSync(wasmPath));
  c.newGame(0, 5, 0);
  const r0 = c.snapshot().region;
  for (let i = 0; i < 6000; i++) c.step(0.1);
  const r1 = c.snapshot().region;
  assert.ok(r1[2] - r1[0] > r0[2] - r0[0], 'wider');
  assert.ok(r1[3] - r1[1] > r0[3] - r0[1], 'taller');
});
