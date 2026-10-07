import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { getDefaultSettings } from '../../../src/main/default-settings';
import { ISplitTunnelingApplication } from '../../../src/shared/application-types';
import { AppRoutingSettings } from '../../../src/shared/daemon-rpc-types';
import { RoutePath } from '../../../src/shared/routes';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

const SCREENSHOTS = process.env.APP_ROUTING_SCREENSHOTS ?? 'test-results/app-routing';

const FIREFOX: ISplitTunnelingApplication = {
  absolutepath: 'C:\\Program Files\\Mozilla Firefox\\firefox.exe',
  name: 'Firefox',
  deletable: false,
};

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;

function routing(overrides: Partial<AppRoutingSettings>): AppRoutingSettings {
  return {
    splitMode: 'off',
    excludedApps: [],
    includedApps: [],
    appExitsEnabled: true,
    appExits: [],
    lockedApps: [],
    ...overrides,
  };
}

async function notifyRouting(appRouting: AppRoutingSettings) {
  await util.ipc.settings[''].notify({ ...getDefaultSettings(), appRouting });
}

test.describe('Never without the VPN, on Windows', () => {
  test.beforeAll(async () => {
    process.env.WARREN_E2E_PLATFORM = 'win32';
    ({ page, util } = await startMockedApp());
    delete process.env.WARREN_E2E_PLATFORM;
    routes = new RoutesObjectModel(page, util);
    await util.expectRoute(RoutePath.main);
    await util.ipc.splitTunneling.isSupported.notify(true);
    await util.ipc.appRouting.getApplications.handle({ fromCache: false, applications: [FIREFOX] });
    await routes.main.gotoSettings();
    await routes.settings.gotoSplitTunnelingSettings();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('locks an app on the VPN from its route', async () => {
    await page.getByRole('button', { name: 'Add an app' }).click();
    await page.getByTestId('add-app-screen').getByRole('button', { name: 'Firefox' }).click();
    const lock = page.getByRole('switch', { name: 'Never without the VPN' });
    await expect(lock).toBeEnabled();
    await expect(lock).not.toBeChecked();

    const [locked] = await Promise.all([
      util.ipc.appRouting.addLockedApp.expect(undefined),
      lock.click(),
    ]);
    expect(locked).toEqual(FIREFOX);
    await notifyRouting(routing({ lockedApps: [FIREFOX.absolutepath] }));

    await expect(lock).toBeChecked();
    await expect(page.getByTestId('route-lock')).toContainText(
      'When the VPN is off, Firefox has no Internet, even with Warren VPN closed.',
    );
    await page.screenshot({ path: `${SCREENSHOTS}/30-lock-route.png` });
  });

  test('shows the locked app as a rule that is blocked while the VPN is off', async () => {
    await page.getByRole('button', { name: 'Done' }).click();
    const rule = page.getByRole('button', {
      name: 'Firefox, Through the VPN, never without the VPN',
    });
    await expect(rule).toBeVisible();
    await expect(rule.getByTestId('route-chip-lock')).toBeVisible();
    await expect(rule).toContainText('Blocked until the VPN connects');
    await page.screenshot({ path: `${SCREENSHOTS}/31-lock-list.png` });
  });

  test('cannot lock an app outside the VPN, and lifts the lock before sending it there', async () => {
    await page
      .getByRole('button', { name: 'Firefox, Through the VPN, never without the VPN' })
      .click();
    const calls: string[] = [];
    const unlocked = util.ipc.appRouting.removeLockedApp
      .expect(undefined)
      .then(() => calls.push('unlock'));
    const excluded = util.ipc.splitTunneling.addApplication
      .expect(undefined)
      .then(() => calls.push('exclude'));
    await page.getByTestId('route-outside').click();
    await Promise.all([unlocked, excluded]);
    expect(calls).toEqual(['unlock', 'exclude']);

    await notifyRouting(routing({ splitMode: 'exclude', excludedApps: [FIREFOX.absolutepath] }));
    const lock = page.getByRole('switch', { name: 'Never without the VPN' });
    await expect(lock).toBeDisabled();
    await expect(page.getByTestId('route-lock')).toContainText(
      'Choose a route through the VPN to turn this on',
    );
  });

  test('says on the main screen how many apps wait for the VPN', async () => {
    await notifyRouting(routing({ lockedApps: [FIREFOX.absolutepath] }));
    await page.getByRole('button', { name: 'Done' }).click();
    await page.getByRole('button', { name: 'Back' }).click();
    await util.expectRoute(RoutePath.settings);
    await page.getByRole('button', { name: 'Close' }).click();
    await util.expectRoute(RoutePath.main);

    const label = page.getByTestId('locked-apps-label');
    await expect(label).toHaveText('1 app blocked until the VPN connects');
    await page.screenshot({ path: `${SCREENSHOTS}/32-lock-main-screen.png` });

    await label.click();
    await util.expectRoute(RoutePath.splitTunneling);
  });
});
