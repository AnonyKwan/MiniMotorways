import { test, expect } from '@playwright/test';

function watchErrors(page) {
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  return errors;
}

test('achievements, challenges and the tutorial are on the menu', async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto('/index.html');
  await page.locator('#btn-ach').click();
  await expect(page.locator('#ach .ach')).toHaveCount(197);
  await page.locator('#ach .row button').click();
  await page.locator('#unlock-all').check();
  await page.locator('#btn-chal').click();
  await expect(page.locator('#chal .ch').first()).toBeVisible();
  await page.screenshot({ path: 'shots/challenges.png' });
  await page.locator('#chal .ch button[data-challenge^="daily:"]').click();
  await expect(page.locator('#hud')).toBeVisible();
  expect(await page.evaluate(() => window.__ll.challenge && window.__ll.challenge.kind)).toBe('daily');
  await page.locator('#btn-menu').click();
  await page.locator('#btn-tutorial').click();
  await expect(page.locator('.coach')).toBeVisible();
  await expect(page.locator('.coach')).toContainText('1/5');
  await page.screenshot({ path: 'shots/tutorial.png' });
  expect(errors).toEqual([]);
});

test('undo, photo mode and carrying on as Endless', async ({ page }) => {
  const errors = watchErrors(page);
  await page.goto('/index.html?city=tokyo&seed=7');
  await page.locator('#btn-help-close').click();
  await page.locator('#btn-undo').click();
  await page.locator('#btn-photo').click();
  await expect(page.locator('body')).toHaveClass(/photo/);
  await expect(page.locator('#hud')).toBeHidden();
  await page.screenshot({ path: 'shots/photo.png' });
  await page.locator('.photobar button', { hasText: 'Done' }).click();
  await expect(page.locator('#hud')).toBeVisible();
  await page.evaluate(() => { for (let i = 0; i < 6000 && !window.__ll.core.snapshot().over; i++) window.__ll.core.step(0.1); });
  await expect(page.locator('#over')).toBeVisible({ timeout: 10000 });
  await page.locator('#btn-endless').click();
  await expect(page.locator('#over')).toBeHidden();
  await expect(page.locator('#hud-city')).toContainText('Endless');
  await page.waitForTimeout(500);
  expect(await page.evaluate(() => window.__ll.core.snapshot().over)).toBe(false);
  expect(errors).toEqual([]);
});
