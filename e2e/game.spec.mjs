import { test, expect } from '@playwright/test';
import { mkdirSync } from 'node:fs';

mkdirSync('shots', { recursive: true });

function watchErrors(page) {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  return errors;
}

test('the map list shows five open maps and the rest locked', async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto('/index.html');
  await expect(page.locator('h1')).toHaveText('Little Lanes');
  await expect(page.locator('#city-grid .card')).toHaveCount(27);
  await expect(page.locator('#city-grid .card:not([disabled])')).toHaveCount(5);
  await expect(page.locator('#city-grid .card[data-city="beijing"]')).toBeEnabled();
  await expect(page.locator('#city-grid .card[data-city="hongkong"]')).toBeDisabled();
  await page.screenshot({ path: 'shots/menu.png' });
  expect(errors).toEqual([]);
});

test('unlocking every map opens them all', async ({ page }) => {
  await page.goto('/index.html');
  await page.locator('#unlock-all').check();
  await expect(page.locator('#city-grid .card:not([disabled])')).toHaveCount(28);
});

test('start a map, let the town grow, draw and erase a road', async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto('/index.html?city=beijing&seed=4');
  await expect(page.locator('#hud')).toBeVisible();
  await expect(page.locator('#help')).toBeVisible(); // first-run help
  await page.locator('#btn-help-close').click();
  await expect(page.locator('#hud-city')).toHaveText('Beijing');

  // fast-forward the simulation so the town has some buildings
  await page.evaluate(() => { for (let i = 0; i < 400; i++) window.__ll.core.step(0.1); });
  await page.waitForTimeout(300);
  const counts = await page.evaluate(() => ({ houses: window.__ll.statics.houses.length, dests: window.__ll.statics.dests.length }));
  expect(counts.dests).toBeGreaterThanOrEqual(1);
  expect(counts.houses).toBeGreaterThanOrEqual(1);

  // the canvas is not blank
  const painted = await page.evaluate(() => {
    const c = document.getElementById('stage');
    const g = c.getContext('2d');
    const d = g.getImageData(0, 0, c.width, c.height).data;
    const seen = new Set();
    for (let i = 0; i < d.length; i += 4 * 997) seen.add(`${d[i]},${d[i + 1]},${d[i + 2]}`);
    return seen.size;
  });
  expect(painted).toBeGreaterThan(4);

  // find two free neighbouring land cells and drag a road between them with the mouse
  const pair = await page.evaluate(() => {
    const a = window.__ll, w = a.statics.w, h = a.statics.h;
    const taken = new Set();
    for (const [c] of a.statics.houses) taken.add(c);
    for (const [c] of a.statics.dests) for (const d of [0, 1, w, w + 1]) taken.add(c + d);
    for (let y = 2; y < h - 2; y++) for (let x = 2; x < w - 4; x++) {
      const c = y * w + x;
      if ([c, c + 1, c + 2].every((k) => (a.terrain[k] & 3) === 0 && !taken.has(k))) return [c, c + 2];
    }
    return null;
  });
  expect(pair).not.toBeNull();
  const [p0, p1] = await page.evaluate(([a, b]) => [window.__ll.cellCenter(a), window.__ll.cellCenter(b)], pair);
  await page.mouse.move(p0[0], p0[1]);
  await page.mouse.down();
  await page.mouse.move((p0[0] + p1[0]) / 2, p0[1], { steps: 4 });
  await page.mouse.move(p1[0], p1[1], { steps: 4 });
  await page.mouse.up();
  const used = await page.evaluate(() => window.__ll.snap.up[0][1]);
  expect(used).toBe(2);
  await expect(page.locator('.tool[data-tool="road"] .count')).toHaveText('28');

  // right-drag erases it again
  await page.mouse.move(p0[0], p0[1]);
  await page.mouse.down({ button: 'right' });
  await page.mouse.move(p1[0], p1[1], { steps: 6 });
  await page.mouse.up({ button: 'right' });
  const after = await page.evaluate(() => window.__ll.snap.up[0][1]);
  expect(after).toBe(0);
  await page.screenshot({ path: 'shots/game.png' });
  expect(errors).toEqual([]);
});

test('the weekly upgrade card appears and can be picked', async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto('/index.html?city=london&seed=2');
  await page.locator('#btn-help-close').click();
  await page.evaluate(() => { for (let i = 0; i < 1500; i++) window.__ll.core.step(0.1); });
  await expect(page.locator('#offer')).toBeVisible();
  await expect(page.locator('#offer-cards button')).toHaveCount(2);
  await page.screenshot({ path: 'shots/offer.png' });
  await page.locator('#offer-cards button').first().click();
  await expect(page.locator('#offer')).toBeHidden();
  expect(errors).toEqual([]);
});

test('an unattended town ends in the game over screen', async ({ page }) => {
  await page.goto('/index.html?city=tokyo&seed=7');
  await page.locator('#btn-help-close').click();
  await page.evaluate(() => { for (let i = 0; i < 5000 && !window.__ll.core.snapshot().over; i++) window.__ll.core.step(0.1); });
  await expect(page.locator('#over')).toBeVisible({ timeout: 10_000 });
  await page.screenshot({ path: 'shots/over.png' });
  await page.locator('#btn-over-menu').click();
  await expect(page.locator('#menu')).toBeVisible();
});

test('light and dark themes both work', async ({ page }) => {
  await page.goto('/index.html?theme=light');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.locator('#btn-theme-menu').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.screenshot({ path: 'shots/menu-dark.png' });
});

test('it fits a phone screen and takes touch', async ({ browser }) => {
  const ctx = await browser.newContext({ viewport: { width: 390, height: 780 }, hasTouch: true, isMobile: true, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  const errors = watchErrors(page);
  await page.goto('/index.html?city=losangeles&seed=3');
  await page.locator('#btn-help-close').tap();
  await page.evaluate(() => { for (let i = 0; i < 300; i++) window.__ll.core.step(0.1); });
  await page.waitForTimeout(300);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
  expect(overflow).toBe(false);
  await expect(page.locator('#tools')).toBeVisible();
  await page.screenshot({ path: 'shots/phone.png' });
  expect(errors).toEqual([]);
  await ctx.close();
});
