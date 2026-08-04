import { describe, expect, it, vi } from 'vitest';

// `vi.hoisted` runs before any import is evaluated. The settings
// reducer module reads `window.env.platform` at top level when
// computing its default state, so we install a stub `window` here
// before the import below executes.
vi.hoisted(() => {
  (globalThis as { window?: unknown }).window = {
    env: { platform: 'linux', development: false },
  };
});

import settingsActions from '../../src/renderer/redux/settings/actions';
import settingsReducer, { ISettingsReduxState } from '../../src/renderer/redux/settings/reducers';
import { NatPmpProto, NatPmpSettings, NatPmpStatus } from '../../src/shared/daemon-rpc-types';

// Minimal redux state focused on the NAT-PMP slice; the reducer treats
// the other fields as pass-through so we can fill them with arbitrary
// shapes (`as unknown as ISettingsReduxState`) without needing the
// renderer environment (`window.env.platform`, ...).
function makeStateWithNatPmp(
  natPmp: NatPmpSettings,
  status: NatPmpStatus | undefined,
): ISettingsReduxState {
  return {
    warrenNatPmp: natPmp,
    natPmpStatus: status,
  } as unknown as ISettingsReduxState;
}

const OFF_DEFAULTS: NatPmpSettings = {
  enabled: false,
  lifetimeSecs: 3600,
  rules: [],
  protocol: NatPmpProto.udp,
  suggestedExternalPort: 0,
  internalPort: 0,
};

describe('settings reducer - NAT-PMP slice', () => {
  it('UPDATE_NAT_PMP_SETTINGS replaces the whole struct (multi-port rules)', () => {
    const initial = makeStateWithNatPmp(OFF_DEFAULTS, undefined);
    const next = settingsReducer(
      initial,
      settingsActions.updateNatPmpSettings({
        enabled: true,
        lifetimeSecs: 3600,
        rules: [
          { protocol: NatPmpProto.tcp, suggestedExternalPort: 51820, internalPort: 51820 },
          { protocol: NatPmpProto.udp, suggestedExternalPort: 8080, internalPort: 8080 },
        ],
        protocol: NatPmpProto.udp,
        suggestedExternalPort: 0,
        internalPort: 0,
      }),
    );
    expect(next.warrenNatPmp.enabled).toBe(true);
    expect(next.warrenNatPmp.rules).toHaveLength(2);
    expect(next.warrenNatPmp.rules[0].internalPort).toBe(51820);
    expect(next.warrenNatPmp.rules[1].protocol).toBe(NatPmpProto.udp);
  });

  it('UPDATE_NAT_PMP_STATUS sets a multi-mapping snapshot', () => {
    const initial = makeStateWithNatPmp(OFF_DEFAULTS, undefined);
    const next = settingsReducer(
      initial,
      settingsActions.updateNatPmpStatus({
        mappings: [
          {
            internalPort: 49152,
            protocol: NatPmpProto.udp,
            status: {
              state: 'mapped',
              externalPort: 49152,
              lifetimeGrantedSecs: 3600,
              attemptsRemaining: 4,
              windowResetSecs: 0,
            },
          },
        ],
      }),
    );
    expect(next.natPmpStatus?.mappings).toHaveLength(1);
    expect(next.natPmpStatus?.mappings[0].status).toEqual({
      state: 'mapped',
      externalPort: 49152,
      lifetimeGrantedSecs: 3600,
      attemptsRemaining: 4,
      windowResetSecs: 0,
    });
  });

  it('UPDATE_NAT_PMP_STATUS sets a per-rule RateLimited snapshot with retry-after', () => {
    const initial = makeStateWithNatPmp(OFF_DEFAULTS, undefined);
    const next = settingsReducer(
      initial,
      settingsActions.updateNatPmpStatus({
        mappings: [
          {
            internalPort: 22,
            protocol: NatPmpProto.tcp,
            status: { state: 'rate-limited', retryAfterSecs: 47 },
          },
        ],
      }),
    );
    expect(next.natPmpStatus?.mappings[0].status).toEqual({
      state: 'rate-limited',
      retryAfterSecs: 47,
    });
  });

  it('UPDATE_NAT_PMP_STATUS overwrites a previous Mapped with Failed', () => {
    const initial = makeStateWithNatPmp(OFF_DEFAULTS, {
      mappings: [
        {
          internalPort: 49152,
          protocol: NatPmpProto.udp,
          status: {
            state: 'mapped',
            externalPort: 49152,
            lifetimeGrantedSecs: 3600,
            attemptsRemaining: 4,
            windowResetSecs: 0,
          },
        },
      ],
    });
    const next = settingsReducer(
      initial,
      settingsActions.updateNatPmpStatus({
        mappings: [
          {
            internalPort: 49152,
            protocol: NatPmpProto.udp,
            status: {
              state: 'failed',
              errorMessage: 'server returned error: OutOfResources',
              errorReason: 'out-of-resources',
            },
          },
        ],
      }),
    );
    expect(next.natPmpStatus?.mappings[0].status).toEqual({
      state: 'failed',
      errorMessage: 'server returned error: OutOfResources',
      errorReason: 'out-of-resources',
    });
  });

  it('UPDATE_NAT_PMP_STATUS with no mappings clears the list', () => {
    const initial = makeStateWithNatPmp(OFF_DEFAULTS, {
      mappings: [
        {
          internalPort: 49152,
          protocol: NatPmpProto.udp,
          status: {
            state: 'mapped',
            externalPort: 49152,
            lifetimeGrantedSecs: 3600,
            attemptsRemaining: 4,
            windowResetSecs: 0,
          },
        },
      ],
    });
    const next = settingsReducer(initial, settingsActions.updateNatPmpStatus({ mappings: [] }));
    expect(next.natPmpStatus?.mappings).toEqual([]);
  });

  it('UPDATE_NAT_PMP_SETTINGS does not touch natPmpStatus', () => {
    const liveStatus: NatPmpStatus = {
      mappings: [
        {
          internalPort: 60000,
          protocol: NatPmpProto.udp,
          status: {
            state: 'mapped',
            externalPort: 60000,
            lifetimeGrantedSecs: 1800,
            attemptsRemaining: 5,
            windowResetSecs: 0,
          },
        },
      ],
    };
    const initial = makeStateWithNatPmp(OFF_DEFAULTS, liveStatus);
    const next = settingsReducer(
      initial,
      settingsActions.updateNatPmpSettings({
        ...OFF_DEFAULTS,
        enabled: true,
        rules: [{ protocol: NatPmpProto.tcp, suggestedExternalPort: 51820, internalPort: 51820 }],
      }),
    );
    expect(next.warrenNatPmp.enabled).toBe(true);
    expect(next.warrenNatPmp.rules).toHaveLength(1);
    expect(next.natPmpStatus).toEqual(liveStatus);
  });
});
