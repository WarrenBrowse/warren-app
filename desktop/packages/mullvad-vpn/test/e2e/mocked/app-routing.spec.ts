import { expect, test } from '@playwright/test';
import { ElectronApplication, Page } from 'playwright';

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

const FIREFOX = app('Firefox', '#b8430d');
const BANK = app('My bank', '#2f6b57');
const QBIT = app('qBittorrent', '#2f5fa8');
const SIGNAL = app('Signal', '#2c58c4');
const STEAM = app('Steam', '#3b5b7a');
const applications = [FIREFOX, BANK, QBIT, SIGNAL, STEAM];

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
    country('Germany', 'de', [
      ['Berlin', 'ber'],
      ['Frankfurt', 'fra'],
    ]),
    country('Finland', 'fi', [['Helsinki', 'hel']]),
    country('France', 'fr', [['Paris', 'par']]),
    country('Netherlands', 'nl', [['Amsterdam', 'ams']]),
    country('Romania', 'ro', [['Bucharest', 'buh']]),
    country('Singapore', 'sg', [['Singapore', 'sin']]),
  ],
};

const location: ILocation = {
  country: 'Germany',
  city: 'Berlin',
  latitude: 52,
  longitude: 13,
  mullvadExitIp: true,
};

let electronApp: ElectronApplication;
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

// The daemon calls the view makes, in order, named after their IPC group.
type RoutingCall = [string, unknown];

function routingEvents(): Record<string, string> {
  return {
    [util.ipc.appRouting.setSplitMode.eventKey]: 'setSplitMode',
    [util.ipc.appRouting.addIncludedApp.eventKey]: 'addIncludedApp',
    [util.ipc.appRouting.removeIncludedApp.eventKey]: 'removeIncludedApp',
    [util.ipc.appRouting.setAppExitsEnabled.eventKey]: 'setAppExitsEnabled',
    [util.ipc.appRouting.setAppExit.eventKey]: 'setAppExit',
    [util.ipc.appRouting.clearAppExit.eventKey]: 'clearAppExit',
    [util.ipc.splitTunneling.addApplication.eventKey]: 'addExcluded',
    [util.ipc.splitTunneling.removeApplication.eventKey]: 'removeExcluded',
  };
}

async function recordRoutingCalls() {
  await electronApp.evaluate(({ ipcMain }, events) => {
    const store = globalThis as { routingCalls?: Array<[string, unknown]> };
    store.routingCalls = [];
    for (const event of events) {
      ipcMain.removeHandler(event);
      ipcMain.handle(event, (_event, arg) => {
        store.routingCalls!.push([event, arg]);
        return { type: 'success', value: undefined };
      });
    }
  }, Object.keys(routingEvents()));
}

async function takeRoutingCalls(count: number): Promise<RoutingCall[]> {
  const names = routingEvents();
  let calls: Array<[string, unknown]> = [];
  await expect
    .poll(async () => {
      calls = await electronApp.evaluate(() => {
        const store = globalThis as { routingCalls?: Array<[string, unknown]> };
        return store.routingCalls ?? [];
      });
      return calls.length;
    })
    .toBe(count);
  await electronApp.evaluate(() => {
    (globalThis as { routingCalls?: unknown[] }).routingCalls = [];
  });
  return calls.map(([event, arg]) => [names[event], arg]);
}

const screenshot = (name: string) => page.screenshot({ path: `${SCREENSHOTS}/${name}.png` });
const rules = () => page.getByTestId('app-rules');
const segment = (name: string) =>
  page.getByRole('group', { name: 'Other apps go' }).getByRole('button', { name });

test.describe.configure({ mode: 'serial' });

