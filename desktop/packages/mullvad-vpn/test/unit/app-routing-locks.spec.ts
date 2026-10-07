import { describe, expect, it } from 'vitest';

import {
  appLockSupported,
  applyRoutingOps,
  type AppRoute,
  appRouteFor,
  appRules,
  effectiveIncludedApps,
  isAppLocked,
  planAppLock,
  planAppRoute,
  planDefaultRoute,
  type RoutingOp,
  routingReflects,
  ruleLine,
} from '../../src/shared/app-routing';
import type { AppRoutingSettings, ExitChoice } from '../../src/shared/daemon-rpc-types';

const FIREFOX = 'C:\\Program Files\\Mozilla Firefox\\firefox.exe';
const SLACK = 'C:\\Program Files\\Slack\\slack.exe';
const STEAM = 'C:\\Games\\Steam\\steam.exe';
const SIGNAL = 'C:\\Program Files\\Signal\\Signal.exe';
const APPS = [FIREFOX, SLACK, STEAM, SIGNAL];

const NL: ExitChoice = { country: 'nl' };

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
    lockedApps: [],
    ...overrides,
  };
}

function statesAlong(routing: AppRoutingSettings, ops: RoutingOp[]): AppRoutingSettings[] {
  const states = [routing];
  for (const op of ops) {
    states.push(applyRoutingOps(states[states.length - 1], [op], 'win32'));
  }
  return states;
}

// A lock change never moves the app or any other: the lock decides only what
// happens while the VPN does not carry the app.
function expectCleanLock(routing: AppRoutingSettings, app: string, locked: boolean) {
  const states = statesAlong(routing, planAppLock(routing, app, locked, 'win32'));
  for (const state of states) {
    for (const any of APPS) {
      expect(appRouteFor(state, any, 'win32')).toEqual(appRouteFor(routing, any, 'win32'));
    }
  }
  const after = states[states.length - 1];
  expect(isAppLocked(after, app, 'win32')).toBe(locked);
  return after;
}

describe('a locked app in the list', () => {
  it('is a rule even when it takes the default route', () => {
    const routing = settings({ lockedApps: [FIREFOX] });

    expect(appRules(routing, 'win32')).toEqual([{ app: FIREFOX, route: VPN, locked: true }]);
  });

  it('is never outside the VPN, whatever list it is also in', () => {
    const routing = settings({
      splitMode: 'exclude',
      excludedApps: [FIREFOX, STEAM],
      lockedApps: [FIREFOX],
    });

    expect(appRouteFor(routing, FIREFOX, 'win32')).toEqual(VPN);
    expect(appRules(routing, 'win32')).toEqual([
      { app: FIREFOX, route: VPN, locked: true },
      { app: STEAM, route: DIRECT },
    ]);
  });

  it('is on the VPN when the other apps go outside it', () => {
    const routing = settings({ splitMode: 'include-only', lockedApps: [SIGNAL] });

    expect(appRouteFor(routing, SIGNAL, 'win32')).toEqual(VPN);
    expect(effectiveIncludedApps(routing, 'win32')).toEqual([SIGNAL]);
  });

  it('keeps its country', () => {
    const routing = settings({
      appExits: [{ app: FIREFOX, exit: NL }],
      lockedApps: [FIREFOX],
    });

    expect(appRules(routing, 'win32')).toEqual([
      { app: FIREFOX, route: country(NL), locked: true },
    ]);
  });

  it('matches its id the way the platform compares paths', () => {
    const routing = settings({ lockedApps: ['c:\\program files\\mozilla firefox\\FIREFOX.EXE'] });

    expect(isAppLocked(routing, FIREFOX, 'win32')).toBe(true);
  });

  it('is ignored on Linux, where the daemon keeps no list', () => {
    const routing = settings({ lockedApps: ['/usr/bin/firefox'] });

    expect(isAppLocked(routing, '/usr/bin/firefox', 'linux')).toBe(false);
    expect(appRules(routing, 'linux')).toEqual([]);
  });
});

describe('planAppLock', () => {
  it('locks an app on the VPN in one call', () => {
    expect(planAppLock(settings(), FIREFOX, true, 'win32')).toEqual([{ op: 'lock', app: FIREFOX }]);
    expectCleanLock(settings(), FIREFOX, true);
  });

  it('locks an app with a country without moving it', () => {
    expectCleanLock(settings({ appExits: [{ app: FIREFOX, exit: NL }] }), FIREFOX, true);
  });

  it('refuses to lock an app outside the VPN', () => {
    const bypassing = settings({ splitMode: 'exclude', excludedApps: [STEAM] });

    expect(() => planAppLock(bypassing, STEAM, true, 'win32')).toThrow();
    expect(() =>
      planAppLock(settings({ splitMode: 'include-only' }), STEAM, true, 'win32'),
    ).toThrow();
  });

  it('keeps on the VPN an app only the lock put there when the other apps go outside it', () => {
    const routing = settings({ splitMode: 'include-only', lockedApps: [SIGNAL] });

    const after = expectCleanLock(routing, SIGNAL, false);

    expect(after.includedApps).toEqual([SIGNAL]);
  });

  it('drops an exclusion the lock was overriding, so unlocking does not send the app outside', () => {
    const routing = settings({
      splitMode: 'exclude',
      excludedApps: [FIREFOX],
      lockedApps: [FIREFOX],
    });

    const after = expectCleanLock(routing, FIREFOX, false);

    expect(after.excludedApps).toEqual([]);
    expect(after.splitMode).toBe('off');
  });

  it('removes the lock under the id the daemon stored', () => {
    const stored = 'c:\\program files\\mozilla firefox\\FIREFOX.EXE';

    expect(planAppLock(settings({ lockedApps: [stored] }), FIREFOX, false, 'win32')).toEqual([
      { op: 'unlock', app: stored },
    ]);
  });

  it('does nothing when the app is already as asked', () => {
    expect(planAppLock(settings(), FIREFOX, false, 'win32')).toEqual([]);
    expect(planAppLock(settings({ lockedApps: [FIREFOX] }), FIREFOX, true, 'win32')).toEqual([]);
  });

  it('is refused on Linux, where a lock is a launch', () => {
    expect(() => planAppLock(settings(), '/usr/bin/firefox', true, 'linux')).toThrow();
  });
});

