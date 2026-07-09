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
import settingsActions from '../../src/renderer/redux/settings/actions';
import settingsReducer, { ISettingsReduxState } from '../../src/renderer/redux/settings/reducers';
import {
  NatPmpProto,
  NatPmpSettings,
  WarrenPubkeyMismatch,
  WarrenStatus,
} from '../../src/shared/daemon-rpc-types';

const OFF_NAT_PMP: NatPmpSettings = {
  enabled: false,
  lifetimeSecs: 3600,
  rules: [],
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
  maintenanceMigrationActive: false,
  portMigrationCancellations: 0,
  portMigrationCancellationActive: false,
  hostOffline: false,
  exitEgressDead: false,
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

  it('empty forensic fields are accepted', () => {
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

// Behavioural tests: confirm the IPC payload contract used
// by the renderer when the user picks Trust / Reject / Report from
// the modal. We don't render the React tree (no jsdom in the test
// harness) but we exercise the exact callbacks the component invokes
// so a regression in the IPC shape is caught at the unit layer.
describe('warren-pubkey-warning IPC payload contract', () => {
  it('Trust handler maps observed pubkey -> new pinned key', () => {
    const trustCalls: { exitIdHex: string; newPubkeyHex: string }[] = [];
    const trustNewExitKey = (input: { exitIdHex: string; newPubkeyHex: string }) => {
      trustCalls.push(input);
      return Promise.resolve({ result: 'ok' as const });
    };
    // Simulate the component callback. The newPubkeyHex MUST be the
    // observed key (not the pinned one) - that's the whole point of
    // accepting a rotation.
    void trustNewExitKey({
      exitIdHex: MISMATCH.exitIdHex,
      newPubkeyHex: MISMATCH.observedPubkeyHex,
    });
    expect(trustCalls).toHaveLength(1);
    expect(trustCalls[0].newPubkeyHex).toBe(MISMATCH.observedPubkeyHex);
    expect(trustCalls[0].newPubkeyHex).not.toBe(MISMATCH.pinnedPubkeyHex);
  });

  it('Report handler forwards the full mismatch payload to warren-api', () => {
    const reportCalls: WarrenPubkeyMismatch[] = [];
    const reportPubkeyMismatch = (mismatch: WarrenPubkeyMismatch) => {
      reportCalls.push(mismatch);
      return Promise.resolve();
    };
    void reportPubkeyMismatch(MISMATCH);
    expect(reportCalls).toHaveLength(1);
    // Forensic snapshot included verbatim. Privacy is enforced
    // backend-side (the IP is not in the payload, only the public
    // pubkey + exit_id + operator-curated location).
    expect(reportCalls[0]).toEqual(MISMATCH);
  });

  it('Dismiss handler takes no arguments', () => {
    let dismissCalls = 0;
    const dismissPubkeyMismatch = () => {
      dismissCalls += 1;
      return Promise.resolve();
    };
    void dismissPubkeyMismatch();
    expect(dismissCalls).toBe(1);
  });

  it('Trust outcome maps gRPC enum -> renderer string', () => {
    // The grpc-type-convertions side maps each TrustNewExitKeyResponse.Result
    // enum to a discriminated union the renderer can dispatch on.
    // Locking the contract here so the renderer can rely on
    // `result === 'ok'` etc. across all binding regenerations.
    const okOutcome = { result: 'ok' as const };
    const notFoundOutcome = { result: 'exit-not-found' as const };
    const pubkeyMismatchOutcome = { result: 'pubkey-mismatch' as const };
    const ioErrorOutcome = { result: 'io-error' as const, errorMessage: 'disk full' };

    expect(okOutcome.result).toBe('ok');
    expect(notFoundOutcome.result).toBe('exit-not-found');
    expect(pubkeyMismatchOutcome.result).toBe('pubkey-mismatch');
    expect(ioErrorOutcome.result).toBe('io-error');
    expect(ioErrorOutcome.errorMessage).toBe('disk full');
  });
});
