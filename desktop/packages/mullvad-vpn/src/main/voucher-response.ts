import * as grpc from '@grpc/grpc-js';

import { VoucherResponse } from '../shared/daemon-rpc-types';

/**
 * The refusal a failed `SubmitVoucher` call stands for, from the status code
 * the daemon maps each voucher error to (`map_device_error`).
 */
export function voucherFailureOfStatus(code: grpc.status | undefined): VoucherResponse {
  switch (code) {
    case grpc.status.NOT_FOUND:
      return { type: 'invalid' };
    case grpc.status.RESOURCE_EXHAUSTED:
      return { type: 'already_used' };
    case grpc.status.FAILED_PRECONDITION:
      return { type: 'expired' };
    // The account is banned (warren-core doc 105 section 5.3): the voucher
    // was not consumed and stays the user's for after the ban.
    case grpc.status.PERMISSION_DENIED:
      return { type: 'banned' };
    // Also emitted on daemon-transport failures: both mean
    // "nothing definitive happened, retry later".
    case grpc.status.UNAVAILABLE:
      return { type: 'not_ready' };
    default:
      return { type: 'error' };
  }
}
