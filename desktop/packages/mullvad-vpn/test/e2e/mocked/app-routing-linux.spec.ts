import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { getDefaultSettings } from '../../../src/main/default-settings';
import { ISplitTunnelingApplication } from '../../../src/shared/application-types';
import { RoutePath } from '../../../src/shared/routes';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

const SCREENSHOTS = process.env.APP_ROUTING_SCREENSHOTS ?? 'test-results/app-routing';

const applications: ISplitTunnelingApplication[] = [
  { absolutepath: '/usr/lib/signal-desktop/signal-desktop', name: 'Signal', deletable: false },
  {
    absolutepath: '/var/lib/flatpak/exports/share/applications/org.gimp.GIMP.desktop',
    name: 'GIMP',
    deletable: false,
    routingLimitation: 'flatpak',
  },
  {
    absolutepath: '/usr/share/applications/firefox.desktop',
    name: 'Firefox',
    deletable: false,
    routingLimitation: 'script',
  },
];

let page: Page;
let util: MockedTestUtils;

test.describe('App routing on Linux', () => {
  test.beforeAll(async () => {
    process.env.WARREN_E2E_PLATFORM = 'linux';
    ({ page, util } = await startMockedApp());
    delete process.env.WARREN_E2E_PLATFORM;
    const routes = new RoutesObjectModel(page, util);
    await util.expectRoute(RoutePath.main);
    await util.ipc.linuxSplitTunneling.getApplications.handle([]);
    await util.ipc.appRouting.getApplications.handle({ fromCache: false, applications });
    const settings = getDefaultSettings();
    await util.ipc.settings[''].notify({
      ...settings,
      appRouting: { ...settings.appRouting, splitMode: 'include-only', appExitsEnabled: true },
    });

    await routes.main.gotoSettings();
    await routes.settings.gotoSplitTunnelingSettings();
    await page.getByRole('tab', { name: 'Country per app' }).click();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('says why a sandboxed or scripted app takes no country', async () => {
    const list = page.getByTestId('apps-without-country');

    await expect(list).toContainText('Flatpak apps cannot use a country yet');
    await expect(list).toContainText(
      'Opens through a script: pick its program with Find another app',
    );
    await expect(page.getByRole('button', { name: 'Choose a country for GIMP' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Choose a country for Signal' })).toBeEnabled();
  });

  test('says a country takes effect for an app opened from VPN only for', async () => {
    await expect(page.getByRole('note')).toContainText(
      'VPN only for is on: an app uses its country when you open it from VPN only for.',
    );
    await page.screenshot({ path: `${SCREENSHOTS}/17-linux-countries.png` });
  });
});
