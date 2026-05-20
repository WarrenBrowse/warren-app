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

import { NatPmpProto, NatPmpSettings, NatPmpStatus } from '../../src/shared/daemon-rpc-types';
import settingsActions from '../../src/renderer/redux/settings/actions';
import settingsReducer, {
  ISettingsReduxState,
} from '../../src/renderer/redux/settings/reducers';

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
  protocol: NatPmpProto.udp,
  suggestedExternalPort: 0,
  internalPort: 0,
};

describe('settings reducer - NAT-PMP slice', () => {
  it('UPDATE_NAT_PMP_SETTINGS replaces the whole struct', () => {
    const initial = makeStateWithNatPmp(OFF_DEFAULTS, undefined);
    const next = settingsReducer(
      initial,
      settingsActions.updateNatPmpSettings({
        enabled: true,
        lifetimeSecs: 3600,
        protocol: NatPmpProto.tcp,
        suggestedExternalPort: 22222,
        internalPort: 22,
      }),
    );
    expect(next.warrenNatPmp.enabled).toBe(true);
    expect(next.warrenNatPmp.protocol).toBe(NatPmpProto.tcp);
    expect(next.warrenNatPmp.internalPort).toBe(22);
  });

  it('UPDATE_NAT_PMP_STATUS sets a Mapped snapshot', () => {
    const initial = makeStateWithNatPmp(OFF_DEFAULTS, undefined);
    const next = settingsReducer(
      initial,
      settingsActions.updateNatPmpStatus({
        state: 'mapped',
        externalPort: 49152,
        lifetimeRemainingSecs: 3600,
      }),
    );
    expect(next.natPmpStatus).toEqual({
      state: 'mapped',
      externalPort: 49152,
      lifetimeRemainingSecs: 3600,
    });
  });

  it('UPDATE_NAT_PMP_STATUS overwrites a previous Mapped with Failed', () => {
    const initial = makeStateWithNatPmp(OFF_DEFAULTS, {
      state: 'mapped',
      externalPort: 49152,
      lifetimeRemainingSecs: 3600,
    });
    const next = settingsReducer(
      initial,
      settingsActions.updateNatPmpStatus({
        state: 'failed',
        errorMessage: 'server returned error: OutOfResources',
      }),
    );
    expect(next.natPmpStatus).toEqual({
      state: 'failed',
      errorMessage: 'server returned error: OutOfResources',
    });
  });

  it('UPDATE_NAT_PMP_STATUS to Disabled returns the row to the hidden state', () => {
    const initial = makeStateWithNatPmp(OFF_DEFAULTS, {
      state: 'mapped',
      externalPort: 49152,
      lifetimeRemainingSecs: 3600,
    });
    const next = settingsReducer(
      initial,
      settingsActions.updateNatPmpStatus({ state: 'disabled' }),
    );
    expect(next.natPmpStatus).toEqual({ state: 'disabled' });
  });

  it('UPDATE_NAT_PMP_SETTINGS does not touch natPmpStatus', () => {
    const liveStatus: NatPmpStatus = {
      state: 'mapped',
      externalPort: 60000,
      lifetimeRemainingSecs: 1800,
    };
    const initial = makeStateWithNatPmp(OFF_DEFAULTS, liveStatus);
    const next = settingsReducer(
      initial,
      settingsActions.updateNatPmpSettings({
        ...OFF_DEFAULTS,
        enabled: true,
        protocol: NatPmpProto.tcp,
      }),
    );
    expect(next.warrenNatPmp.enabled).toBe(true);
    expect(next.warrenNatPmp.protocol).toBe(NatPmpProto.tcp);
    expect(next.natPmpStatus).toEqual(liveStatus);
  });
});
