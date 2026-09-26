import { describe, expect, it } from 'vitest';

import {
  appExitDisplayState,
  appRouteLine,
  appRoutingSummary,
  buildCountryOptions,
  countryNeedsIncludedLaunch,
  effectiveAppExits,
  effectiveIncludedApps,
  exitChoiceNames,
  exitChoicesInUse,
  modeChangeConfirmation,
  resolveApplications,
  routeStatusForApp,
  sameAppId,
  splitModeAvailability,
  wouldExceedAppExitLimit,
} from '../../src/shared/app-routing';
import type { AppRouteStatus, AppRoutingSettings } from '../../src/shared/daemon-rpc-types';

const FIREFOX = '/Applications/Firefox.app/Contents/MacOS/firefox';
const SLACK = '/Applications/Slack.app/Contents/MacOS/Slack';
const STEAM = '/Applications/Steam.app/Contents/MacOS/steam_osx';

function settings(overrides: Partial<AppRoutingSettings> = {}): AppRoutingSettings {
  return {
    splitMode: 'off',
    excludedApps: [],
    includedApps: [],
    appExitsEnabled: true,
    appExits: [],
    ...overrides,
  };
}

describe('sameAppId', () => {
  it('ignores case on Windows, where paths are case insensitive', () => {
    expect(sameAppId('C:\\Apps\\Foo.exe', 'c:\\apps\\foo.EXE', 'win32')).toBe(true);
  });

  it('respects case on Linux, where two such paths name two programs', () => {
    expect(sameAppId('/usr/bin/Foo', '/usr/bin/foo', 'linux')).toBe(false);
  });
});

describe('wouldExceedAppExitLimit mirrors the daemon rule', () => {
  const twoCountries = [
    { app: FIREFOX, exit: { country: 'se' } },
    { app: SLACK, exit: { country: 'de' } },
  ];

  it('refuses a third distinct exit', () => {
    expect(wouldExceedAppExitLimit(twoCountries, STEAM, { country: 'fr' }, 'darwin')).toBe(true);
  });

  it('accepts an exit already used by another app, since they share a session', () => {
    expect(wouldExceedAppExitLimit(twoCountries, STEAM, { country: 'se' }, 'darwin')).toBe(false);
  });

  it('accepts moving an app that holds one of the two exits to a new one', () => {
    expect(wouldExceedAppExitLimit(twoCountries, SLACK, { country: 'fr' }, 'darwin')).toBe(false);
  });

  it('counts a city as a different exit from its country', () => {
    expect(
      wouldExceedAppExitLimit(twoCountries, STEAM, { country: 'se', city: 'got' }, 'darwin'),
    ).toBe(true);
  });
});

describe('exitChoicesInUse', () => {
  it('lists each distinct exit once', () => {
    const exits = exitChoicesInUse([
      { app: FIREFOX, exit: { country: 'se' } },
      { app: SLACK, exit: { country: 'se' } },
      { app: STEAM, exit: { country: 'se', city: 'got' } },
    ]);
    expect(exits).toEqual([{ country: 'se' }, { country: 'se', city: 'got' }]);
  });
});

