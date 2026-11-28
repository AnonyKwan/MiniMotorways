import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { Core, ERR } from '../web/js/core.js';
import { MODES, dailyChallenge, hashString, errorText } from '../web/js/model.js';
import { noteFreq } from '../web/js/audio.js';

const wasmPath = new URL('../dist/little_lanes_core.wasm', import.meta.url);
const opts = { skip: existsSync(wasmPath) ? false : 'wasm not built' };
const CITIES = [{ id: 'a' }, { id: 'b' }, { id: 'osaka' }, { id: 'c' }, { id: 'd' }];

test('four modes are offered', () => {
  assert.deepEqual(MODES.map((m) => m.name), ['Classic', 'Expert', 'Creative', 'Endless']);
});

test("today's map is the same for everyone and never the hidden one", () => {
  const a = dailyChallenge('2026-11-28', CITIES), b = dailyChallenge('2026-11-28', CITIES);
  assert.deepEqual(a, b);
  assert.notEqual(a.city.id, 'osaka');
  const days = new Set();
  for (let d = 1; d <= 28; d++) {
    const r = dailyChallenge(`2026-02-${String(d).padStart(2, '0')}`, CITIES);
    assert.notEqual(r.city.id, 'osaka');
    assert.ok(r.seed > 0);
    days.add(r.city.id);
  }
  assert.ok(days.size > 1, 'the map changes from day to day');
  assert.notEqual(dailyChallenge('2026-11-29', CITIES).seed, a.seed);
});

test('hashing is stable', () => {
  assert.equal(hashString('abc'), hashString('abc'));
  assert.notEqual(hashString('abc'), hashString('abd'));
});

test('each colour plays in its own register of one scale', () => {
  assert.ok(noteFreq(0, 0) > 100 && noteFreq(0, 0) < 200);
  assert.ok(noteFreq(1, 0) > noteFreq(0, 0));
  assert.ok(Math.abs(noteFreq(0, 5) / noteFreq(0, 0) - 2) < 1e-9, 'five steps is an octave');
});

test('permanent roads have a message', () => {
  assert.match(errorText(ERR.PERMANENT), /set/);
});

test('expert roads set and creative has no limits (through the real WASM)', opts, async () => {
  const c = await Core.load(readFileSync(wasmPath));
  const bj = c.cities.findIndex((x) => x.id === 'beijing');
  c.newGame(bj, 3, 1);
  const w = c.cities[bj].w, t = c.terrain();
  let a = -1;
  for (let i = 2 * w; i < t.length - 3; i++) if ([i, i + 1].every((k) => (t[k] & 3) === 0)) { a = i; break; }
  assert.equal(c.addRoad(a, a + 1), ERR.OK);
  for (let i = 0; i < 150; i++) c.step(0.1);
  assert.equal(c.removeRoad(a, a + 1), ERR.PERMANENT);
  c.newGame(bj, 3, 2);
  const up = c.snapshot().up;
  assert.ok(up.every(([owned]) => owned >= 1000));
  for (let i = 0; i < 4000; i++) c.step(0.1);
  assert.equal(c.snapshot().over, false);
});
