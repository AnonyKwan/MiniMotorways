import { test, expect } from '@playwright/test';

test('a game controller moves a cursor and draws a road', async ({ page }) => {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(() => {
    const buttons = Array.from({ length: 17 }, () => ({ pressed: false, value: 0 }));
    window.__pad = { id: 'test pad', index: 0, connected: true, axes: [0, 0, 0, 0], buttons };
    navigator.getGamepads = () => [window.__pad];
  });
  await page.goto('/index.html?city=beijing&seed=4');
  await page.locator('#btn-help-close').click();
  await page.evaluate(() => { for (let i = 0; i < 300; i++) window.__ll.core.step(0.1); });
  await page.waitForFunction(() => window.__ll.camSettled());

  // pick two free neighbouring cells inside the open map and put the cursor on the first
  const cells = await page.evaluate(() => {
    const a = window.__ll, w = a.statics.w, h = a.statics.h;
    const taken = new Set();
    for (const [c] of a.statics.houses) taken.add(c);
    for (const [c] of a.statics.dests) for (const d of [0, 1, w, w + 1]) taken.add(c + d);
    const [rx0, ry0, rx1, ry1] = a.snap.region;
    for (let y = Math.max(2, ry0 + 1); y < Math.min(h - 2, ry1 - 1); y++) for (let x = Math.max(2, rx0 + 1); x < Math.min(w - 4, rx1 - 3); x++) {
      const c = y * w + x;
      if ([c, c + 1, c + 2].every((k) => (a.terrain[k] & 3) === 0 && !taken.has(k))) return [c, c + 2];
    }
    return null;
  });
  expect(cells).not.toBeNull();
  const [start] = await page.evaluate(([a, b]) => [window.__ll.cellCenter(a), window.__ll.cellCenter(b)], cells);
  await page.evaluate(([x, y]) => { window.__ll.cursor = [x, y]; window.__ll.padActive = true; }, start);

  // the stick moves the cursor
  const x0 = await page.evaluate(() => window.__ll.cursor[0]);
  await page.evaluate(() => { window.__pad.axes = [1, 0, 0, 0]; });
  await page.waitForTimeout(250);
  await page.evaluate(() => { window.__pad.axes = [0, 0, 0, 0]; });
  expect(await page.evaluate(() => window.__ll.cursor[0])).toBeGreaterThan(x0 + 20);
  expect(await page.evaluate(() => window.__ll.padActive)).toBe(true);

  // button A starts a road; moving the stick lays it
  const before = await page.evaluate(() => window.__ll.snap.up[0][1]);
  await page.evaluate(([x, y]) => { window.__ll.cursor = [x, y]; window.__pad.buttons[0] = { pressed: true, value: 1 }; }, start);
  await page.waitForTimeout(120);
  await page.evaluate(() => { window.__pad.axes = [0.8, 0, 0, 0]; });
  await page.waitForTimeout(400);
  await page.evaluate(() => { window.__pad.axes = [0, 0, 0, 0]; window.__pad.buttons[0] = { pressed: false, value: 0 }; });
  await page.waitForTimeout(150);
  const after = await page.evaluate(() => window.__ll.snap.up[0][1]);
  expect(after).toBeGreaterThan(before);
  expect(errors).toEqual([]);
});
