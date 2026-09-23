import * as grpc from '@grpc/grpc-js';

import { DaemonAccessRefusal } from '../shared/daemon-access-refusal';

/** Where the daemon puts the machine-readable reason of a refused call. */
const REASON_TRAILER = 'grpc-status-details-bin';

/**
 * The daemon's reason for refusing a call from this account, or `null` when
 * the error is not a refusal. Retrying cannot change the answer until the
 * ownership changes, so the app stops, says why, and asks again slowly.
 */
export function daemonAccessRefusal(error: unknown): DaemonAccessRefusal | null {
  if (
    typeof error !== 'object' ||
    error === null ||
    (error as { code?: unknown }).code !== grpc.status.PERMISSION_DENIED
  ) {
    return null;
  }
  const metadata = (error as { metadata?: unknown }).metadata;
  const reason =
    metadata instanceof grpc.Metadata ? metadata.get(REASON_TRAILER)[0]?.toString() : undefined;
  return reason === 'claim_needs_console_user' ? 'claimNeedsConsoleUser' : 'ownedByAnotherAccount';
}
