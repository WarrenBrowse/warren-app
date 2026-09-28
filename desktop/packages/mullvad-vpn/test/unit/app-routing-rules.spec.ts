import { describe, expect, it } from 'vitest';

import {
  applyRoutingOps,
  type AppRoute,
  appRouteFor,
  appRules,
  defaultRoute,
  planAppRoute,
  planDefaultRoute,
  type RoutingOp,
  routingReflects,
} from '../../src/shared/app-routing';
import type { AppRoutingSettings, ExitChoice } from '../../src/shared/daemon-rpc-types';

const FIREFOX = '/Applications/Firefox.app/Contents/MacOS/firefox';
const SLACK = '/Applications/Slack.app/Contents/MacOS/Slack';
const STEAM = '/Applications/Steam.app/Contents/MacOS/steam_osx';
const SIGNAL = '/Applications/Signal.app/Contents/MacOS/Signal';
const APPS = [FIREFOX, SLACK, STEAM, SIGNAL];

const RO: ExitChoice = { country: 'ro' };
const NL: ExitChoice = { country: 'nl' };
const DE_BER: ExitChoice = { country: 'de', city: 'ber' };

const VPN: AppRoute = { kind: 'vpn' };
const DIRECT: AppRoute = { kind: 'direct' };
const country = (exit: ExitChoice): AppRoute => ({ kind: 'country', exit });

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

// Every state the daemon passes through while it applies the operations one
// by one, the starting state first.
function statesAlong(routing: AppRoutingSettings, ops: RoutingOp[]): AppRoutingSettings[] {
  const states = [routing];
  for (const op of ops) {
    states.push(applyRoutingOps(states[states.length - 1], [op], 'darwin'));
  }
  return states;
}

// The whole contract of a change to one app: it ends on the route asked for,
// never passes through a route that is neither the old nor the new one (the
// operations are separate calls, so each intermediate state is live traffic),
// and no other app moves at any step.
function expectCleanChange(routing: AppRoutingSettings, app: string, next: AppRoute) {
  const before = appRouteFor(routing, app, 'darwin');
  const ops = planAppRoute(routing, app, next, 'darwin');
  const states = statesAlong(routing, ops);
  const others = APPS.filter((other) => other !== app);

  for (const state of states) {
    expect([before, next]).toContainEqual(appRouteFor(state, app, 'darwin'));
    for (const other of others) {
      expect(appRouteFor(state, other, 'darwin')).toEqual(appRouteFor(routing, other, 'darwin'));
    }
  }
  expect(appRouteFor(states[states.length - 1], app, 'darwin')).toEqual(next);
  return states[states.length - 1];
}

describe('defaultRoute, what the apps without a rule do', () => {
  it('is the VPN unless VPN only for is on', () => {
    expect(defaultRoute(settings())).toBe('vpn');
    expect(defaultRoute(settings({ splitMode: 'exclude' }))).toBe('vpn');
    expect(defaultRoute(settings({ splitMode: 'include-only' }))).toBe('direct');
  });
});

