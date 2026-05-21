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

import { truncatePubkeyHex } from '../../src/renderer/features/warren-pubkey-warning/lib/truncate-pubkey';
import {
  NatPmpProto,
  NatPmpSettings,
  WarrenPubkeyMismatch,
  WarrenStatus,
} from '../../src/shared/daemon-rpc-types';
import settingsActions from '../../src/renderer/redux/settings/actions';
import settingsReducer, {
  ISettingsReduxState,
} from '../../src/renderer/redux/settings/reducers';

const OFF_NAT_PMP: NatPmpSettings = {
  enabled: false,
  lifetimeSecs: 3600,
  protocol: NatPmpProto.udp,
  suggestedExternalPort: 0,
  internalPort: 0,
};

function makeState(warrenStatus: WarrenStatus | undefined): ISettingsReduxState {
  return {
    warrenNatPmp: OFF_NAT_PMP,
    warrenStatus,
  } as unknown as ISettingsReduxState;
}

const BASE_STATUS: WarrenStatus = {
  reconnectCount: 2,
  lastReconnectAgeMs: 1000,
  obfuscationActive: true,
  failoverCount: 0,
  lastFailoverAgeMs: null,
  pubkeyMismatchPending: null,
};

const MISMATCH: WarrenPubkeyMismatch = {
  exitIdHex: 'aa'.repeat(16),
  pinnedPubkeyHex: 'bb'.repeat(32),
  observedPubkeyHex: 'cc'.repeat(32),
  countryCode: 'fr',
  city: 'Paris',
};

describe('truncatePubkeyHex', () => {
  it('returns the input untouched when shorter than 2 * chars', () => {
    expect(truncatePubkeyHex('aabb', 4)).toBe('aabb');
    expect(truncatePubkeyHex('aabbcc', 4)).toBe('aabbcc');
  });

  it('truncates the middle of long fingerprints, keeping head + tail', () => {
    const long = 'a'.repeat(64);
    const truncated = truncatePubkeyHex(long, 8);
    expect(truncated).toMatch(/^a{8}\.\.\.a{8}$/);
    expect(truncated.length).toBeLessThan(long.length);
  });

  it('uses the custom char count argument', () => {
    const hex = '0123456789abcdef0123456789abcdef';
    expect(truncatePubkeyHex(hex, 4)).toBe('0123...cdef');
  });
});

describe('settings reducer - pubkeyMismatchPending slice', () => {
  it('UPDATE_WARREN_STATUS sets pubkeyMismatchPending when daemon pushes a mismatch', () => {
    const initial = makeState(undefined);
    const next = settingsReducer(
      initial,
      settingsActions.updateWarrenStatus({
        ...BASE_STATUS,
        pubkeyMismatchPending: MISMATCH,
      }),
    );
    expect(next.warrenStatus?.pubkeyMismatchPending).toEqual(MISMATCH);
  });

  it('UPDATE_WARREN_STATUS clears pubkeyMismatchPending on next push (steady state)', () => {
    const initial = makeState({ ...BASE_STATUS, pubkeyMismatchPending: MISMATCH });
    const next = settingsReducer(
      initial,
      settingsActions.updateWarrenStatus({ ...BASE_STATUS, pubkeyMismatchPending: null }),
    );
    expect(next.warrenStatus?.pubkeyMismatchPending).toBeNull();
  });

  it('preserves other fields when only pubkeyMismatchPending mutates', () => {
    const initial = makeState({
      ...BASE_STATUS,
      reconnectCount: 7,
      pubkeyMismatchPending: null,
    });
    const next = settingsReducer(
      initial,
      settingsActions.updateWarrenStatus({
        ...BASE_STATUS,
        reconnectCount: 7,
        pubkeyMismatchPending: MISMATCH,
      }),
    );
    expect(next.warrenStatus?.reconnectCount).toBe(7);
    expect(next.warrenStatus?.pubkeyMismatchPending).toEqual(MISMATCH);
  });

  it('payload validates hex lengths and forensic shape', () => {
    // 16 bytes -> 32 hex chars
    expect(MISMATCH.exitIdHex).toHaveLength(32);
    // 32 bytes -> 64 hex chars
    expect(MISMATCH.pinnedPubkeyHex).toHaveLength(64);
    expect(MISMATCH.observedPubkeyHex).toHaveLength(64);
    expect(MISMATCH.countryCode).toHaveLength(2);
  });

  it('empty forensic fields are accepted (pre-H.6 pin)', () => {
    const initial = makeState(undefined);
    const stripped: WarrenPubkeyMismatch = {
      ...MISMATCH,
      countryCode: '',
      city: '',
    };
    const next = settingsReducer(
      initial,
      settingsActions.updateWarrenStatus({
        ...BASE_STATUS,
        pubkeyMismatchPending: stripped,
      }),
    );
    expect(next.warrenStatus?.pubkeyMismatchPending?.countryCode).toBe('');
    expect(next.warrenStatus?.pubkeyMismatchPending?.city).toBe('');
  });
});
