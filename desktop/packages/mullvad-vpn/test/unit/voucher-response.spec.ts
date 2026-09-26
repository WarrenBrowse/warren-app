import * as grpc from '@grpc/grpc-js';
import { describe, expect, it } from 'vitest';

import { voucherFailureOfStatus } from '../../src/main/voucher-response';

describe('the voucher refusal the daemon answers', () => {
  it('reads permission denied as a ban, which keeps the voucher', () => {
    expect(voucherFailureOfStatus(grpc.status.PERMISSION_DENIED)).to.deep.equal({
      type: 'banned',
    });
  });

  it('keeps the four refusals it already told apart', () => {
    expect(voucherFailureOfStatus(grpc.status.NOT_FOUND).type).to.equal('invalid');
    expect(voucherFailureOfStatus(grpc.status.RESOURCE_EXHAUSTED).type).to.equal('already_used');
    expect(voucherFailureOfStatus(grpc.status.FAILED_PRECONDITION).type).to.equal('expired');
    expect(voucherFailureOfStatus(grpc.status.UNAVAILABLE).type).to.equal('not_ready');
  });

  it('reads anything else as an error', () => {
    expect(voucherFailureOfStatus(grpc.status.INTERNAL).type).to.equal('error');
    expect(voucherFailureOfStatus(undefined).type).to.equal('error');
  });
});