describe('a route change of a locked app', () => {
  it('lifts the lock first when the app goes outside the VPN, and never passes through a third route', () => {
    const routing = settings({
      appExits: [{ app: FIREFOX, exit: NL }],
      lockedApps: [FIREFOX],
    });
    const ops = planAppRoute(routing, FIREFOX, DIRECT, 'win32');

    expect(ops[0]).toEqual({ op: 'unlock', app: FIREFOX });
    const states = statesAlong(routing, ops);
    for (const state of states) {
      expect([country(NL), DIRECT]).toContainEqual(appRouteFor(state, FIREFOX, 'win32'));
    }
    expect(appRouteFor(states[states.length - 1], FIREFOX, 'win32')).toEqual(DIRECT);
  });

  it('lifts the lock of an app sent outside the VPN when that is the default', () => {
    const routing = settings({
      splitMode: 'include-only',
      includedApps: [SIGNAL],
      lockedApps: [SIGNAL],
    });

    const after = applyRoutingOps(routing, planAppRoute(routing, SIGNAL, DIRECT, 'win32'), 'win32');

    expect(appRouteFor(after, SIGNAL, 'win32')).toEqual(DIRECT);
    expect(after.lockedApps).toEqual([]);
  });

  it('keeps the lock when the app moves to a country or back', () => {
    const routing = settings({ lockedApps: [FIREFOX] });

    const ops = planAppRoute(routing, FIREFOX, country(NL), 'win32');
    const after = applyRoutingOps(routing, ops, 'win32');

    expect(ops.some((op) => op.op === 'unlock')).toBe(false);
    expect(isAppLocked(after, FIREFOX, 'win32')).toBe(true);
  });
});

describe('the locks across a change of the default', () => {
  it('keeps every locked app on the VPN at each step toward direct', () => {
    const routing = settings({ lockedApps: [FIREFOX], excludedApps: [], splitMode: 'off' });
    const ops = planDefaultRoute(routing, 'direct', 'win32');

    for (const state of statesAlong(routing, ops)) {
      expect(appRouteFor(state, FIREFOX, 'win32')).toEqual(VPN);
    }
    expect(applyRoutingOps(routing, ops, 'win32').lockedApps).toEqual([FIREFOX]);
  });
});

describe('locks in the daemon model', () => {
  it('locking drops the exclusion, as the daemon does', () => {
    const routing = settings({ splitMode: 'exclude', excludedApps: [FIREFOX, STEAM] });

    const after = applyRoutingOps(routing, [{ op: 'lock', app: FIREFOX }], 'win32');

    expect(after.excludedApps).toEqual([STEAM]);
    expect(after.lockedApps).toEqual([FIREFOX]);
  });

  it('a lock not yet reported is a change not yet applied', () => {
    const planned = settings({ lockedApps: [FIREFOX] });

    expect(routingReflects(settings(), planned, 'win32')).toBe(false);
    expect(routingReflects(planned, planned, 'win32')).toBe(true);
  });
});

describe('ruleLine, the line under a rule', () => {
  const routing = settings({
    appExits: [{ app: SLACK, exit: NL }],
    lockedApps: [FIREFOX, SLACK],
  });

  it('says a locked app is blocked while the VPN is not connected, whatever its route', () => {
    expect(ruleLine(routing, [], FIREFOX, 'win32', false)).toEqual({ kind: 'blocked' });
    expect(ruleLine(routing, [], SLACK, 'win32', false)).toEqual({ kind: 'blocked' });
  });

  it('says nothing for a locked app on the main connection once the VPN is up', () => {
    expect(ruleLine(routing, [], FIREFOX, 'win32', true)).toBeUndefined();
  });

  it('gives a locked app with a country the state of its route once the VPN is up', () => {
    expect(ruleLine(routing, [], SLACK, 'win32', true)).toEqual({ kind: 'waiting' });
  });
});

describe('appLockSupported', () => {
  it('holds a lock on Windows only for now', () => {
    expect(appLockSupported('win32')).toBe(true);
    expect(appLockSupported('darwin')).toBe(false);
    expect(appLockSupported('linux')).toBe(false);
  });
});
