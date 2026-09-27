import { expect, test } from '@playwright/test';
import { readFileSync } from 'fs';
import path from 'path';
import { Page } from 'playwright';

import {
  ILocation,
  IRelayList,
  IRelayListHostname,
  ITunnelEndpoint,
} from '../../../src/shared/daemon-rpc-types';
import { RoutePath } from '../../../src/shared/routes';
import { mockData } from '../mock-data';
import { RoutesObjectModel } from '../route-object-models';
import { NavigationObjectModel } from '../route-object-models/navigation';
import { MockedTestUtils, startMockedApp } from './mocked-utils';

// Screenshots for a human to look at; nothing asserts on pixels.
const SHOTS = process.env.WARREN_SHOTS_DIR ?? '/tmp/warren-desktop-shots';

const LIVE_ID = 'ab'.repeat(16);
const QUIET_ID = 'cd'.repeat(16);
const OFFLINE_ID = 'ef'.repeat(16);
const LIVE_HOST = 'warren-abababababababab';
const QUIET_HOST = 'warren-cdcdcdcdcdcdcdcd';
const OFFLINE_HOST = 'warren-efefefefefefefef';

function relay(hostname: string): IRelayListHostname {
  return {
    hostname,
    provider: 'warren',
    ipv4AddrIn: '10.0.0.1',
    includeInCountry: true,
    active: true,
    weight: 100,
    owned: true,
    daita: false,
    lwo: false,
  };
}

const relayList: IRelayList = {
  countries: [
    {
      name: 'Finland',
      code: 'fi',
      cities: [
        { name: 'Helsinki', code: 'hel', latitude: 0, longitude: 0, relays: [relay(OFFLINE_HOST)] },
      ],
    },
    {
      name: 'France',
      code: 'fr',
      cities: [
        { name: 'Paris', code: 'par', latitude: 0, longitude: 0, relays: [relay(LIVE_HOST)] },
      ],
    },
    {
      name: 'Romania',
      code: 'ro',
      cities: [
        { name: 'Bucharest', code: 'buc', latitude: 0, longitude: 0, relays: [relay(QUIET_HOST)] },
      ],
    },
  ],
};

// warren-contract's frozen fixture, dated a few seconds ago, plus an offline
// exit so every exit state is on screen.
function snapshotJson(): string {
  const fixture = JSON.parse(
    readFileSync(path.resolve(import.meta.dirname, '../../fixtures/network-stats-v1.json'), 'utf8'),
  );
  fixture.generated_at = Math.floor(Date.now() / 1000) - 12;
  fixture.exits[0].history = Array.from({ length: 60 }, (_, index) => ({
    t: fixture.generated_at - (59 - index) * 60,
    connected: 40,
    throughput_bps: Math.round(260e6 + 60e6 * Math.sin(index / 6) + index * 1e6),
    load_percent: 35,
  }));
  // The reference server withholds the uptime, and counts the fleet history
  // by clock hour.
  delete fixture.exits[0].uptime_secs;
  fixture.history = Array.from({ length: 24 }, (_, index) => ({
    t: fixture.generated_at - (23 - index) * 3600,
    connected: Math.round(45 + 20 * Math.sin(index / 3)),
    throughput_bps: Math.round(400e6 + 250e6 * Math.sin(index / 3 + 0.5)),
  }));
  fixture.exits.push({
    exit_id: OFFLINE_ID,
    country: 'FI',
    city: 'Helsinki',
    online: false,
    live: false,
    connected: 0,
    download_bps: 0,
    upload_bps: 0,
    history: [],
  });
  return JSON.stringify(fixture);
}

let page: Page;
let util: MockedTestUtils;
let routes: RoutesObjectModel;