test.describe('App routing', () => {
  test.beforeAll(async () => {
    ({ app: electronApp, page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);
    await util.expectRoute(RoutePath.main);

    await util.ipc.relays[''].notify({
      relayList,
      wireguardEndpointData: mockData.wireguardEndpointData,
    });
    await util.ipc.macOsSplitTunneling.needFullDiskPermissions.handle(false);
    await util.ipc.splitTunneling.isSupported.notify(true);
    await util.ipc.appRouting.getApplications.handle({ fromCache: false, applications });
    await pushState({});
    await recordRoutingCalls();

    await routes.main.gotoSettings();
    await routes.settings.gotoSplitTunnelingSettings();
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('opens on one list with the VPN as the default, and no switch', async () => {
    await expect(page.getByRole('heading', { level: 1, name: 'App routing' })).toBeVisible();
    await expect(page.getByRole('tab')).toHaveCount(0);
    await expect(page.getByRole('switch')).toHaveCount(0);
    await expect(segment('Through the VPN')).toHaveAttribute('aria-pressed', 'true');
    await expect(segment('Outside the VPN')).toHaveAttribute('aria-pressed', 'false');
    await expect(
      page.getByText('All apps are protected by the VPN, except for the rules below.'),
    ).toBeVisible();
    await expect(
      page.getByText('No rules. Add an app to send it through another country or outside the VPN.'),
    ).toBeVisible();
    await screenshot('01-empty');
  });

  test('gives an app a country from the list, with no switch to turn on first', async () => {
    await page.getByRole('button', { name: 'Add an app' }).click();
    const add = page.getByTestId('add-app-screen');
    await expect(add.getByRole('heading', { name: 'Add an app' })).toBeVisible();
    await expect(add).toContainText('Steam');
    await screenshot('02-add-app');

    await add.getByRole('button', { name: 'Firefox' }).click();
    const route = page.getByTestId('route-screen');
    await expect(route.getByRole('heading', { name: 'Route for Firefox' })).toBeVisible();
    await expect(page.getByTestId('route-vpn')).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByTestId('route-vpn')).toContainText('Default');
    await expect(route.getByRole('button', { name: 'Remove the rule' })).toHaveCount(0);

    await page.getByTestId('route-country').click();
    const countries = page.getByTestId('country-screen');
    await expect(countries.getByRole('heading', { name: 'Country for Firefox' })).toBeVisible();
    await screenshot('03-country');
    await countries.getByPlaceholder('Search for...').fill('neth');
    await countries.getByRole('button', { name: 'Netherlands', exact: true }).click();

    expect(await takeRoutingCalls(1)).toEqual([
      ['setAppExit', { application: FIREFOX, exit: { country: 'nl' } }],
    ]);
    await pushState({ appExits: [{ app: FIREFOX.absolutepath, exit: { country: 'nl' } }] }, [
      {
        exit: { country: 'nl' },
        state: 'connected',
        publicIp: '198.51.100.7',
        apps: [FIREFOX.absolutepath],
      },
    ]);
    await expect(page.getByTestId('route-country')).toContainText('Netherlands');
    await expect(page.getByTestId('route-country')).toContainText(
      'Through the VPN, from this country',
    );
    await expect(route.getByRole('button', { name: 'Remove the rule' })).toBeVisible();
    await screenshot('04-route');

    await route.getByRole('button', { name: 'Done' }).click();
    await expect(rules()).toContainText('Firefox');
    await expect(rules()).toContainText('Netherlands');
    await expect(rules()).toContainText('Connected, IP 198.51.100.7');
  });

  test('sends an app outside the VPN, which turns the bypass on by itself', async () => {
    await page.getByRole('button', { name: 'Add an app' }).click();
    await page.getByTestId('add-app-screen').getByRole('button', { name: 'Steam' }).click();
    await page.getByTestId('route-outside').click();

    expect(await takeRoutingCalls(2)).toEqual([
      ['addExcluded', STEAM],
      ['setSplitMode', 'exclude'],
    ]);
    await pushState({
      splitMode: 'exclude',
      excludedApps: [STEAM.absolutepath, BANK.absolutepath],
      appExits: [
        { app: FIREFOX.absolutepath, exit: { country: 'nl' } },
        { app: QBIT.absolutepath, exit: { country: 'ro' } },
      ],
    });
    await expect(page.getByTestId('route-outside')).toHaveAttribute('aria-pressed', 'true');
    await page.getByRole('button', { name: 'Done' }).click();

    await expect(page.getByText('Rules per app4')).toBeVisible();
    const names = await rules().getByRole('button').allInnerTexts();
    expect(names.map((text) => text.split('\n')[0])).toEqual([
      'Firefox',
      'My bank',
      'qBittorrent',
      'Steam',
    ]);
    await expect(rules().getByRole('button', { name: 'Steam, Outside the VPN' })).toBeVisible();
    await expect(rules().getByRole('button', { name: 'qBittorrent, Romania' })).toBeVisible();
    await screenshot('05-rules');
  });

  test('turns the bypass off with the rule of its last app', async () => {
    await pushState({ splitMode: 'exclude', excludedApps: [STEAM.absolutepath] });
    await rules().getByRole('button', { name: 'Steam, Outside the VPN' }).click();
    await page.getByRole('button', { name: 'Remove the rule' }).click();

    expect(await takeRoutingCalls(2)).toEqual([
      ['removeExcluded', STEAM.absolutepath],
      ['setSplitMode', 'off'],
    ]);
    await expect(page.getByTestId('route-screen')).toHaveCount(0);
  });

  test('makes "Outside the VPN" the default, keeping the countries', async () => {
    await pushState({
      splitMode: 'exclude',
      excludedApps: [STEAM.absolutepath],
      appExits: [{ app: QBIT.absolutepath, exit: { country: 'ro' } }],
    });
    await segment('Outside the VPN').click();

    expect(await takeRoutingCalls(2)).toEqual([
      ['removeExcluded', STEAM.absolutepath],
      ['setSplitMode', 'include-only'],
    ]);
    await pushState({
      splitMode: 'include-only',
      appExits: [{ app: QBIT.absolutepath, exit: { country: 'ro' } }],
    });
    await expect(segment('Outside the VPN')).toHaveAttribute('aria-pressed', 'true');
    await expect(
      page.getByText('Direct connection by default. Only the apps below use the VPN.'),
    ).toBeVisible();
    await expect(rules().getByRole('button', { name: 'qBittorrent, Romania' })).toBeVisible();
  });

  test('puts an app on the VPN while the others connect directly', async () => {
    await page.getByRole('button', { name: 'Add an app' }).click();
    await page.getByTestId('add-app-screen').getByRole('button', { name: 'Signal' }).click();
    await expect(page.getByTestId('route-outside')).toContainText('Default');
    await page.getByTestId('route-vpn').click();

    expect(await takeRoutingCalls(1)).toEqual([['addIncludedApp', SIGNAL]]);
    await pushState({
      splitMode: 'include-only',
      includedApps: [SIGNAL.absolutepath, FIREFOX.absolutepath],
      appExits: [{ app: QBIT.absolutepath, exit: { country: 'ro' } }],
    });
    await page.getByRole('button', { name: 'Done' }).click();
    await expect(rules().getByRole('button', { name: 'Signal, Through the VPN' })).toBeVisible();
    await screenshot('06-outside-default');
  });

  test('warns when nothing uses the VPN', async () => {
    await pushState({ splitMode: 'include-only' });
    await expect(page.getByRole('note')).toContainText(
      'No app uses the VPN. Add one, or send the other apps through the VPN again.',
    );
    await screenshot('07-outside-empty');

    await segment('Through the VPN').click();
    expect(await takeRoutingCalls(1)).toEqual([['setSplitMode', 'off']]);
  });

  test('labels the main screen and leads back to the view', async () => {
    await pushState(
      {
        splitMode: 'include-only',
        includedApps: [SIGNAL.absolutepath],
        appExits: [
          { app: FIREFOX.absolutepath, exit: { country: 'nl' } },
          { app: QBIT.absolutepath, exit: { country: 'ro' } },
        ],
      },
      [
        {
          exit: { country: 'nl' },
          state: 'connected',
          publicIp: '198.51.100.7',
          apps: [FIREFOX.absolutepath],
        },
        { exit: { country: 'ro' }, state: 'connecting', apps: [QBIT.absolutepath] },
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
    await expect(page.getByTestId('app-countries-indicator')).toHaveText(
      '2 apps in other countries',
    );
    await screenshot('08-main-screen');

    await page.getByTestId('include-only-label').click();
    await util.expectRoute(RoutePath.splitTunneling);
    await expect(segment('Outside the VPN')).toHaveAttribute('aria-pressed', 'true');
  });

  test('keeps "Outside the VPN" out of reach of a build that cannot run it', async () => {
    test.skip(process.platform !== 'darwin', 'the signed-build state is macOS only');
    await pushState({ appExits: [{ app: FIREFOX.absolutepath, exit: { country: 'nl' } }] });
    await util.ipc.splitTunneling.isSupported.notify(false);

    await expect(segment('Outside the VPN')).toBeDisabled();
    await expect(page.getByRole('note')).toContainText(
      'Outside the VPN needs a signed build of Warren VPN.',
    );
    await screenshot('09-unsigned');

    await rules().getByRole('button', { name: 'Firefox, Netherlands' }).click();
    await expect(page.getByTestId('route-outside')).toBeDisabled();
    await expect(page.getByTestId('route-outside')).toContainText(
      'Needs a signed build of Warren VPN',
    );
    // A country needs no signature.
    await expect(page.getByTestId('route-country')).toBeEnabled();
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('route-screen')).toHaveCount(0);
  });

  test('fits its French copy', async () => {
    await util.ipc.splitTunneling.isSupported.notify(true);
    await pushState(
      {
        splitMode: 'exclude',
        excludedApps: [STEAM.absolutepath, BANK.absolutepath],
        appExits: [
          { app: FIREFOX.absolutepath, exit: { country: 'nl' } },
          { app: QBIT.absolutepath, exit: { country: 'ro' } },
        ],
      },
      [
        {
          exit: { country: 'nl' },
          state: 'connected',
          publicIp: '198.51.100.7',
          apps: [FIREFOX.absolutepath],
        },
      ],
    );
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

    await expect(page.getByText('Les autres apps passent')).toBeVisible();
    await screenshot('10-fr-rules');
    await rules()
      .getByRole('button', { name: /^qBittorrent/ })
      .click();
    await expect(page.getByRole('heading', { name: 'Route de qBittorrent' })).toBeVisible();
    await screenshot('11-fr-route');
    await page.getByTestId('route-country').click();
    await screenshot('12-fr-country');
    await page.keyboard.press('Escape');
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: 'Ajouter une app' }).click();
    await screenshot('13-fr-add');
    await page.keyboard.press('Escape');

    await pushState({ splitMode: 'include-only' });
    await screenshot('14-fr-outside-empty');
  });
});
