import { expect, test } from '@playwright/test';
import fs from 'fs';
import { po } from 'gettext-parser';
import path from 'path';
import { Locator, Page } from 'playwright';

import { getDefaultSettings } from '../../../src/main/default-settings';
import { ISplitTunnelingApplication } from '../../../src/shared/application-types';
import {
  AppRouteStatus,
  AppRoutingSettings,
  ILocation,
  IRelayListCountry,
} from '../../../src/shared/daemon-rpc-types';
import { RoutePath } from '../../../src/shared/routes';
import { textDirection } from '../../../src/shared/text-direction';
import { mockData } from '../mock-data';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

// The App routing view is narrow (three tabs side by side, chips and badges), so every catalog
// is rendered here and checked for clipped labels, and Arabic and Persian are checked to be laid
// out right to left. Screenshots land in <APP_ROUTING_SCREENSHOTS>/<locale>/ for whoever changes
// the copy.
// APP_ROUTING_LOCALES=all renders every catalog; the default is a sample of scripts and lengths.
const SCREENSHOTS = process.env.APP_ROUTING_SCREENSHOTS ?? 'test-results/app-routing';
const LOCALES_DIR = path.resolve(import.meta.dirname, '../../../locales');
const ALL_LOCALES = fs
  .readdirSync(LOCALES_DIR)
  .filter((entry) => fs.statSync(path.join(LOCALES_DIR, entry)).isDirectory());
const requested = process.env.APP_ROUTING_LOCALES ?? 'de,ro,ar,ja';
const LOCALES = requested === 'all' ? ALL_LOCALES : requested.split(',');

function catalog(locale: string) {
  const parsed = po.parse(fs.readFileSync(path.join(LOCALES_DIR, locale, 'messages.po')));
  // An empty context looks up the messages extracted without one.
  return (context: string, msgid: string) => {
    const msgstr = parsed.translations[context]?.[msgid]?.msgstr[0];
    expect(msgstr, `${locale} translates "${msgid}"`).toBeTruthy();
    return msgstr!;
  };
}

const app = (name: string): ISplitTunnelingApplication => ({
  name,
  absolutepath: `/Applications/${name}.app/Contents/MacOS/${name.toLowerCase()}`,
  deletable: false,
});
const FIREFOX = app('Firefox');
const SLACK = app('Slack');
const STEAM = app('Steam');
const SPOTIFY = app('Spotify');
const applications = [FIREFOX, SLACK, STEAM, SPOTIFY];

const relay = mockData.relayList.countries[0].cities[0].relays[0];
const country = (
  name: string,
  code: string,
  city: string,
  cityCode: string,
): IRelayListCountry => ({
  name,
  code,
  cities: [
    {
      name: city,
      code: cityCode,
      latitude: 0,
      longitude: 0,
      relays: [{ ...relay, hostname: `${code}-${cityCode}-1` }],
    },
  ],
});
const relayList = {
  countries: [
    country('Sweden', 'se', 'Gothenburg', 'got'),
    country('Germany', 'de', 'Berlin', 'ber'),
    country('Switzerland', 'ch', 'Zurich', 'zrh'),
    country('Japan', 'jp', 'Tokyo', 'tyo'),
  ],
};

const location: ILocation = {
  country: 'Sweden',
  city: 'Gothenburg',
  latitude: 58,
  longitude: 12,
  mullvadExitIp: true,
};

const appRouting: AppRoutingSettings = {
  splitMode: 'include-only',
  excludedApps: [SPOTIFY.absolutepath],
  includedApps: [SLACK.absolutepath, FIREFOX.absolutepath],
  appExitsEnabled: true,
  appExits: [
    { app: FIREFOX.absolutepath, exit: { country: 'se' } },
    { app: STEAM.absolutepath, exit: { country: 'de' } },
  ],
};

const statuses: AppRouteStatus[] = [
  {
    exit: { country: 'se' },
    state: 'connected',
    publicIp: '198.51.100.7',
    apps: [FIREFOX.absolutepath],
  },
  { exit: { country: 'de' }, state: 'unavailable', reason: 'no-token', apps: [STEAM.absolutepath] },
];

// Text an element cannot show in full: wider than its box and not ellipsized on purpose.
async function clipped(locator: Locator): Promise<string[]> {
  return locator.evaluateAll((elements) =>
    elements
      .filter(
        (element) =>
          element.scrollWidth > element.clientWidth + 1 &&
          getComputedStyle(element).textOverflow !== 'ellipsis',
      )
      .map((element) => element.textContent ?? ''),
  );
}