describe('appRules, the list the view shows', () => {
  it('shows nothing for a fresh install', () => {
    expect(appRules(settings(), 'darwin')).toEqual([]);
  });

  it('shows excluded apps outside the VPN and apps with a country, the VPN being the default', () => {
    const routing = settings({
      splitMode: 'exclude',
      excludedApps: [STEAM],
      appExits: [{ app: FIREFOX, exit: NL }],
    });
    expect(appRules(routing, 'darwin')).toEqual([
      { app: STEAM, route: DIRECT },
      { app: FIREFOX, route: country(NL) },
    ]);
  });

  it('shows the included apps on the VPN and apps with a country, direct being the default', () => {
    const routing = settings({
      splitMode: 'include-only',
      includedApps: [SIGNAL],
      appExits: [{ app: FIREFOX, exit: RO }],
    });
    expect(appRules(routing, 'darwin')).toEqual([
      { app: SIGNAL, route: VPN },
      { app: FIREFOX, route: country(RO) },
    ]);
  });

  it('shows one rule per app, the one the daemon applies', () => {
    const excludedWithCountry = settings({
      splitMode: 'exclude',
      excludedApps: [STEAM],
      appExits: [{ app: STEAM, exit: NL }],
    });
    expect(appRules(excludedWithCountry, 'darwin')).toEqual([{ app: STEAM, route: DIRECT }]);

    const includedWithCountry = settings({
      splitMode: 'include-only',
      includedApps: [STEAM],
      appExits: [{ app: STEAM, exit: NL }],
    });
    expect(appRules(includedWithCountry, 'darwin')).toEqual([{ app: STEAM, route: country(NL) }]);
  });

  it('hides what is saved but not in force', () => {
    const routing = settings({
      splitMode: 'off',
      excludedApps: [STEAM],
      includedApps: [SIGNAL],
      appExitsEnabled: false,
      appExits: [{ app: FIREFOX, exit: NL }],
    });
    expect(appRules(routing, 'darwin')).toEqual([]);
  });

  it('matches app ids the way the platform compares paths', () => {
    const routing = settings({
      splitMode: 'exclude',
      excludedApps: ['C:\\Games\\Steam.exe'],
      appExits: [{ app: 'c:\\games\\steam.EXE', exit: NL }],
    });
    expect(appRules(routing, 'win32')).toEqual([{ app: 'C:\\Games\\Steam.exe', route: DIRECT }]);
  });

  it('takes no list on Linux, where bypass and VPN only for are launch based', () => {
    const routing = settings({
      splitMode: 'include-only',
      excludedApps: ['/usr/bin/steam'],
      includedApps: ['/usr/bin/signal-desktop'],
      appExits: [{ app: '/usr/lib/firefox/firefox', exit: RO }],
    });
    expect(appRules(routing, 'linux')).toEqual([
      { app: '/usr/lib/firefox/firefox', route: country(RO) },
    ]);
  });
});

describe('planAppRoute with the VPN as the default', () => {
  it('turns bypass on for the first app sent outside the VPN, dropping a list saved while it was off', () => {
    const routing = settings({ splitMode: 'off', excludedApps: [SLACK] });
    const after = expectCleanChange(routing, STEAM, DIRECT);
    expect(after.splitMode).toBe('exclude');
    expect(after.excludedApps).toEqual([STEAM]);
  });

  it('adds to bypass without touching the mode when it is already on', () => {
    const routing = settings({ splitMode: 'exclude', excludedApps: [SLACK] });
    const ops = planAppRoute(routing, STEAM, DIRECT, 'darwin');
    expect(ops).toEqual([{ op: 'add-excluded', app: STEAM }]);
    expectCleanChange(routing, STEAM, DIRECT);
  });

  it('turns bypass off with the last app brought back to the VPN', () => {
    const routing = settings({ splitMode: 'exclude', excludedApps: [STEAM] });
    const after = expectCleanChange(routing, STEAM, VPN);
    expect(after.splitMode).toBe('off');
    expect(after.excludedApps).toEqual([]);
  });

  it('keeps bypass on while other apps still bypass', () => {
    const routing = settings({ splitMode: 'exclude', excludedApps: [STEAM, SLACK] });
    const after = expectCleanChange(routing, STEAM, VPN);
    expect(after.splitMode).toBe('exclude');
    expect(after.excludedApps).toEqual([SLACK]);
  });

  it('gives a country without a switch, dropping countries saved while they were off', () => {
    const routing = settings({ appExitsEnabled: false, appExits: [{ app: SLACK, exit: NL }] });
    const after = expectCleanChange(routing, FIREFOX, country(RO));
    expect(after.appExitsEnabled).toBe(true);
    expect(after.appExits).toEqual([{ app: FIREFOX, exit: RO }]);
  });

  it('moves an app from one country to another in one call', () => {
    const routing = settings({ appExits: [{ app: FIREFOX, exit: NL }] });
    expect(planAppRoute(routing, FIREFOX, country(DE_BER), 'darwin')).toEqual([
      { op: 'set-exit', app: FIREFOX, exit: DE_BER },
    ]);
    expectCleanChange(routing, FIREFOX, country(DE_BER));
  });

  it('moves a bypassing app to a country, and back outside the VPN', () => {
    const bypassing = settings({ splitMode: 'exclude', excludedApps: [STEAM] });
    const withCountry = expectCleanChange(bypassing, STEAM, country(NL));
    expect(withCountry.splitMode).toBe('off');
    const back = expectCleanChange(withCountry, STEAM, DIRECT);
    expect(back.appExits).toEqual([]);
  });

  it('clears the country and the bypass entry of an app the daemon saw in both', () => {
    const routing = settings({
      splitMode: 'exclude',
      excludedApps: [STEAM],
      appExits: [{ app: STEAM, exit: NL }],
    });
    const after = expectCleanChange(routing, STEAM, VPN);
    expect(after.appExits).toEqual([]);
    expect(after.excludedApps).toEqual([]);
  });

  it('does nothing when the app already takes that route', () => {
    expect(planAppRoute(settings(), FIREFOX, VPN, 'darwin')).toEqual([]);
    const routing = settings({ appExits: [{ app: FIREFOX, exit: NL }] });
    expect(planAppRoute(routing, FIREFOX, country({ country: 'nl' }), 'darwin')).toEqual([]);
  });
});

