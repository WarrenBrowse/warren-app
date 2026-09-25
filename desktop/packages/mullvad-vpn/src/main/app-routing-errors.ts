import * as grpc from '@grpc/grpc-js';

// `mullvad_management_interface::APP_EXIT_LIMIT_DETAILS`. The daemon sends it
// as the binary status details, which gRPC carries in this trailer; the status
// message is prose meant for logs and may change.
const APP_EXIT_LIMIT_DETAILS = 'app_exit_limit';
const STATUS_DETAILS_KEY = 'grpc-status-details-bin';

export function isAppExitLimitError(error: unknown): boolean {
  if (typeof error !== 'object' || error === null) {
    return false;
  }
  const { code, metadata } = error as Partial<grpc.ServiceError>;
  if (code !== grpc.status.FAILED_PRECONDITION || !(metadata instanceof grpc.Metadata)) {
    return false;
  }
  return metadata
    .get(STATUS_DETAILS_KEY)
    .some((value) => Buffer.from(value).toString('utf8') === APP_EXIT_LIMIT_DETAILS);
}