for (const locale of LOCALES) {
  test.describe(`App routing in ${locale}`, () => {
    let page: Page;
    let util: MockedTestUtils;
    const t = catalog(locale);
    const shot = (name: string) =>
      page.screenshot({ path: `${SCREENSHOTS}/${locale}/${name}.png` });

    test.beforeAll(async () => {
      process.env.WARREN_E2E_LOCALE = locale;
      ({ page, util } = await startMockedApp());
      delete process.env.WARREN_E2E_LOCALE;
      await util.expectRoute(RoutePath.main);

      await util.ipc.relays[''].notify({
        relayList,
        wireguardEndpointData: mockData.wireguardEndpointData,
      });
      await util.ipc.macOsSplitTunneling.needFullDiskPermissions.handle(false);
      await util.ipc.splitTunneling.getApplications.handle({ fromCache: false, applications });
      await util.ipc.appRouting.getApplications.handle({ fromCache: false, applications });
      await util.ipc.settings[''].notify({ ...getDefaultSettings(), appRouting });
      await util.ipc.appRouting.applications.notify(applications);
      await util.ipc.appRouting.routes.notify(statuses);
      await util.ipc.tunnel[''].notify({
        state: 'connected',
        details: {
          endpoint: {
            address: 'wg10:80',
            protocol: 'udp',
            quantumResistant: false,
            daita: false,
            tunnelType: 'wireguard',
          },
          location,
        },
        featureIndicators: undefined,
      });
    });

    test.afterAll(async () => {
      await util?.closePage();
    });

    test('lays out in the reading direction of the catalog', async () => {
      const root = page.locator('html');
      await expect(root).toHaveAttribute('lang', locale);
      await expect(root).toHaveAttribute('dir', textDirection(locale));
    });

    test('fits the main screen labels', async () => {
      await expect(
        page.getByText(t('tunnel-control', 'Only selected apps are protected')),
      ).toBeVisible();
      const label = page.getByTestId('include-only-label');
      await expect(label).toBeVisible();
      await expect(page.getByTestId('app-countries-indicator')).toBeVisible();
      expect(await clipped(label)).toEqual([]);
      await shot('01-main');
    });

    test('renders the settings', async () => {
      await page.getByRole('button', { name: t('', 'Settings'), exact: true }).click();
      await util.expectRoute(RoutePath.settings);
      await expect(
        page.getByRole('heading', { level: 1, name: t('settings-view', 'Settings') }),
      ).toBeVisible();
      await shot('07-settings');

      await page.getByText(t('settings-view', 'User interface settings'), { exact: true }).click();
      await util.expectRoute(RoutePath.userInterfaceSettings);
      await shot('08-user-interface-settings');

      await page.getByRole('button', { name: t('', 'Back'), exact: true }).click();
      await util.expectRoute(RoutePath.settings);
      await page.getByRole('button', { name: t('', 'Close'), exact: true }).click();
      await util.expectRoute(RoutePath.main);
    });

    test('fits the three tabs', async () => {
      await page.getByTestId('include-only-label').click();
      await util.expectRoute(RoutePath.splitTunneling);
      await expect(
        page.getByRole('heading', { level: 1, name: t('split-tunneling-view', 'App routing') }),
      ).toBeVisible();

      const tabs = page.getByRole('tab');
      await expect(tabs).toHaveText([
        t('split-tunneling-view', 'Bypass VPN'),
        t('split-tunneling-view', 'Country per app'),
        t('split-tunneling-view', 'VPN only for'),
      ]);
      expect(await clipped(tabs)).toEqual([]);
      // The tabs share the bar equally; a word too long to wrap widens its tab and squeezes the
      // other two instead of overflowing, so a clip check alone would not see it.
      const widths = await tabs.evaluateAll((elements) =>
        elements.map((element) => element.getBoundingClientRect().width),
      );
      expect(Math.max(...widths) - Math.min(...widths)).toBeLessThan(2);
      // The first tab sits on the side the catalog is read from.
      const [first, last] = await Promise.all([
        tabs.nth(0).boundingBox(),
        tabs.nth(2).boundingBox(),
      ]);
      if (textDirection(locale) === 'rtl') {
        expect(first!.x).toBeGreaterThan(last!.x);
      } else {
        expect(first!.x).toBeLessThan(last!.x);
      }
      await shot('02-include-only');

      await tabs.nth(1).click();
      await expect(page.getByTestId('apps-with-country')).toBeVisible();
      await shot('03-countries');

      await tabs.nth(0).click();
      await shot('04-bypass');
    });

    test('fits the country picker', async () => {
      await page.getByRole('tab').nth(1).click();
      await page.getByTestId('apps-without-country').getByRole('button').first().click();
      const picker = page.getByTestId('country-picker');
      await expect(picker).toBeVisible();
      expect(await clipped(picker.getByRole('button'))).toEqual([]);
      await shot('05-country-picker');
      await page.keyboard.press('Escape');
      await expect(picker).not.toBeVisible();
    });

    test('fits the mode change dialog', async () => {
      await page.getByRole('tab').nth(0).click();
      await page.getByRole('switch').click();
      const dialog = page.getByTestId('mode-change-dialog');
      await expect(dialog).toBeVisible();
      expect(await clipped(dialog.getByRole('button'))).toEqual([]);
      await shot('06-mode-change');
      await page.keyboard.press('Escape');
      await expect(dialog).not.toBeVisible();
    });
  });
}
