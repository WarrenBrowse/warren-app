import { status as grpcStatus } from '@grpc/grpc-js';

import { LogoutOutcome } from '../shared/daemon-rpc-types';

/**
 * Runs a logout and tells its refusal apart from a failure. The daemon answers
 * ABORTED when the tunnel did not come down within its bound
 * (`LogoutTunnelStillUp`): nothing was changed and a retry can succeed, so the
 * GUI says so instead of throwing it away. Any other failure is passed on.
 */
export async function logoutReportingRefusal(logout: () => Promise<void>): Promise<LogoutOutcome> {
  try {
    await logout();
    return 'logged-out';
  } catch (error) {
    if ((error as { code?: number } | undefined)?.code === grpcStatus.ABORTED) {
      return 'tunnel-still-up';
    }
    throw error;
  }
}
