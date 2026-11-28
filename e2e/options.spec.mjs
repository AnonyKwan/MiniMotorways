import { test, expect } from '@playwright/test';

function watchErrors(page) {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  return errors;
}

test('pause, options and the saved game', async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto('/index.html?city=beijing&seed=11');
  await page.locator('#btn-help-close').click();
  await page.evaluate(() => { for (let i = 0; i < 300; i++) window.__ll.core.step(0.1); });
  await page.keyboard.press('Escape');
  await expect(page.locator('#pause')).toBeVisible();
  const t0 = await page.evaluate(() => window.__ll.core.snapshot().t);
  await page.waitForTimeout(600);
  const t1 = await page.evaluate(() => window.__ll.core.snapshot().t);
  expect(t1).toBe(t0); // paused: the clock does not move

  await page.locator('#btn-pause-options').click();
  await expect(page.locator('#options')).toBeVisible();
  await page.locator('#options input[data-opt="colorblind"]').check();
  await page.locator('#options select[data-opt="quality"]').selectOption('0.75');
  await page.screenshot({ path: 'shots/options.png' });
  const stored = await page.evaluate(() => JSON.parse(localStorage.getItem('littlelanes.options')));
  expect(stored.colorblind).toBe(true);
  expect(stored.quality).toBe(0.75);
  await page.locator('#btn-options-close').click();
  await page.locator('#btn-resume').click();
  await expect(page.locator('#pause')).toBeHidden();

  // leaving saves the game; the map list offers to continue it
  const tick = await page.evaluate(() => window.__ll.core.ticks());
  await page.locator('#btn-menu').click();
  await expect(page.locator('.card.resume')).toBeVisible();
  await page.locator('.card.resume').click();
  await expect(page.locator('#hud')).toBeVisible();
  const again = await page.evaluate(() => window.__ll.core.ticks());
  expect(again).toBe(tick);
  expect(await page.evaluate(() => window.__ll.core.snapshot().t)).toBeGreaterThan(0);
  await expect(page.locator('#pause')).toBeVisible(); // resumes paused
  await page.screenshot({ path: 'shots/resume.png' });
  expect(errors).toEqual([]);
});

test('options survive a reload', async ({ page }) => {
  await page.goto('/index.html');
  await page.locator('#btn-options').click();
  await page.locator('#options input[data-opt="reduceMotion"]').check();
  await page.locator('#btn-options-close').click();
  await page.reload();
  expect(await page.evaluate(() => window.__ll.options.reduceMotion)).toBe(true);
  expect(await page.evaluate(() => document.body.classList.contains('calm'))).toBe(true);
});