describe('precedence, as enforced by the daemon', () => {
  it('ignores the country of an app that bypasses the VPN while Bypass is on', () => {
    const routing = settings({
      splitMode: 'exclude',
      excludedApps: [FIREFOX],
      appExits: [
        { app: FIREFOX, exit: { country: 'se' } },
        { app: SLACK, exit: { country: 'de' } },
      ],
    });

    expect(effectiveAppExits(routing, 'darwin').map((entry) => entry.app)).toEqual([SLACK]);
    expect(appExitDisplayState(routing, FIREFOX, 'darwin')).toBe('bypassed');
  });

  it('keeps that country once Bypass is off', () => {
    const routing = settings({
      splitMode: 'off',
      excludedApps: [FIREFOX],
      appExits: [{ app: FIREFOX, exit: { country: 'se' } }],
    });

    expect(appExitDisplayState(routing, FIREFOX, 'darwin')).toBe('active');
  });

  it('puts no app in another country while the tab switch is off', () => {
    const routing = settings({
      appExitsEnabled: false,
      appExits: [{ app: FIREFOX, exit: { country: 'se' } }],
    });

    expect(effectiveAppExits(routing, 'darwin')).toEqual([]);
    expect(appExitDisplayState(routing, FIREFOX, 'darwin')).toBe('paused');
    expect(appExitDisplayState(routing, SLACK, 'darwin')).toBe('none');
  });

  it('tunnels an app with a country in include-only mode', () => {
    const routing = settings({
      splitMode: 'include-only',
      includedApps: [SLACK],
      appExits: [{ app: FIREFOX, exit: { country: 'se' } }],
    });

    expect(effectiveIncludedApps(routing, 'darwin')).toEqual([SLACK, FIREFOX]);
  });

  it('does not tunnel that app when the countries are switched off', () => {
    const routing = settings({
      splitMode: 'include-only',
      includedApps: [SLACK],
      appExitsEnabled: false,
      appExits: [{ app: FIREFOX, exit: { country: 'se' } }],
    });

    expect(effectiveIncludedApps(routing, 'darwin')).toEqual([SLACK]);
  });

  it('includes nothing outside include-only mode', () => {
    const routing = settings({ splitMode: 'exclude', includedApps: [SLACK] });

    expect(effectiveIncludedApps(routing, 'darwin')).toEqual([]);
  });

  it('counts an app both included and given a country once', () => {
    const routing = settings({
      splitMode: 'include-only',
      includedApps: [SLACK],
      appExits: [{ app: SLACK, exit: { country: 'se' } }],
    });

    expect(effectiveIncludedApps(routing, 'darwin')).toEqual([SLACK]);
  });
});

describe('routeStatusForApp', () => {
  const statuses: AppRouteStatus[] = [
    { exit: { country: 'se' }, state: 'connected', publicIp: '198.51.100.7', apps: [FIREFOX] },
    { exit: { country: 'de' }, state: 'connecting', apps: [SLACK] },
  ];

  it('finds the route that carries the app', () => {
    expect(routeStatusForApp(statuses, SLACK, 'darwin')?.exit).toEqual({ country: 'de' });
  });

  it('answers nothing for an app no route carries', () => {
    expect(routeStatusForApp(statuses, STEAM, 'darwin')).toBeUndefined();
  });
});

describe('appRoutingSummary, for the main screen', () => {
  it('counts the protected apps while include-only is on', () => {
    const routing = settings({
      splitMode: 'include-only',
      includedApps: [SLACK, STEAM],
      appExits: [{ app: FIREFOX, exit: { country: 'se' } }],
    });

    expect(appRoutingSummary(routing, [], 'darwin').vpnOnlyForCount).toBe(3);
  });

  it('has no include-only count in any other mode', () => {
    const routing = settings({ splitMode: 'exclude', includedApps: [SLACK] });

    expect(appRoutingSummary(routing, [], 'darwin').vpnOnlyForCount).toBeUndefined();
    expect(appRoutingSummary(routing, [], 'darwin').includeOnly).toBe(false);
  });

  it('says include-only is on without a count on Linux, where apps join when opened', () => {
    const routing = settings({
      splitMode: 'include-only',
      appExits: [{ app: '/usr/lib/firefox/firefox', exit: { country: 'se' } }],
    });

    const summary = appRoutingSummary(routing, [], 'linux');

    expect(summary.includeOnly).toBe(true);
    expect(summary.vpnOnlyForCount).toBeUndefined();
  });

  it('counts the apps that leave from their own country', () => {
    const routing = settings({
      splitMode: 'exclude',
      excludedApps: [STEAM],
      appExits: [
        { app: FIREFOX, exit: { country: 'se' } },
        { app: SLACK, exit: { country: 'se' } },
        { app: STEAM, exit: { country: 'de' } },
      ],
    });

    expect(appRoutingSummary(routing, [], 'darwin').appsWithOwnCountry).toBe(2);
  });

  it('flags a route that cannot run, but not one waiting for the VPN', () => {
    const routing = settings({ appExits: [{ app: FIREFOX, exit: { country: 'se' } }] });
    const down: AppRouteStatus[] = [
      { exit: { country: 'se' }, state: 'unavailable', reason: 'tunnel-down', apps: [FIREFOX] },
    ];
    const noToken: AppRouteStatus[] = [
      { exit: { country: 'se' }, state: 'unavailable', reason: 'no-token', apps: [FIREFOX] },
    ];

    expect(appRoutingSummary(routing, down, 'darwin').anyRouteUnavailable).toBe(false);
    expect(appRoutingSummary(routing, noToken, 'darwin').anyRouteUnavailable).toBe(true);
  });
});

