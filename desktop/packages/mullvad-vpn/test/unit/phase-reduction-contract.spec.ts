import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { getConnectionPhase } from '../../src/renderer/lib/connection-phase';
import { TunnelState } from '../../src/shared/daemon-rpc-types';

// Pins the renderer's `getConnectionPhase` reducer to the shared phase-reduction
// contract by replaying its golden fixture. The canonical reducer is
// `warren_contract::phase::reduce_phase`; the same fixture is already replayed
// in Rust (`warren-contract/tests/phase_vectors.rs`) and in the TS SDK
// (`@warrenbrowse/sdk-core` `packages/core/test/phase.test.ts`).
//
// We deliberately do NOT add `@warrenbrowse/sdk-core` as a renderer dependency
// to call `reduce_phase` directly: that pulls a heavy transitive tree into the
// Electron bundle for one pure function. Per doc-94 A8 the compliant fallback is
// to replay the shared fixture against the local reducer, which pins the same
// semantics without the dependency.
//
// We read the fixture from the checked-out sibling `../warren-contract` at test
// time rather than vendoring a copy: a vendored copy would re-duplicate the
// vector (the exact refragmentation this work removes). The sibling checkout is
// guaranteed by warren-app's CLAUDE.md.
const FIXTURE_URL = new URL(
  '../../../../../../warren-contract/tests/fixtures/phase-reduction.json',
  import.meta.url,
);

// Shared-contract tunnel states with no `TunnelState.state` equivalent in the
// app: the daemon never surfaces them as such (a drain reads as connected, a
// redial as connecting), so the local reducer has no input for them. Their
// fixture rows are skipped, not replayed.
const SHARED_ONLY_STATES = ['draining', 'reconnecting'];

interface FixtureStatus {
  state: string;
  lockedDown?: boolean;
  blockingError?: boolean;
}

interface FixtureRow {
  status: FixtureStatus;
  egress: { hostOffline: boolean; exitEgressDead: boolean };
  phase: string;
}

// Maps a fixture status to the app `TunnelState` the reducer consumes, or
// `undefined` for a shared-only state. The reducer reads only `state`,
// `lockedDown` and `details.blockingError`, so the other required fields are
// filled minimally (matching the existing connection-scenery.spec fixtures).
function toTunnelState(status: FixtureStatus): TunnelState | undefined {
  switch (status.state) {
    case 'connected':
      return { state: 'connected', details: {} } as unknown as TunnelState;
    case 'connecting':
      return { state: 'connecting' } as TunnelState;
    case 'disconnecting':
      return { state: 'disconnecting', details: 'nothing' } as TunnelState;
    case 'disconnected':
      return { state: 'disconnected', lockedDown: status.lockedDown ?? false } as TunnelState;
    case 'error':
      return {
        state: 'error',
        details: status.blockingError ? { blockingError: {} } : {},
      } as unknown as TunnelState;
    default:
      return undefined;
  }
}

const rows: FixtureRow[] = JSON.parse(readFileSync(fileURLToPath(FIXTURE_URL), 'utf-8'));
const appRows = rows.filter((row) => toTunnelState(row.status) !== undefined);
const skippedRows = rows.filter((row) => toTunnelState(row.status) === undefined);

describe('getConnectionPhase conforms to the shared phase-reduction fixture', () => {
  it('the fixture loaded and has app-representable rows to replay', () => {
    expect(rows.length).toBeGreaterThan(0);
    expect(appRows.length).toBeGreaterThan(0);
  });

  it.each(appRows)('row $status.state (phase $phase) matches the local reducer', (row) => {
    const tunnelState = toTunnelState(row.status)!;
    expect(getConnectionPhase(tunnelState, row.egress.hostOffline, row.egress.exitEgressDead)).toBe(
      row.phase,
    );
  });

  it('every skipped row is a documented shared-only state', () => {
    for (const row of skippedRows) {
      expect(SHARED_ONLY_STATES).toContain(row.status.state);
    }
  });
});
