import test from 'node:test';
import assert from 'node:assert/strict';
import { stick, PadReader, DEAD_ZONE, clampPoint, BASE_CURSOR_PX_PER_SECOND, CURSOR_SPEEDS } from '../web/js/gamepad.js';

const pad = (axes = [0, 0, 0, 0], held = []) => ({ axes, buttons: Array.from({ length: 17 }, (_, i) => ({ pressed: held.includes(i), value: held.includes(i) ? 1 : 0 })) });

test('small stick drift is ignored', () => {
  assert.deepEqual(stick(0.1, 0.05), [0, 0]);
  const [x] = stick(DEAD_ZONE + 0.01, 0);
  assert.ok(x > 0 && x < 0.05, 'starts gently');
  const [fx] = stick(1, 0);
  assert.ok(Math.abs(fx - 1) < 1e-9, 'full tilt is full speed');
});

test('the left stick moves the cursor, scaled by time and sensitivity', () => {
  const r = new PadReader();
  const a = r.read(pad([1, 0, 0, 0]), 0.5, 1);
  assert.ok(Math.abs(a.cursor[0] - BASE_CURSOR_PX_PER_SECOND * 0.5) < 1e-6);
  const fast = new PadReader().read(pad([1, 0, 0, 0]), 0.5, CURSOR_SPEEDS[4]);
  assert.ok(fast.cursor[0] > a.cursor[0] * 1.7);
  assert.deepEqual(new PadReader().read(pad(), 0.5).cursor, [0, 0]);
});

test('buttons report when they go down and come back up', () => {
  const r = new PadReader();
  assert.ok(r.read(pad([0, 0, 0, 0], [0]), 0.016).pressed.has('draw'));
  const held = r.read(pad([0, 0, 0, 0], [0]), 0.016);
  assert.ok(held.down.has('draw') && !held.pressed.has('draw'), 'held, not pressed again');
  assert.ok(r.read(pad(), 0.016).released.has('draw'));
});

test('triggers zoom in and out', () => {
  assert.equal(new PadReader().read(pad([0, 0, 0, 0], [7]), 0.016).zoom, 1);
  assert.equal(new PadReader().read(pad([0, 0, 0, 0], [6]), 0.016).zoom, -1);
  assert.equal(new PadReader().read(pad([0, 0, 0, 0], [6, 7]), 0.016).zoom, 0);
});

test('the cursor stays on screen', () => {
  assert.deepEqual(clampPoint(-50, 9999, 800, 600), [4, 596]);
});
