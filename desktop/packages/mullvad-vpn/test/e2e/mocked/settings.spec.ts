import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { RoutePath } from '../../../src/shared/routes';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

let page: Page;
let util: MockedTestUtils;

test.beforeAll(async () => {
  ({ page, util } = await startMockedApp());
  await util.expectRoute(RoutePath.main);
});

test.afterAll(async () => {
  await util?.closePage();
});

test('Account button should be displayed correctly', async () => {
  const accountButton = page.getByLabel('Account settings');
  await expect(accountButton).toBeVisible();
});

// The main view carries the remaining time in its footer; the header only
// shows it on the views that have no footer.
test('Footer time left should be displayed correctly', async () => {
  const timeLeft = page.getByTestId('time-left');
  await expect(timeLeft).toHaveText(/^29 days left$/i);

  /**
   * 729 days left
   * Add a one-second margin to the test, since it randomly fails in Github Actions otherwise
   */
  await util.ipc.account[''].notify({
    expiry: new Date(Date.now() + 730 * 24 * 60 * 60 * 1000 - 1000).toISOString(),
  });
  await expect(timeLeft).toHaveText(/^729 days left$/i);

  /**
   * 2 years left
   */
  await util.ipc.account[''].notify({
    expiry: new Date(Date.now() + 731 * 24 * 60 * 60 * 1000).toISOString(),
  });
  await expect(timeLeft).toHaveText(/^2 years left$/i);

  /**
   * Expiry 1 day ago should show 'out of time'
   */
  await util.ipc.account[''].notify({
    expiry: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000).toISOString(),
  });
  await expect(timeLeft).not.toBeVisible();
});

test('Settings Page', async () => {
  await page.click('button[aria-label="Settings"]');
  await util.expectRoute(RoutePath.settings);

  const title = page.locator('h1');
  await expect(title).toContainText('Settings');

  const closeButton = page.locator('button[aria-label="Close"]');
  await expect(closeButton).toBeVisible();
});
