import { expect, test } from '@playwright/test';
import { Page } from 'playwright';

import { getDefaultSettings } from '../../../src/main/default-settings';
import { ISplitTunnelingApplication } from '../../../src/shared/application-types';
import {
  AppRouteStatus,
  AppRoutingSettings,
  ILocation,
  IRelayListCountry,
  ISettings,
} from '../../../src/shared/daemon-rpc-types';
import { RoutePath } from '../../../src/shared/routes';
import { mockData } from '../mock-data';
import { RoutesObjectModel } from '../route-object-models';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

// Screenshots of each state, looked at by whoever changes this view.
const SCREENSHOTS = process.env.APP_ROUTING_SCREENSHOTS ?? 'test-results/app-routing';

const icon = (color: string, letter: string) =>
  `data:image/svg+xml;utf8,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" rx="14" fill="${color}"/><text x="32" y="43" font-family="Arial" font-size="30" font-weight="bold" fill="white" text-anchor="middle">${letter}</text></svg>`,
  )}`;

const app = (name: string, color: string): ISplitTunnelingApplication => ({
  name,
  absolutepath: `/Applications/${name}.app/Contents/MacOS/${name.toLowerCase()}`,
  icon: icon(color, name[0]),
  deletable: false,
});

const FIREFOX = app('Firefox', '#e66000');
const SLACK = app('Slack', '#4a154b');
const STEAM = app('Steam', '#1b2838');
const SPOTIFY = app('Spotify', '#1db954');
const applications = [FIREFOX, SLACK, STEAM, SPOTIFY];

const relay = mockData.relayList.countries[0].cities[0].relays[0];
const country = (
  name: string,
  code: string,
  cities: Array<[string, string]>,
): IRelayListCountry => ({
  name,
  code,
  cities: cities.map(([cityName, cityCode]) => ({
    name: cityName,
    code: cityCode,
    latitude: 0,
    longitude: 0,
    relays: [{ ...relay, hostname: `${code}-${cityCode}-1` }],
  })),
});
const relayList = {
  countries: [
    country('Sweden', 'se', [
      ['Gothenburg', 'got'],
      ['Stockholm', 'sto'],
    ]),
    country('Germany', 'de', [
      ['Berlin', 'ber'],
      ['Frankfurt', 'fra'],
    ]),
    country('France', 'fr', [['Paris', 'par']]),
    country('Switzerland', 'ch', [['Zurich', 'zrh']]),
    country('Japan', 'jp', [['Tokyo', 'tyo']]),
  ],
};

const location: ILocation = {
  country: 'Sweden',
  city: 'Gothenburg',
  latitude: 58,
  longitude: 12,
  mullvadExitIp: true,
};

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;

function settingsWith(appRouting: Partial<AppRoutingSettings>): ISettings {
  const settings = getDefaultSettings();
  return {
    ...settings,
    appRouting: {
      splitMode: 'off',
      excludedApps: [],
      includedApps: [],
      appExitsEnabled: true,
      appExits: [],
      ...appRouting,
    },
  };
}

async function pushState(appRouting: Partial<AppRoutingSettings>, statuses: AppRouteStatus[] = []) {
  await util.ipc.settings[''].notify(settingsWith(appRouting));
  await util.ipc.appRouting.applications.notify(applications);
  await util.ipc.appRouting.routes.notify(statuses);
}

const tab = (name: string) => page.getByRole('tab', { name });
const screenshot = (name: string) => page.screenshot({ path: `${SCREENSHOTS}/${name}.png` });

test.describe.configure({ mode: 'serial' });

test.describe('App routing', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);
    await util.expectRoute(RoutePath.main);

    await util.ipc.relays[''].notify({
      relayList,
      wireguardEndpointData: mockData.wireguardEndpointData,
    });
    await util.ipc.macOsSplitTunneling.needFullDiskPermissions.handle(false);
    await util.ipc.splitTunneling.getApplications.handle({ fromCache: false, applications });
    await util.ipc.appRouting.getApplications.handle({ fromCache: false, applications });
    await pushState({});

    await routes.main.gotoSettings();
    await routes.settings.gotoSplitTunnelingSettings();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('opens on Bypass VPN under its new name', async () => {
    await expect(page.getByRole('heading', { level: 1, name: 'App routing' })).toBeVisible();
    await expect(tab('Bypass VPN')).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('tabpanel')).toContainText(
      'The apps you choose connect as if the VPN were off.',
    );
    await screenshot('01-bypass');
  });

  test('moves between tabs with the arrow keys', async () => {
    await tab('Bypass VPN').focus();
    await page.keyboard.press('ArrowRight');

    await expect(tab('Country per app')).toHaveAttribute('aria-selected', 'true');
    await expect(tab('Country per app')).toBeFocused();
    await expect(page.getByTestId('apps-without-country')).toContainText('Spotify');
    const description = page.getByRole('tabpanel');
    await expect(description).toContainText(
      'Choose the country an app appears from. Other apps keep your main connection.',
    );
    await expect(description).not.toContainText('at a time');
    await screenshot('02-countries-empty');
  });

  test('gives an app a country in one click', async () => {
    await page.getByRole('button', { name: 'Choose a country for Firefox' }).click();
    const picker = page.getByTestId('country-picker');
    await expect(picker).toBeVisible();
    await screenshot('03-country-picker');

    await picker.getByPlaceholder('Search for...').fill('swe');
    const [request] = await Promise.all([
      util.ipc.appRouting.setAppExit.expect(undefined),
      picker.getByRole('button', { name: 'Sweden', exact: true }).click(),
    ]);
    expect(request).toEqual({ application: FIREFOX, exit: { country: 'se' } });

    await pushState({ appExits: [{ app: FIREFOX.absolutepath, exit: { country: 'se' } }] }, [
      {
        exit: { country: 'se' },
        state: 'connected',
        publicIp: '198.51.100.7',
        apps: [FIREFOX.absolutepath],
      },
    ]);

    const routed = page.getByTestId('apps-with-country');
    await expect(routed).toContainText('Firefox');
    await expect(routed).toContainText('Connected, IP 198.51.100.7');
    await screenshot('04-country-set');
  });

  test('offers a third country while two are in use', async () => {
    await pushState(
      {
        appExits: [
          { app: FIREFOX.absolutepath, exit: { country: 'se' } },
          { app: SLACK.absolutepath, exit: { country: 'de', city: 'ber' } },
        ],
      },
      [
        {
          exit: { country: 'se' },
          state: 'connected',
          publicIp: '198.51.100.7',
          apps: [FIREFOX.absolutepath],
        },
        {
          exit: { country: 'de', city: 'ber' },
          state: 'connected',
          publicIp: '198.51.100.8',
          apps: [SLACK.absolutepath],
        },
      ],
    );

    await page.getByRole('button', { name: 'Choose a country for Steam' }).click();
    const picker = page.getByTestId('country-picker');
    await expect(picker.getByRole('button', { name: /^Sweden/ })).toContainText('In use');
    await expect(picker.locator('[aria-disabled="true"]')).toHaveCount(0);
    await expect(picker.getByRole('note')).toHaveCount(0);
    // Only the app's own country opens on its cities.
    await expect(picker.getByRole('button', { name: /^Berlin/ })).toHaveCount(0);
    await screenshot('05-third-country-picker');

    const [request] = await Promise.all([
      util.ipc.appRouting.setAppExit.expect(undefined),
      picker.getByRole('button', { name: /^France/ }).click(),
    ]);
    expect(request).toEqual({ application: STEAM, exit: { country: 'fr' } });
    await expect(picker).not.toBeVisible();
  });

  test('saves a fourth country and says when its route waits for a free one', async () => {
    const threeCountries = [
      { app: FIREFOX.absolutepath, exit: { country: 'se' } },
      { app: SLACK.absolutepath, exit: { country: 'de', city: 'ber' } },
      { app: STEAM.absolutepath, exit: { country: 'fr' } },
    ];
    await pushState({ appExits: threeCountries });

    await page.getByRole('button', { name: 'Choose a country for Spotify' }).click();
    const picker = page.getByTestId('country-picker');
    await expect(picker.locator('[aria-disabled="true"]')).toHaveCount(0);
    const [request] = await Promise.all([
      util.ipc.appRouting.setAppExit.expect(undefined),
      picker.getByRole('button', { name: /^Switzerland/ }).click(),
    ]);
    expect(request).toEqual({ application: SPOTIFY, exit: { country: 'ch' } });

    await pushState(
      { appExits: [...threeCountries, { app: SPOTIFY.absolutepath, exit: { country: 'ch' } }] },
      [
        {
          exit: { country: 'se' },
          state: 'connected',
          publicIp: '198.51.100.7',
          apps: [FIREFOX.absolutepath],
        },
        {
          exit: { country: 'de', city: 'ber' },
          state: 'connected',
          publicIp: '198.51.100.8',
          apps: [SLACK.absolutepath],
        },
        {
          exit: { country: 'fr' },
          state: 'unavailable',
          reason: 'limit-reached',
          apps: [STEAM.absolutepath],
        },
        {
          exit: { country: 'ch' },
          state: 'unavailable',
          reason: 'waiting-for-route',
          apps: [SPOTIFY.absolutepath],
        },
      ],
    );

    const rows = page.getByTestId('apps-with-country').getByTestId('app-country-row');
    await expect(rows).toHaveCount(4);
    await expect(rows.filter({ hasText: 'Spotify' })).toContainText('Waiting for a free route');
    await expect(rows.filter({ hasText: 'Steam' })).toContainText('Session limit reached');
    await screenshot('06-four-countries');
  });

  test('asks once before VPN only for leaves the device unprotected', async () => {
    await tab('VPN only for').click();
    await page.getByRole('switch').click();

    const dialog = page.getByTestId('mode-change-dialog');
    await expect(dialog).toContainText('The rest of this device will not be protected.');
    await screenshot('07-include-only-confirm');

    const [mode] = await Promise.all([
      util.ipc.appRouting.setSplitMode.expect(undefined),
      dialog.getByRole('button', { name: 'Turn on' }).click(),
    ]);
    expect(mode).toBe('include-only');
  });

  test('keeps a warning in the tab while VPN only for is on', async () => {
    await pushState({
      splitMode: 'include-only',
      includedApps: [SLACK.absolutepath],
      appExits: [{ app: FIREFOX.absolutepath, exit: { country: 'se' } }],
    });

    await expect(page.getByTestId('include-only-banner')).toContainText(
      'Only these apps are protected. Everything else on this device uses your normal connection.',
    );
    const included = page.getByTestId('included-applications');
    await expect(included).toContainText('Slack');
    await expect(included).toContainText('Uses the VPN through its country');
    await screenshot('08-include-only-on');
  });

  test('says Bypass VPN replaces VPN only for and keeps the lists', async () => {
    await tab('Bypass VPN').click();
    await page.getByRole('switch').click();

    const dialog = page.getByTestId('mode-change-dialog');
    await expect(dialog).toContainText('This replaces VPN only for');
    await screenshot('09-bypass-replaces');
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await expect(dialog).not.toBeVisible();
  });

  test('labels the main screen and leads back to the tab', async () => {
    await pushState(
      {
        splitMode: 'include-only',
        includedApps: [SLACK.absolutepath],
        appExits: [
          { app: FIREFOX.absolutepath, exit: { country: 'se' } },
          { app: STEAM.absolutepath, exit: { country: 'de' } },
        ],
      },
      [
        {
          exit: { country: 'se' },
          state: 'connected',
          publicIp: '198.51.100.7',
          apps: [FIREFOX.absolutepath],
        },
        { exit: { country: 'de' }, state: 'connecting', apps: [STEAM.absolutepath] },
      ],
    );
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

    await page.getByRole('button', { name: 'Back' }).click();
    await util.expectRoute(RoutePath.settings);
    await page.getByRole('button', { name: 'Close' }).click();
    await util.expectRoute(RoutePath.main);

    await expect(page.getByTestId('include-only-label')).toHaveText('VPN only for 3 apps');
    await expect(page.getByText('Only selected apps are protected')).toBeVisible();
    await expect(page.getByText('You are protected')).not.toBeVisible();
    await expect(page.getByTestId('app-countries-indicator')).toHaveText(
      '2 apps in other countries',
    );
    await screenshot('10-main-screen');

    await page.getByTestId('include-only-label').click();
    await util.expectRoute(RoutePath.splitTunneling);
    await expect(tab('VPN only for')).toHaveAttribute('aria-selected', 'true');
  });

  test('explains a macOS build that cannot run Bypass VPN', async () => {
    test.skip(process.platform !== 'darwin', 'the signed-build state is macOS only');
    await util.ipc.splitTunneling.isSupported.notify(false);
    await tab('Bypass VPN').click();

    await expect(page.getByRole('note')).toContainText('It needs a signed build.');
    await expect(page.getByRole('switch')).toBeDisabled();
    await screenshot('11-needs-signed-build');
  });

  test('fits its French copy', async () => {
    await util.ipc.splitTunneling.isSupported.notify(true);
    await page.getByRole('button', { name: 'Close' }).click();
    await util.expectRoute(RoutePath.main);

    await routes.main.gotoSettings();
    await routes.settings.gotoUserInterfaceSettings();
    await routes.userInterfaceSettings.gotoSelectLanguage();
    await routes.selectLanguage.selectLanguage('Français');
    await routes.selectLanguage.goBack();
    // The back button is the first one, whatever its translated label.
    await util.expectRouteChange(() => page.getByRole('button').first().click());
    await page.getByRole('button', { name: 'Routage des apps' }).click();
    await util.expectRoute(RoutePath.splitTunneling);

    await expect(tab('VPN ciblé')).toHaveAttribute('aria-selected', 'true');
    await screenshot('12-fr-include-only');
    await tab('Pays par app').click();
    await screenshot('13-fr-countries');
    await page.getByRole('button', { name: 'Choisir un pays pour Spotify' }).click();
    await screenshot('14-fr-picker');
    await page.keyboard.press('Escape');

    await util.expectRouteChange(() => page.getByRole('button').first().click());
    await util.expectRouteChange(() => page.getByRole('button').first().click());
    await util.expectRoute(RoutePath.main);
    await screenshot('15-fr-main');
  });
});
