import { status as grpcStatus } from '@grpc/grpc-js';

import log from '../shared/logging';

/**
 * Why the daemon refused a connect, read from the status code it answers with
 * (`map_connect_refusal` in the daemon): UNAUTHENTICATED when no account is
 * logged in, FAILED_PRECONDITION when this build stood down for another
 * product environment. `undefined` for any other failure.
 */
export type ConnectRefusal = 'not-logged-in' | 'stood-down';

export function connectRefusalOf(error: unknown): ConnectRefusal | undefined {
  switch ((error as { code?: number } | undefined)?.code) {
    case grpcStatus.UNAUTHENTICATED:
      return 'not-logged-in';
    case grpcStatus.FAILED_PRECONDITION:
      return 'stood-down';
    default:
      return undefined;
  }
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
 * when the daemon had none (a CLI logout it missed): reading the login state
 * again lands it on the login view, which is the whole answer to the user.
 */
export async function connectHandlingRefusal(deps: ConnectDeps): Promise<void> {
  try {
    await deps.connect();
  } catch (error) {
    deps.discardExpectedState();
    if (connectRefusalOf(error) === 'not-logged-in') {
      log.info('Connect refused by the daemon: no account is logged in');
      await deps.resyncDeviceState();
      return;
    }
    throw error;
  }
}
