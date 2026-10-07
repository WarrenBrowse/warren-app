import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { getDefaultSettings } from '../../../src/main/default-settings';
import { ISplitTunnelingApplication } from '../../../src/shared/application-types';
import { RoutePath } from '../../../src/shared/routes';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

const SCREENSHOTS = process.env.APP_ROUTING_SCREENSHOTS ?? 'test-results/app-routing';

const STEAM: ISplitTunnelingApplication = {
  absolutepath: 'C:\\Program Files (x86)\\Steam\\steam.exe',
  name: 'Steam',
  deletable: false,
};

let page: Page;
let util: MockedTestUtils;

test.describe('App routing on Windows', () => {
  test.beforeAll(async () => {
    process.env.WARREN_E2E_PLATFORM = 'win32';
    ({ page, util } = await startMockedApp());
    delete process.env.WARREN_E2E_PLATFORM;
    const routes = new RoutesObjectModel(page, util);
    await util.expectRoute(RoutePath.main);
    await util.ipc.splitTunneling.isSupported.notify(true);
    await util.ipc.appRouting.getApplications.handle({ fromCache: false, applications: [STEAM] });

    await routes.main.gotoSettings();
    await routes.settings.gotoSplitTunnelingSettings();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('offers "Outside the VPN" as a default and per app, with the driver', async () => {
    const outside = page
      .getByRole('group', { name: 'Other apps go' })
      .getByRole('button', { name: 'Outside the VPN' });
    await expect(outside).toBeEnabled();

    await page.getByRole('button', { name: 'Add an app' }).click();
    await page.getByTestId('add-app-screen').getByRole('button', { name: 'Steam' }).click();
    const [added] = await Promise.all([
      util.ipc.splitTunneling.addApplication.expect(undefined),
      page.getByTestId('route-outside').click(),
    ]);
    expect(added).toEqual(STEAM);
    await util.ipc.settings[''].notify({
      ...getDefaultSettings(),
      appRouting: {
        splitMode: 'exclude',
        excludedApps: [STEAM.absolutepath.toLowerCase()],
        includedApps: [],
        appExitsEnabled: true,
        appExits: [],
        lockedApps: [],
      },
    });
    // Windows paths compare without case, like the daemon compares them.
    await expect(page.getByTestId('route-outside')).toHaveAttribute('aria-pressed', 'true');
    await page.screenshot({ path: `${SCREENSHOTS}/16-windows-route.png` });
  });

  test('picks another program with the Windows executable filter', async () => {
    await page.getByRole('button', { name: 'Done' }).click();
    await page.getByRole('button', { name: 'Add an app' }).click();
    const [options] = await Promise.all([
      util.ipc.app.showOpenDialog.expect({ canceled: true, filePaths: [] }),
      page.getByRole('button', { name: 'Find another app' }).click(),
    ]);
    expect(options).toMatchObject({
      filters: [{ name: 'Executables', extensions: ['exe', 'lnk'] }],
    });
  });
});