describe('modeChangeConfirmation', () => {
  it('warns that the rest of the device is unprotected when include-only turns on', () => {
    expect(modeChangeConfirmation('off', 'include-only')).toEqual({
      leavesDeviceUnprotected: true,
      replaces: undefined,
    });
  });

  it('says include-only replaces Bypass when Bypass is on', () => {
    expect(modeChangeConfirmation('exclude', 'include-only')).toEqual({
      leavesDeviceUnprotected: true,
      replaces: 'exclude',
    });
  });

  it('says Bypass replaces include-only when include-only is on', () => {
    expect(modeChangeConfirmation('include-only', 'exclude')).toEqual({
      leavesDeviceUnprotected: false,
      replaces: 'include-only',
    });
  });

  it('asks nothing to turn Bypass on from off, or to turn a mode off', () => {
    expect(modeChangeConfirmation('off', 'exclude')).toBeUndefined();
    expect(modeChangeConfirmation('include-only', 'off')).toBeUndefined();
    expect(modeChangeConfirmation('exclude', 'off')).toBeUndefined();
  });
});

describe('buildCountryOptions, for the per-app country picker', () => {
  const relay = (active: boolean) => ({ active });
  const locations = [
    {
      name: 'Sweden',
      code: 'se',
      cities: [
        { name: 'Stockholm', code: 'sto', relays: [relay(true)] },
        { name: 'Gothenburg', code: 'got', relays: [relay(true)] },
        { name: 'Malmo', code: 'mma', relays: [relay(false)] },
      ],
    },
    {
      name: 'Germany',
      code: 'de',
      cities: [{ name: 'Berlin', code: 'ber', relays: [relay(true)] }],
    },
    {
      name: 'Chile',
      code: 'cl',
      cities: [{ name: 'Santiago', code: 'scl', relays: [relay(false)] }],
    },
  ];
  const identity = (name: string) => name;

  it('lists only countries and cities that have a server, sorted by name', () => {
    const options = buildCountryOptions(locations, '', identity);

    expect(options.map((option) => option.country)).toEqual(['de', 'se']);
    expect(options[1].cities.map((city) => city.code)).toEqual(['got', 'sto']);
  });

  it('matches the search against country and city names, keeping only matching cities', () => {
    const byCity = buildCountryOptions(locations, 'gothen', identity);
    const byCountry = buildCountryOptions(locations, 'swe', identity);

    expect(byCity.map((option) => option.country)).toEqual(['se']);
    expect(byCity[0].cities.map((city) => city.code)).toEqual(['got']);
    expect(byCountry[0].cities.map((city) => city.code)).toEqual(['got', 'sto']);
  });

  it('sorts and searches the translated names', () => {
    const french: Record<string, string> = { Sweden: 'Suede', Germany: 'Allemagne' };
    const options = buildCountryOptions(locations, 'sue', (name) => french[name] ?? name);

    expect(options.map((option) => option.name)).toEqual(['Suede']);
  });
});

describe('countryNeedsIncludedLaunch, for the Country per app tab', () => {
  it('holds on Linux in include-only mode, where only apps opened from VPN only for are in it', () => {
    expect(countryNeedsIncludedLaunch('linux', 'include-only')).toBe(true);
  });

  it('holds nowhere else: the daemon includes an app with a country itself', () => {
    expect(countryNeedsIncludedLaunch('linux', 'exclude')).toBe(false);
    expect(countryNeedsIncludedLaunch('darwin', 'include-only')).toBe(false);
  });
});

describe('splitModeAvailability, for Bypass VPN and VPN only for', () => {
  const base = { supported: true, needsFullDiskAccess: false, isMacOs13OrNewer: true };

  it('is available where the daemon supports it', () => {
    expect(splitModeAvailability({ ...base, platform: 'win32' })).toBe('available');
    expect(splitModeAvailability({ ...base, platform: 'darwin' })).toBe('available');
  });

  it('is unsupported on a Windows or Linux system the daemon refuses', () => {
    expect(splitModeAvailability({ ...base, platform: 'linux', supported: false })).toBe(
      'unsupported',
    );
  });

  it('needs a signed build on a macOS build the daemon refuses', () => {
    expect(splitModeAvailability({ ...base, platform: 'darwin', supported: false })).toBe(
      'needs-signed-build',
    );
  });

  it('needs macOS 13 before anything else', () => {
    expect(
      splitModeAvailability({
        ...base,
        platform: 'darwin',
        supported: false,
        isMacOs13OrNewer: false,
      }),
    ).toBe('needs-newer-macos');
  });

  it('needs Full Disk Access on macOS until it is granted', () => {
    expect(splitModeAvailability({ ...base, platform: 'darwin', needsFullDiskAccess: true })).toBe(
      'needs-full-disk-access',
    );
  });

  it('is still being checked on macOS while the answer is unknown', () => {
    expect(
      splitModeAvailability({ ...base, platform: 'darwin', needsFullDiskAccess: undefined }),
    ).toBe('checking');
  });
});

