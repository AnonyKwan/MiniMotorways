import test from 'node:test';
import assert from 'node:assert/strict';
import { isUnlocked, unlockRule, lockText, stepsBetween, errorText, clockText, packText, GROUP_COLORS, GROUP_SHAPES } from '../web/js/model.js';
import { ERR } from '../web/js/core.js';

const CITIES = [{ id: 'london', name: 'London' }, { id: 'mumbai', name: 'Mumbai' }, { id: 'manila', name: 'Manila' }];

test('five maps are open from the start', () => {
  for (const id of ['losangeles', 'beijing', 'tokyo', 'daressalaam', 'london']) assert.equal(isUnlocked(id, {}), true, id);
  for (const id of ['manila', 'dubai', 'cairns', 'hongkong', 'mumbai', 'newyorkcity', 'osaka']) assert.equal(isUnlocked(id, {}), false, id);
});

test('the unlock chain follows the design', () => {
  assert.equal(isUnlocked('manila', { beijing: 199 }), false);
  assert.equal(isUnlocked('manila', { beijing: 200 }), true);
  assert.equal(isUnlocked('dubai', { manila: 249 }), false);
  assert.equal(isUnlocked('dubai', { manila: 250 }), true);
  assert.equal(isUnlocked('cairns', { dubai: 300 }), true);
  assert.equal(isUnlocked('hongkong', { cairns: 350 }), true);
  assert.equal(isUnlocked('hongkong', { beijing: 5000 }), false, 'tiers cannot be skipped');
  assert.equal(isUnlocked('mumbai', { london: 300 }), true);
  assert.equal(isUnlocked('newyorkcity', { mumbai: 399 }), false);
  assert.equal(isUnlocked('newyorkcity', { mumbai: 400 }), true);
  assert.equal(isUnlocked('osaka', { beijing: 99999 }), false);
});

test('unlock all opens everything', () => {
  assert.equal(isUnlocked('newyorkcity', {}, true), true);
});

test('lock text names the requirement', () => {
  assert.match(lockText('mumbai', CITIES), /300 trips in London/);
  assert.equal(lockText('beijing', CITIES), '');
  assert.equal(unlockRule('beijing'), null);
});

test('dragging across cells moves one step at a time and ends on target', () => {
  const w = 20;
  const steps = stepsBetween(w, 0, 5 * w + 5);
  assert.equal(steps[steps.length - 1], 5 * w + 5);
  let prev = 0;
  for (const s of steps) {
    const dx = Math.abs((s % w) - (prev % w)), dy = Math.abs(Math.floor(s / w) - Math.floor(prev / w));
    assert.ok(dx <= 1 && dy <= 1 && dx + dy > 0, `step ${prev}->${s}`);
    prev = s;
  }
  assert.deepEqual(stepsBetween(w, 7, 7), []);
  const long = stepsBetween(w, 0, 10);
  assert.equal(long.length, 10, 'a straight drag is straight');
});

test('error codes have friendly text', () => {
  assert.match(errorText(ERR.NO_CONCRETE), /road tiles/);
  assert.match(errorText(ERR.NO_BRIDGE), /bridge/);
  assert.equal(errorText(ERR.OK), '');
});

test('clock and pack text', () => {
  assert.equal(clockText(0), 'Week 1 · Mon');
  assert.equal(clockText(7.5), 'Week 2 · Mon');
  assert.equal(packText([0, 30, 0]), '30 road tiles');
  assert.match(packText([1, 1, 20]), /1 bridge \+ 20 road tiles/);
});

test('every colour has its own shape, in both themes', () => {
  assert.equal(new Set(GROUP_SHAPES).size, 6);
  assert.equal(GROUP_COLORS.light.length, 6);
  assert.equal(GROUP_COLORS.dark.length, 6);
});