describe('planAppRoute with direct as the default', () => {
  const base = { splitMode: 'include-only' as const };

  it('puts an app on the VPN', () => {
    const after = expectCleanChange(settings(base), SIGNAL, VPN);
    expect(after.includedApps).toEqual([SIGNAL]);
  });

  it('never lets an app with a country leave the VPN on its way back to the main country', () => {
    const routing = settings({ ...base, appExits: [{ app: FIREFOX, exit: RO }] });
    const after = expectCleanChange(routing, FIREFOX, VPN);
    expect(after.appExits).toEqual([]);
    expect(after.includedApps).toEqual([FIREFOX]);
  });

  it('never lets an included app leave the VPN on its way to a country', () => {
    const routing = settings({ ...base, includedApps: [SIGNAL], appExitsEnabled: false });
    const after = expectCleanChange(routing, SIGNAL, country(NL));
    expect(after.includedApps).toEqual([]);
    expect(after.appExitsEnabled).toBe(true);
  });

  it('sends an app back outside the VPN by removing its rule', () => {
    const routing = settings({
      ...base,
      includedApps: [FIREFOX],
      appExits: [{ app: FIREFOX, exit: RO }],
    });
    const after = expectCleanChange(routing, FIREFOX, DIRECT);
    expect(after.includedApps).toEqual([]);
    expect(after.appExits).toEqual([]);
  });

  it('leaves the mode alone when the last rule goes, since direct is what the user chose', () => {
    const routing = settings({ ...base, includedApps: [SIGNAL] });
    const after = expectCleanChange(routing, SIGNAL, DIRECT);
    expect(after.splitMode).toBe('include-only');
  });
});

describe('planAppRoute on Linux', () => {
  it('stores countries only, since leaving or joining the VPN is a launch there', () => {
    const routing = settings();
    expect(() => planAppRoute(routing, '/usr/bin/steam', DIRECT, 'linux')).toThrow();
    expect(() =>
      planAppRoute(settings({ splitMode: 'include-only' }), '/usr/bin/steam', VPN, 'linux'),
    ).toThrow();
    expect(planAppRoute(routing, '/usr/bin/steam', country(NL), 'linux')).toEqual([
      { op: 'set-exit', app: '/usr/bin/steam', exit: NL },
    ]);
  });
});

