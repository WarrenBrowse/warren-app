import { describe, expect, it, vi } from 'vitest';

// The settings reducer module reads `window.env.platform` at top level when it
// computes its default state, so a stub `window` exists before the import.
vi.hoisted(() => {
  (globalThis as { window?: unknown }).window = {
    env: { platform: 'darwin', development: false },
  };
});

import settingsActions from '../../src/renderer/redux/settings/actions';
import settingsReducer from '../../src/renderer/redux/settings/reducers';
import type { AppRoutingSettings } from '../../src/shared/daemon-rpc-types';

const initial = settingsReducer(undefined, { type: '@@INIT' } as never);

describe('settings reducer, app routing slice', () => {
  it('starts with nothing routed', () => {
    expect(initial.appRouting).toEqual({
      splitMode: 'off',
      excludedApps: [],
      includedApps: [],
      appExitsEnabled: false,
      appExits: [],
    });
    expect(initial.appRouteStatus).toEqual([]);
    expect(initial.appRoutingApplications).toEqual([]);
  });

  it('stores the app routing settings the daemon sends', () => {
    const routing: AppRoutingSettings = {
      splitMode: 'include-only',
      excludedApps: [],
      includedApps: ['/usr/bin/firefox'],
      appExitsEnabled: true,
      appExits: [{ app: '/usr/bin/slack', exit: { country: 'se' } }],
    };

    const next = settingsReducer(initial, settingsActions.updateAppRouting(routing));

    expect(next.appRouting).toEqual(routing);
  });

  it('replaces the route statuses with the latest push', () => {
    const first = settingsReducer(
      initial,
      settingsActions.setAppRouteStatus([
        { exit: { country: 'se' }, state: 'connecting', apps: ['/usr/bin/slack'] },
      ]),
    );

    const next = settingsReducer(
      first,
      settingsActions.setAppRouteStatus([
        { exit: { country: 'se' }, state: 'connected', apps: ['/usr/bin/slack'] },
      ]),
    );

    expect(next.appRouteStatus.map((status) => status.state)).toEqual(['connected']);
  });

  it('stores the name and icon of the routed apps', () => {
    const next = settingsReducer(
      initial,
      settingsActions.setAppRoutingApplications([
        { absolutepath: '/usr/bin/slack', name: 'Slack', deletable: false },
      ]),
    );

    expect(next.appRoutingApplications.map((application) => application.name)).toEqual(['Slack']);
  });
});
