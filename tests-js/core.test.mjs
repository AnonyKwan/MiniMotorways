// Runs the browser-side code against the real compiled WebAssembly.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { Core, ERR, KIND } from '../web/js/core.js';

const wasmPath = new URL('../dist/little_lanes_core.wasm', import.meta.url);
const haveWasm = existsSync(wasmPath);
const bytes = haveWasm ? readFileSync(wasmPath) : null;
const opts = { skip: haveWasm ? false : 'dist/little_lanes_core.wasm has not been built' };

async function core() { return Core.load(bytes); }

function landPair(core) {
  const [w, h] = [core.city.w, core.city.h];
  const t = core.terrain();
  for (let y = 2; y < h - 2; y++) for (let x = 2; x < w - 3; x++) {
    const a = y * w + x, b = a + 1;
    if ((t[a] & 3) === 0 && (t[b] & 3) === 0) return [a, b];
  }
  throw new Error('no land');
}

test('catalog lists the 27 maps and the hidden one', opts, async () => {
  const c = await core();
  assert.equal(c.cities.length, 28);
  const names = c.cities.map((x) => x.name);
  for (const n of ['Beijing', 'London', 'Tokyo', 'Cape Town', 'Zürich']) assert.ok(names.includes(n), n);
  const london = c.cities.find((x) => x.id === 'london');
  assert.deepEqual(london.start, [[0, 30], [1, 2]]);
  assert.equal(london.w, 41);
});

test('a new game starts empty and clean', opts, async () => {
  const c = await core();
  c.newGame(c.cities.findIndex((x) => x.id === 'beijing'), 5);
  const s = c.snapshot();
  assert.equal(s.score, 0);
  assert.equal(s.over, false);
  assert.deepEqual(s.up[KIND.CONCRETE], [30, 0]);
  assert.deepEqual(s.up[KIND.BRIDGE], [1, 0]);
  assert.equal(c.terrain().length, 36 * 20);
});

test('same seed gives the same game, different seed a different one', opts, async () => {
  const a = await core(), b = await core(), d = await core();
  const run = (k, seed) => { k.newGame(11, seed); for (let i = 0; i < 600; i++) k.step(0.1); return JSON.stringify([k.snapshot(), k.statics()]); };
  assert.equal(run(a, 9), run(b, 9));
  assert.notEqual(run(d, 10), run(a, 9));
});

test('drawing a road spends concrete and erasing gives it back', opts, async () => {
  const c = await core();
  c.newGame(c.cities.findIndex((x) => x.id === 'beijing'), 3);
  const [a, b] = landPair(c);
  assert.equal(c.addRoad(a, b), ERR.OK);
  assert.deepEqual(c.snapshot().up[KIND.CONCRETE], [30, 1]);
  assert.equal(c.addRoad(a, b), ERR.EXISTS);
  assert.equal(c.statics().roads.length, 1);
  assert.equal(c.eraseCell(a), ERR.OK);
  assert.deepEqual(c.snapshot().up[KIND.CONCRETE], [30, 0]);
  assert.equal(c.statics().roads.length, 0);
});

test('bad requests come back as codes, not crashes', opts, async () => {
  const c = await core();
  c.newGame(0, 1);
  assert.equal(c.addRoad(0, 5), ERR.NOT_ADJACENT);
  assert.equal(c.addRoad(0, 99999999), ERR.BOUNDS);
  assert.equal(c.addMotorway(1, 1), ERR.BOUNDS);
  assert.equal(c.toggleLight(3), ERR.BAD_PLACEMENT);
  assert.equal(c.chooseUpgrade(0), ERR.NO_OFFER);
  assert.equal(c.removeMotorway(4), ERR.NOTHING);
});

test('every map plays for two minutes of game time without trapping', opts, async () => {
  const c = await core();
  for (let i = 0; i < c.cities.length; i++) {
    c.newGame(i, 100 + i);
    for (let k = 0; k < 1200; k++) c.step(0.1);
    const s = c.snapshot();
    assert.ok(s.t > 100 || s.over, c.cities[i].id);
    assert.ok(c.statics().dests.length >= 1, c.cities[i].id);
  }
});

test('the first weekly offer opens after 140 seconds and can be taken', opts, async () => {
  const c = await core();
  c.newGame(c.cities.findIndex((x) => x.id === 'beijing'), 8);
  for (let i = 0; i < 1430; i++) c.step(0.1);
  let s = c.snapshot();
  assert.equal(s.offerOpen, true);
  assert.equal(s.offers.length, 2);
  assert.equal(c.chooseUpgrade(1), ERR.OK);
  s = c.snapshot();
  assert.equal(s.offerOpen, false);
  assert.equal(s.week, 1);
});
