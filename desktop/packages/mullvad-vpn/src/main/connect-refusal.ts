import { Metadata, status as grpcStatus } from '@grpc/grpc-js';

import log from '../shared/logging';

/** Where the daemon puts the machine-readable reason of a refused call. */
const REASON_TRAILER = 'grpc-status-details-bin';

/** The reason a revoked access (a ban) carries, `ACCESS_REVOKED_DETAILS`. */
const ACCESS_REVOKED = 'access_revoked';

/**
 * Why the daemon refused a connect, read from the status code it answers with
 * (`map_connect_refusal` in the daemon): UNAUTHENTICATED when no account is
 * logged in or the device is revoked, PERMISSION_DENIED with its own reason
 * while the account's access is revoked (a ban), FAILED_PRECONDITION when this
 * build stood down for another product environment. `undefined` for any other
 * failure, among them the PERMISSION_DENIED of the daemon's access gate.
 */
export type ConnectRefusal = 'not-logged-in' | 'access-revoked' | 'stood-down';

export function connectRefusalOf(error: unknown): ConnectRefusal | undefined {
  switch ((error as { code?: number } | undefined)?.code) {
    case grpcStatus.UNAUTHENTICATED:
      return 'not-logged-in';
    case grpcStatus.PERMISSION_DENIED:
      return reasonOf(error) === ACCESS_REVOKED ? 'access-revoked' : undefined;
    case grpcStatus.FAILED_PRECONDITION:
      return 'stood-down';
    default:
      return undefined;
  }
}

function reasonOf(error: unknown): string | undefined {
  const metadata = (error as { metadata?: unknown }).metadata;
  return metadata instanceof Metadata ? metadata.get(REASON_TRAILER)[0]?.toString() : undefined;
}

export interface ConnectDeps {
  connect(): Promise<void>;
  /** Drops the `connecting` state the GUI predicted when the button was pressed. */
  discardExpectedState(): void;
  /** Reads the login state from the daemon again. */
  resyncDeviceState(): Promise<void>;
}

/**
 * Issues a connect. A refused connect never reaches the tunnel, so the
 * predicted `connecting` state goes at once instead of lingering until its
 * timer. A logged-out refusal means the GUI believed an account was logged in
 * when the daemon had none (a CLI logout it missed), or that the device was
 * revoked: reading the login state again lands it on the view that fixes it.
 * A revoked access needs nothing more: the banner of the ban in force says
 * why the tunnel stays off, from the standing the GUI already holds.
 */
export async function connectHandlingRefusal(deps: ConnectDeps): Promise<void> {
  try {
    await deps.connect();
  } catch (error) {
    deps.discardExpectedState();
    const refusal = connectRefusalOf(error);
    if (refusal === 'not-logged-in') {
      log.info('Connect refused by the daemon: no account is logged in');
      await deps.resyncDeviceState();
      return;
    }
    if (refusal === 'access-revoked') {
      log.info('Connect refused by the daemon: access to the account is revoked');
      return;
    }
    throw error;
  }
}