describe('appRouteLine, the status under an app with a country', () => {
  const routing = settings({ appExits: [{ app: FIREFOX, exit: { country: 'se' } }] });
  const connected: AppRouteStatus = {
    exit: { country: 'se' },
    state: 'connected',
    publicIp: '198.51.100.7',
    apps: [FIREFOX],
  };

  it('shows the address the app appears from once its route is up', () => {
    expect(appRouteLine(routing, [connected], FIREFOX, 'darwin')).toEqual({
      kind: 'connected',
      publicIp: '198.51.100.7',
    });
  });

  it('shows why a route cannot run', () => {
    const noRelay: AppRouteStatus = { ...connected, state: 'unavailable', reason: 'no-relay' };

    expect(appRouteLine(routing, [noRelay], FIREFOX, 'darwin')).toEqual({
      kind: 'unavailable',
      reason: 'no-relay',
    });
  });

  it('shows the route connecting', () => {
    const connecting: AppRouteStatus = { ...connected, state: 'connecting' };

    expect(appRouteLine(routing, [connecting], FIREFOX, 'darwin')).toEqual({
      kind: 'connecting',
    });
  });

  it('waits for the VPN while no route carries the app', () => {
    expect(appRouteLine(routing, [], FIREFOX, 'darwin')).toEqual({ kind: 'waiting' });
  });

  it('says the country is off rather than any stale route state', () => {
    const off = { ...routing, appExitsEnabled: false };

    expect(appRouteLine(off, [connected], FIREFOX, 'darwin')).toEqual({ kind: 'paused' });
  });

  it('says the app bypasses the VPN when Bypass takes it out', () => {
    const bypass = { ...routing, splitMode: 'exclude' as const, excludedApps: [FIREFOX] };

    expect(appRouteLine(bypass, [], FIREFOX, 'darwin')).toEqual({ kind: 'bypassed' });
  });
});

describe('exitChoiceNames, for labels', () => {
  const locations = [
    {
      name: 'Sweden',
      code: 'se',
      cities: [{ name: 'Gothenburg', code: 'got', relays: [{ active: true }] }],
    },
  ];
  const french = (name: string) => ({ Sweden: 'Suede', Gothenburg: 'Goteborg' })[name] ?? name;

  it('names the country and city from the relay list, translated', () => {
    expect(exitChoiceNames({ country: 'se', city: 'got' }, locations, french)).toEqual({
      country: 'Suede',
      city: 'Goteborg',
    });
  });

  it('falls back to the code of a location the relay list no longer has', () => {
    expect(exitChoiceNames({ country: 'xk', city: 'prn' }, locations, french)).toEqual({
      country: 'XK',
      city: 'PRN',
    });
  });
});

describe('resolveApplications, which names the apps a list of ids holds', () => {
  const firefox = { absolutepath: FIREFOX, name: 'Firefox', deletable: false };
  const slack = { absolutepath: SLACK, name: 'Slack', icon: 'data:slack', deletable: true };

  it('prefers the name the main process read for the id', () => {
    const catalog = [{ ...firefox, name: 'Firefox from the catalog' }];

    expect(resolveApplications([FIREFOX], [firefox], catalog, 'darwin')).toEqual([firefox]);
  });

  it('falls back to the app list, whatever the case of the id', () => {
    expect(resolveApplications([SLACK.toLowerCase()], [], [slack], 'darwin')).toEqual([slack]);
  });

  it('names an app nobody knows after its file', () => {
    expect(resolveApplications(['C:\\Tools\\curl.exe'], [], [], 'win32')).toEqual([
      { absolutepath: 'C:\\Tools\\curl.exe', name: 'curl.exe', deletable: false },
    ]);
  });
});
