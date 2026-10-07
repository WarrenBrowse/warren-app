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

// The App routing view is narrow (a two-way switch, chips and badges on 400 px), so every
// catalog is rendered here and checked for clipped labels, and Arabic and Persian are checked to
// be laid out right to left. Screenshots land in <APP_ROUTING_SCREENSHOTS>/<locale>/ for whoever changes
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
    { app: SLACK.absolutepath, exit: { country: 'ch' } },
  ],
  lockedApps: [],
};

const statuses: AppRouteStatus[] = [
  {
    exit: { country: 'se' },
    state: 'connected',
    publicIp: '198.51.100.7',
    apps: [FIREFOX.absolutepath],
  },
  {
    exit: { country: 'de' },
    state: 'unavailable',
    reason: 'limit-reached',
    apps: [STEAM.absolutepath],
  },
  {
    exit: { country: 'ch' },
    state: 'unavailable',
    reason: 'waiting-for-route',
    apps: [SLACK.absolutepath],
  },
];

// Text an element cannot show in full, even where it would end in an ellipsis.
async function truncated(locator: Locator): Promise<string[]> {
  return locator.evaluateAll((elements) =>
    elements
      .filter((element) => element.scrollWidth > element.clientWidth + 1)
      .map((element) => element.textContent ?? ''),
  );
}

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

    test('fits the list of rules', async () => {
      await page.getByTestId('include-only-label').click();
      await util.expectRoute(RoutePath.splitTunneling);
      await expect(
        page.getByRole('heading', { level: 1, name: t('split-tunneling-view', 'App routing') }),
      ).toBeVisible();

      const segments = page
        .getByRole('group', { name: t('split-tunneling-view', 'Other apps go') })
        .getByRole('button');
      await expect(segments).toHaveText([
        t('split-tunneling-view', 'Through the VPN'),
        t('split-tunneling-view', 'Outside the VPN'),
      ]);
      expect(await clipped(segments)).toEqual([]);
      // The two choices share the bar equally, whatever the length of either.
      const widths = await segments.evaluateAll((elements) =>
        elements.map((element) => element.getBoundingClientRect().width),
      );
      expect(Math.max(...widths) - Math.min(...widths)).toBeLessThan(2);
      // The first choice sits on the side the catalog is read from.
      const [first, last] = await Promise.all([
        segments.nth(0).boundingBox(),
        segments.nth(1).boundingBox(),
      ]);
      if (textDirection(locale) === 'rtl') {
        expect(first!.x).toBeGreaterThan(last!.x);
      } else {
        expect(first!.x).toBeLessThan(last!.x);
      }

      const rules = page.getByTestId('app-rules');
      await expect(rules.getByRole('button')).toHaveCount(3);
      // A status line ends in an ellipsis rather than wrap, so a translation that does not
      // fit would lose its end: both lines must show in full.
      for (const line of ['Waiting for a free route', 'Session limit reached']) {
        const text = t('split-tunneling-view', line);
        const element = rules.getByTitle(text, { exact: true });
        await expect(element).toHaveText(text);
        expect(await truncated(element)).toEqual([]);
      }
      await shot('02-rules-outside-default');

      await util.ipc.settings[''].notify({
        ...getDefaultSettings(),
        appRouting: { ...appRouting, splitMode: 'exclude', includedApps: [] },
      });
      await expect(rules.getByRole('button')).toHaveCount(4);
      // The chips of a rule never cut their route short.
      expect(await truncated(rules.locator('[aria-hidden="true"] > span'))).toEqual([]);
      await shot('03-rules-vpn-default');
    });

    test('fits the route of an app', async () => {
      await page.getByTestId('app-rules').getByRole('button').first().click();
      const route = page.getByTestId('route-screen');
      await expect(route).toBeVisible();
      expect(await clipped(route.getByRole('button'))).toEqual([]);
      await shot('04-route');

      await page.getByTestId('route-country').click();
      const countries = page.getByTestId('country-screen');
      await expect(countries).toBeVisible();
      expect(await clipped(countries.getByRole('button'))).toEqual([]);
      await shot('05-country');
      await page.keyboard.press('Escape');
      await page.keyboard.press('Escape');
      await expect(route).not.toBeVisible();
    });

    test('fits the list of apps to add', async () => {
      await page.getByRole('button', { name: t('split-tunneling-view', 'Add an app') }).click();
      const add = page.getByTestId('add-app-screen');
      await expect(add).toBeVisible();
      expect(await clipped(add.getByRole('button'))).toEqual([]);
      await shot('06-add');
      await page.keyboard.press('Escape');
      await expect(add).not.toBeVisible();
    });

    test('fits the warning of a device where nothing uses the VPN', async () => {
      await util.ipc.settings[''].notify({
        ...getDefaultSettings(),
        appRouting: { ...appRouting, includedApps: [], appExits: [] },
      });
      await expect(page.getByRole('note')).toHaveText(
        t(
          'split-tunneling-view',
          'No app uses the VPN. Add one, or send the other apps through the VPN again.',
        ),
      );
      await shot('10-outside-empty');
    });

    test('fits the shared networks of Local network sharing', async () => {
      const networks = ['10.0.0.0/8', '192.168.0.0/16', 'fc00::/7', '400::/7'];
      await util.ipc.settings[''].notify({
        ...getDefaultSettings(),
        appRouting,
        allowLan: true,
        lanNetworks: { networks, custom: true },
      });
      await page.getByRole('button', { name: t('', 'Close'), exact: true }).click();
      await util.expectRoute(RoutePath.main);
      await page.getByRole('button', { name: t('', 'Settings'), exact: true }).click();
      await util.expectRoute(RoutePath.settings);
      await page.getByRole('button', { name: t('settings-view', 'VPN settings') }).click();
      await util.expectRoute(RoutePath.vpnSettings);

      const view = page;
      await expect(
        view.getByText(
          t('vpn-settings-view', 'Traffic to these networks goes outside the VPN tunnel.'),
        ),
      ).toBeVisible();
      // The list is folded behind its summary row.
      const sharedNetworks = view.getByRole('button', {
        name: t('vpn-settings-view', 'Shared networks'),
      });
      expect(await clipped(sharedNetworks)).toEqual([]);
      await sharedNetworks.click();
      // Addresses read left to right in every catalog, right-to-left ones included.
      await expect(view.getByText('192.168.0.0/16', { exact: true })).toHaveCSS('direction', 'ltr');
      const reset = view.getByText(t('vpn-settings-view', 'Reset to default'), { exact: true });
      await expect(reset).toBeVisible();
      const add = view.getByText(t('vpn-settings-view', 'Add a network'), { exact: true });
      await add.click();
      const field = view.getByPlaceholder(
        t('vpn-settings-view', 'Enter a network, e.g. 192.168.1.0/24'),
      );
      await field.fill('0.0.0.0/0');
      await page.keyboard.press('Enter');
      await expect(
        view.getByText(
          t('vpn-settings-view', 'This network is too broad to be shared outside the tunnel.'),
        ),
      ).toBeVisible();
      expect(await clipped(reset)).toEqual([]);
      expect(await clipped(add)).toEqual([]);
      await shot('09-shared-networks');

      await field.blur();
      await page.getByRole('button', { name: t('', 'Back'), exact: true }).click();
      await util.expectRoute(RoutePath.settings);
      await page.getByRole('button', { name: t('', 'Close'), exact: true }).click();
      await util.expectRoute(RoutePath.main);
    });
  });
}
