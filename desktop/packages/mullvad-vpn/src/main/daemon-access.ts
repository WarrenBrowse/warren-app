import * as grpc from '@grpc/grpc-js';

/**
 * Whether the daemon refused a call because this account may not use Warren
 * on this computer: another account set it up, and only that account or an
 * administrator may drive the tunnel or reach the wallet. Retrying cannot
 * change that answer, so the app stops and says so.
 */
export function isDaemonAccessRefusal(error: unknown): boolean {
  return (
    typeof error === 'object' &&
    error !== null &&
    (error as { code?: unknown }).code === grpc.status.PERMISSION_DENIED
  );
}