describe('planDefaultRoute, the "Other apps" choice', () => {
  it('turns VPN only for on, keeps the countries and drops the bypass list', () => {
    const routing = settings({
      splitMode: 'exclude',
      excludedApps: [STEAM],
      includedApps: [SLACK],
      appExits: [{ app: FIREFOX, exit: NL }],
    });
    const after = applyRoutingOps(routing, planDefaultRoute(routing, 'direct', 'darwin'), 'darwin');
    expect(after.splitMode).toBe('include-only');
    expect(after.excludedApps).toEqual([]);
    // A list saved earlier would come back as rules nobody just chose.
    expect(after.includedApps).toEqual([]);
    expect(appRules(after, 'darwin')).toEqual([{ app: FIREFOX, route: country(NL) }]);
  });

  it('turns VPN only for off first, so no app is ever outside the VPN on the way back', () => {
    const routing = settings({
      splitMode: 'include-only',
      includedApps: [SIGNAL],
      excludedApps: [STEAM],
      appExits: [{ app: FIREFOX, exit: RO }],
    });
    const ops = planDefaultRoute(routing, 'vpn', 'darwin');
    expect(ops[0]).toEqual({ op: 'set-split-mode', mode: 'off' });
    const after = applyRoutingOps(routing, ops, 'darwin');
    expect(after.includedApps).toEqual([]);
    expect(after.excludedApps).toEqual([]);
    expect(appRules(after, 'darwin')).toEqual([{ app: FIREFOX, route: country(RO) }]);
    for (const state of statesAlong(routing, ops).slice(1)) {
      for (const app of APPS) {
        expect(appRouteFor(state, app, 'darwin').kind).not.toBe('direct');
      }
    }
  });

  it('does nothing when the default is already the one asked for', () => {
    expect(planDefaultRoute(settings(), 'vpn', 'darwin')).toEqual([]);
    expect(planDefaultRoute(settings({ splitMode: 'include-only' }), 'direct', 'darwin')).toEqual(
      [],
    );
  });
});

describe('planAppRoute with ids that differ in case', () => {
  const STORED = 'C:\\Games\\Steam.exe';
  const ASKED = 'c:\\games\\steam.EXE';

  it('removes an entry under the exact id the daemon stored, which it compares as is', () => {
    const routing = settings({
      splitMode: 'exclude',
      excludedApps: [STORED],
      appExits: [{ app: STORED, exit: NL }],
    });
    expect(planAppRoute(routing, ASKED, VPN, 'win32')).toEqual([
      { op: 'clear-exit', app: STORED },
      { op: 'remove-excluded', app: STORED },
      { op: 'set-split-mode', mode: 'off' },
    ]);
  });

  it('takes an included app out of its list under its stored id', () => {
    const routing = settings({ splitMode: 'include-only', includedApps: [STORED] });
    expect(planAppRoute(routing, ASKED, DIRECT, 'win32')).toEqual([
      { op: 'remove-included', app: STORED },
    ]);
  });
});

describe('planDefaultRoute toward direct', () => {
  it('sends no app outside the VPN before the last call', () => {
    const routing = settings({
      splitMode: 'exclude',
      excludedApps: [STEAM],
      includedApps: [SLACK],
      appExits: [{ app: FIREFOX, exit: NL }],
    });
    const ops = planDefaultRoute(routing, 'direct', 'darwin');
    const states = statesAlong(routing, ops);
    for (const state of states.slice(1, -1)) {
      for (const app of APPS.filter((other) => other !== STEAM)) {
        expect(appRouteFor(state, app, 'darwin').kind).not.toBe('direct');
      }
    }
  });
});

describe('routingReflects, whether the daemon has applied a change', () => {
  const routing = settings({ splitMode: 'exclude', excludedApps: [STEAM] });

  it('holds once every app routes as planned, whatever the list order', () => {
    const planned = applyRoutingOps(
      routing,
      planAppRoute(routing, SLACK, DIRECT, 'darwin'),
      'darwin',
    );
    const reported = { ...planned, excludedApps: [SLACK, STEAM] };
    expect(routingReflects(reported, planned, 'darwin')).toBe(true);
  });

  it('fails while the daemon still reports the state before the change', () => {
    const planned = applyRoutingOps(
      routing,
      planAppRoute(routing, SLACK, DIRECT, 'darwin'),
      'darwin',
    );
    expect(routingReflects(routing, planned, 'darwin')).toBe(false);
  });

  it('fails while the default differs, even with no rule on either side', () => {
    expect(routingReflects(settings(), settings({ splitMode: 'include-only' }), 'darwin')).toBe(
      false,
    );
  });
});
