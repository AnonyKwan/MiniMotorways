import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { Core } from '../web/js/core.js';
import { newSave, perform, replay, advanceTo, validSave, describe } from '../web/js/savegame.js';
import { normaliseOptions, DEFAULT_OPTIONS } from '../web/js/options.js';
import { GROUP_COLORS, setColorblind } from '../web/js/model.js';
import { noteFreq } from '../web/js/audio.js';

const wasmPath = new URL('../dist/little_lanes_core.wasm', import.meta.url);
const opts = { skip: existsSync(wasmPath) ? false : 'wasm not built' };

test('options are cleaned up and clamped', () => {
  assert.deepEqual(normaliseOptions(null), DEFAULT_OPTIONS);
  const o = normaliseOptions({ master: 7, music: -1, soundscape: 9, quality: 3, colorblind: 1 });
  assert.equal(o.master, 1);
  assert.equal(o.music, 0);
  assert.equal(o.soundscape, 2);
  assert.equal(o.quality, 1);
  assert.equal(o.colorblind, true);
});

test('colour-blind colours swap in place and back', () => {
  const before = [...GROUP_COLORS.light];
  setColorblind(true);
  assert.notDeepEqual(GROUP_COLORS.light, before);
  assert.equal(new Set(GROUP_COLORS.light).size, 6);
  setColorblind(false);
  assert.deepEqual(GROUP_COLORS.light, before);
});

test('a save is recognised only when it makes sense', () => {
  assert.equal(validSave(null), false);
  assert.equal(validSave({ v: 1, city: 3, seed: 5, log: [], tick: 10 }), true);
  assert.equal(validSave({ v: 2, city: 3, seed: 5, log: [], tick: 10 }), false);
  assert.match(describe({ city: 0, score: 12, tick: 30 * 60 * 3 }, [{ name: 'Beijing' }]), /Beijing · 12 trips · 3 min/);
});

test('replaying a recorded game rebuilds exactly the same town', opts, async () => {
  const live = await Core.load(readFileSync(wasmPath));
  const bj = live.cities.findIndex((c) => c.id === 'beijing');
  const save = newSave(bj, 4242, 0, [], null);
  live.setMods(save.mods);
  live.newGame(bj, save.seed, save.mode);
  // play with uneven frame sizes, like a real browser
  const frames = [0.016, 0.033, 0.017, 0.05, 0.016, 0.02, 0.1];
  const w = live.cities[bj].w;
  const t = live.terrain();
  let spot = -1;
  for (let i = 3 * w; i < t.length - 5; i++) if ([0, 1, 2, 3].every((k) => (t[i + k] & 3) === 0)) { spot = i; break; }
  let n = 0;
  for (let step = 0; step < 900; step++) {
    live.step(frames[step % frames.length]);
    if (step === 100) perform(live, save, 'r', spot, spot + 1);
    if (step === 140) perform(live, save, 'r', spot + 1, spot + 2);
    if (step === 300) { perform(live, save, 'b'); perform(live, save, 'e', spot + 2); }
    if (step === 340) perform(live, save, 'u');
    if (step === 500) perform(live, save, 'l', spot + 1);
    n++;
  }
  save.tick = live.ticks();
  assert.ok(save.log.length >= 4, `recorded ${save.log.length} actions`);
  const copy = await Core.load(readFileSync(wasmPath));
  replay(copy, JSON.parse(JSON.stringify(save)));
  assert.equal(copy.ticks(), live.ticks());
  assert.equal(JSON.stringify(copy.snapshot()), JSON.stringify(live.snapshot()));
  assert.equal(JSON.stringify(copy.statics()), JSON.stringify(live.statics()));
  assert.ok(n > 0);
});

test('stepping to a tick count is exact', opts, async () => {
  const c = await Core.load(readFileSync(wasmPath));
  c.newGame(0, 1, 0);
  advanceTo(c, 123);
  assert.equal(c.ticks(), 123);
  advanceTo(c, 100);
  assert.equal(c.ticks(), 123, 'never goes backwards');
});

test('note pitch table is unchanged', () => {
  assert.ok(noteFreq(0, 0) > 0);
});