test.describe('Network stats', () => {
  test.beforeAll(async () => {
    ({ page, util } = await startMockedApp());
    routes = new RoutesObjectModel(page, util);
    await routes.main.waitForRoute();

    await util.ipc.relays[''].notify({
      relayList,
      wireguardEndpointData: mockData.wireguardEndpointData,
    });
    await util.ipc.warrenNetworkStats.get.handle({
      result: 'ok',
      snapshotJson: snapshotJson(),
      exitHostnames: [
        { exitId: LIVE_ID, hostname: LIVE_HOST },
        { exitId: QUIET_ID, hostname: QUIET_HOST },
        { exitId: OFFLINE_ID, hostname: OFFLINE_HOST },
      ],
    });
    // The poller only runs while the window has focus.
    await util.ipc.window.focus.notify(true);
  });

  test.afterAll(async () => {
    await util?.closePage();
  });

  test('location rows show a flag and the load of their exit', async () => {
    await routes.main.gotoSelectLocation();

    const badges = page.getByTestId('exit-load-badge');
    await expect(badges).toHaveCount(3);
    await expect(badges.filter({ hasText: '37%' })).toHaveText(/37%.*40\+.*300 Mbit\/s/);
    await expect(badges.filter({ hasText: '< 20' })).toHaveAttribute(
      'aria-label',
      'Moderate load, < 20 people',
    );
    await expect(badges.filter({ hasText: 'Offline' })).toHaveCount(1);
    // One exit per country: every row selects its exit, none opens.
    await expect(page.locator('img[src$="flags/fr.svg"]')).toHaveCount(1);
    await expect(page.getByRole('button', { name: /^Expand / })).toHaveCount(0);
    // Nothing to fill in yet: no empty sections, no headings over a single
    // list, no custom-list menu before a custom list exists.
    await expect(page.getByText('No recent selection history')).toHaveCount(0);
    await expect(page.getByRole('heading', { name: 'Custom lists' })).toHaveCount(0);
    await expect(page.getByRole('heading', { name: 'All locations' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: /^Open menu for / })).toHaveCount(0);

    await page.screenshot({ path: `${SHOTS}/location-list.png` });

    // A custom list is created from the header menu.
    await page.getByRole('button', { name: 'Open select location menu' }).click();
    await expect(page.getByRole('menu').getByText('New custom list')).toBeVisible();
    await page.keyboard.press('Escape');
    await new NavigationObjectModel(page, util).goBackToRoute(RoutePath.main);
  });

  test('a country with several exits opens onto one row per exit', async () => {
    const second = 'warren-abababababababac';
    const withTwoInFrance: IRelayList = {
      countries: relayList.countries.map((country) =>
        country.code !== 'fr'
          ? country
          : {
              ...country,
              cities: [{ ...country.cities[0], relays: [relay(LIVE_HOST), relay(second)] }],
            },
      ),
    };
    await util.ipc.relays[''].notify({
      relayList: withTwoInFrance,
      wireguardEndpointData: mockData.wireguardEndpointData,
    });
    await routes.main.gotoSelectLocation();

    await page.getByRole('button', { name: 'Expand France' }).click();
    await page.getByRole('button', { name: 'Expand Paris' }).click();
    await expect(page.getByText(LIVE_HOST)).toBeVisible();
    await expect(page.getByText(second)).toBeVisible();
    await expect(page.getByTestId('exit-load-badge').filter({ hasText: '37%' })).toHaveCount(1);
    await page.waitForTimeout(400);
    await page.screenshot({ path: `${SHOTS}/location-list-expanded.png` });

    await util.ipc.relays[''].notify({
      relayList,
      wireguardEndpointData: mockData.wireguardEndpointData,
    });
    await new NavigationObjectModel(page, util).goBackToRoute(RoutePath.main);
  });

  for (const [name, hostname, country, city] of [
    ['band-only', QUIET_HOST, 'Romania', 'Bucharest'],
    ['live', LIVE_HOST, 'France', 'Paris'],
  ] as const) {
    test(`the connection card shows the ${name} exit without growing`, async () => {
      // Same card, connected to an exit the snapshot does not know, is the
      // baseline: the load shares the hostname line and adds nothing to the
      // expanded details.
      await connectTo('warren-0000000000000000', country, city);
      await expect(page.getByTestId('connected-exit-load')).toHaveCount(0);
      const collapsed = await cardTop();
      await routes.main.expandConnectionPanel();
      await page.waitForTimeout(600);
      const expanded = await cardTop();
      await routes.main.expandConnectionPanel();
      await page.waitForTimeout(600);

      await connectTo(hostname, country, city);
      await expect(page.getByTestId('connected-exit-load')).toBeVisible();
      await page.waitForTimeout(600);

      expect(await cardTop()).toBe(collapsed);
      await page.screenshot({ path: `${SHOTS}/connect-${name}.png` });

      await routes.main.expandConnectionPanel();
      await page.waitForTimeout(600);
      expect(await cardTop()).toBe(expanded);
      await page.screenshot({ path: `${SHOTS}/connect-${name}-details.png` });
      await routes.main.expandConnectionPanel();
      await page.waitForTimeout(600);
    });
  }

  test('the expanded card keeps its chevron on screen in a short window', async () => {
    // The Electron window cannot be resized from here; the page can.
    const short = await page.addStyleTag({
      content: 'html, body, #app { height: 480px !important; overflow: hidden; }',
    });
    await routes.main.expandConnectionPanel();
    await page.waitForTimeout(600);

    const chevron = await page.getByTestId('connection-panel-chevron').boundingBox();
    expect(chevron!.y).toBeGreaterThanOrEqual(0);
    await expect(page.getByRole('button', { name: 'Disconnect' })).toBeInViewport({ ratio: 1 });

    await page.screenshot({ path: `${SHOTS}/connect-short-window.png` });
    await routes.main.expandConnectionPanel();
    await short.evaluate((style) => (style as HTMLStyleElement).remove());
    await page.waitForTimeout(600);
  });

  test('the load on the connection card opens nothing', async () => {
    await page.getByTestId('connected-exit-load').click();
    await util.expectRoute(RoutePath.main);
  });
});

async function cardTop(): Promise<number | undefined> {
  // The top edge of the connection card, which the scenery rule forbids to rise.
  const card = page.getByTestId('connection-panel-chevron').locator('..');
  return (await card.boundingBox())?.y;
}

async function connectTo(hostname: string, country: string, city: string) {
  const location: ILocation = {
    country,
    city,
    latitude: 0,
    longitude: 0,
    mullvadExitIp: true,
    hostname,
  };
  const endpoint: ITunnelEndpoint = {
    address: '10.0.0.1:443',
    protocol: 'udp',
    quantumResistant: false,
    daita: false,
    tunnelType: 'wireguard',
  };
  await util.ipc.tunnel[''].notify({
    state: 'connected',
    details: { endpoint, location },
    featureIndicators: undefined,
  });
  // The location and hostname lines open with a 300 ms transition.
  await page.waitForTimeout(600);
}
