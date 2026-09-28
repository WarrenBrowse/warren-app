import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { getDefaultSettings } from '../../../src/main/default-settings';
import { ISplitTunnelingApplication } from '../../../src/shared/application-types';
import { AppRoutingSettings } from '../../../src/shared/daemon-rpc-types';
import { RoutePath } from '../../../src/shared/routes';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

const SCREENSHOTS = process.env.APP_ROUTING_SCREENSHOTS ?? 'test-results/app-routing';

const SIGNAL: ISplitTunnelingApplication = {
  absolutepath: '/usr/lib/signal-desktop/signal-desktop',
  name: 'Signal',
  deletable: false,
  launchPath: '/usr/share/applications/signal-desktop.desktop',
};
const applications: ISplitTunnelingApplication[] = [
  SIGNAL,
  {
    absolutepath: '/var/lib/flatpak/exports/share/applications/org.gimp.GIMP.desktop',
    name: 'GIMP',
    deletable: false,
    routingLimitation: 'flatpak',
    launchPath: '/var/lib/flatpak/exports/share/applications/org.gimp.GIMP.desktop',
  },
  {
    absolutepath: '/usr/share/applications/firefox.desktop',
    name: 'Firefox',
    deletable: false,
    routingLimitation: 'script',
    launchPath: '/usr/share/applications/firefox.desktop',
    launchWarning: 'launches-in-existing-process',
  },
];

let page: Page;
let util: MockedTestUtils;

async function pushRouting(appRouting: Partial<AppRoutingSettings>) {
  const settings = getDefaultSettings();
  await util.ipc.settings[''].notify({
    ...settings,
    appRouting: { ...settings.appRouting, appExitsEnabled: true, ...appRouting },
  });
}

async function openRoute(name: string) {
  await page.getByRole('button', { name: 'Add an app' }).click();
  await page.getByTestId('add-app-screen').getByRole('button', { name }).click();
  await expect(page.getByTestId('route-screen')).toBeVisible();
}

test.describe('App routing on Linux', () => {
  test.beforeAll(async () => {
    process.env.WARREN_E2E_PLATFORM = 'linux';
    ({ page, util } = await startMockedApp());
    delete process.env.WARREN_E2E_PLATFORM;
    const routes = new RoutesObjectModel(page, util);
    await util.expectRoute(RoutePath.main);
    await util.ipc.splitTunneling.isSupported.notify(true);
    await util.ipc.appRouting.getApplications.handle({ fromCache: false, applications });
    await pushRouting({});

    await routes.main.gotoSettings();
    await routes.settings.gotoSplitTunnelingSettings();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('opens an app outside the VPN from its desktop entry, since Linux keeps no list', async () => {
    await openRoute('Signal');
    await expect(page.getByTestId('route-outside')).toHaveCount(0);
    const [launched] = await Promise.all([
      util.ipc.linuxSplitTunneling.launchApplication.expect({ success: true }),
      page.getByTestId('route-open-outside').click(),
    ]);
    expect(launched).toBe(SIGNAL.launchPath);
    await page.screenshot({ path: `${SCREENSHOTS}/17-linux-route.png` });
    await page.keyboard.press('Escape');
  });

  test('says why a sandboxed or scripted app takes no country', async () => {
    await openRoute('GIMP');
    await expect(page.getByTestId('route-country')).toBeDisabled();
    await expect(page.getByTestId('route-country')).toContainText(
      'Flatpak apps cannot use a country yet',
    );
    await page.keyboard.press('Escape');

    await openRoute('Firefox');
    await expect(page.getByTestId('route-country')).toContainText(
      'Opens through a script: pick its program with Find another app',
    );
    // Opening it outside the VPN goes through its desktop entry, which works,
    // but a browser already open keeps its window where it was.
    await expect(page.getByTestId('route-open-outside')).toBeEnabled();
    await expect(page.getByTestId('route-open-outside')).toContainText(
      'If it’s already running, close Firefox before launching it from here.',
    );
    await page.keyboard.press('Escape');
  });

  test('opens an app through the VPN while the others connect directly', async () => {
    await pushRouting({
      splitMode: 'include-only',
      appExits: [{ app: SIGNAL.absolutepath, exit: { country: 'se' } }],
    });
    await expect(
      page.getByText('Direct connection by default. Only the apps below use the VPN.'),
    ).toBeVisible();
    // The row says the country waits for the app to be opened from Warren.
    await expect(page.getByTestId('app-rules')).toContainText(
      'It uses this country when you open it with “Open through the VPN”.',
    );
    await page
      .getByTestId('app-rules')
      .getByRole('button', { name: /^Signal/ })
      .click();
    await expect(page.getByTestId('route-vpn')).toHaveCount(0);
    await expect(page.getByRole('note')).toHaveText(
      'It uses this country when you open it with “Open through the VPN”.',
    );
    const [launched] = await Promise.all([
      util.ipc.linuxSplitTunneling.launchIncludedApplication.expect({ success: true }),
      page.getByTestId('route-open-vpn').click(),
    ]);
    expect(launched).toBe(SIGNAL.launchPath);
    await page.screenshot({ path: `${SCREENSHOTS}/18-linux-include.png` });
    await page.keyboard.press('Escape');
  });

  test('does not claim that nothing uses the VPN, since opened apps do', async () => {
    await pushRouting({ splitMode: 'include-only', appExits: [] });
    await expect(page.getByRole('note')).toHaveCount(0);
    await expect(
      page.getByText(
        'No rules. Add an app to open it through the VPN: it uses the VPN until you close it.',
      ),
    ).toBeVisible();
  });
});
