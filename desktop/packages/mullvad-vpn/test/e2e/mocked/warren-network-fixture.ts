import { readFileSync } from 'fs';
import path from 'path';

import { IRelayList, IRelayListHostname } from '../../../src/shared/daemon-rpc-types';
import { MockedTestUtils } from './mocked-utils';

// Screenshots for a human to look at; nothing asserts on pixels.
export const SHOTS = process.env.WARREN_SHOTS_DIR ?? '/tmp/warren-desktop-shots';

export const LIVE_ID = 'ab'.repeat(16);
export const QUIET_ID = 'cd'.repeat(16);
export const OFFLINE_ID = 'ef'.repeat(16);
export const LIVE_HOST = 'warren-abababababababab';
export const QUIET_HOST = 'warren-cdcdcdcdcdcdcdcd';
export const OFFLINE_HOST = 'warren-efefefefefefefef';

export function relay(hostname: string): IRelayListHostname {
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

export const relayList: IRelayList = {
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
export function snapshotJson(): string {
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

/** Serves the relay list and the network stats snapshot above. */
export async function serveWarrenNetwork(util: MockedTestUtils) {
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
}
