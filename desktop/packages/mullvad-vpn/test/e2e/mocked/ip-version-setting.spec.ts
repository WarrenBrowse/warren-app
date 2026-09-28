import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { getDefaultSettings } from '../../../src/main/default-settings';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

let page: Page;
let util: MockedTestUtils;

const expandTrigger = () => page.getByRole('button', { name: 'Device IP version' });

test.describe('Device IP version setting', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startMockedApp());
    const routes = new RoutesObjectModel(page, util);
    await routes.main.waitForRoute();
    await routes.main.gotoSettings();
    await routes.settings.gotoVpnSettings();
    await util.ipc.settings[''].notify(getDefaultSettings());
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('folds its options and shows the current choice', async () => {
    const header = page
      .getByText('Device IP version', { exact: true })
      .locator('xpath=ancestor::*[.//button[@aria-expanded]][1]');
    await expect(header).toContainText('Automatic');
    await expect(page.getByRole('option', { name: 'IPv4' })).not.toBeVisible();

    await expandTrigger().click();
    await expect(page.getByRole('option', { name: 'IPv4' })).toBeVisible();
  });

  test('applies the chosen option', async () => {
    if ((await expandTrigger().getAttribute('aria-expanded')) !== 'true') {
      await expandTrigger().click();
    }
    const [sent] = await Promise.all([
      util.ipc.settings.setRelaySettings.expect(),
      page.getByRole('option', { name: 'IPv6' }).click(),
    ]);
    expect(sent).toMatchObject({
      normal: { wireguardConstraints: { ipVersion: { only: 'ipv6' } } },
    });
  });
});
