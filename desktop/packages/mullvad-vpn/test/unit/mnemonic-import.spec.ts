import * as grpc from '@grpc/grpc-js';
import { expect, it } from 'vitest';

import { mnemonicImportFailure } from '../../src/main/mnemonic-import';

function grpcError(code: grpc.status, details: string, reason?: string): Error {
  const metadata = new grpc.Metadata();
  if (reason !== undefined) {
    metadata.set('grpc-status-details-bin', Buffer.from(reason));
  }
  return Object.assign(new Error(`${code} ${grpc.status[code]}: ${details}`), {
    code,
    details,
    metadata,
  });
}

// The daemon answers INVALID_ARGUMENT only when the phrase fails BIP39
// validation (`set_warren_mnemonic` in its management interface).
it('reports a phrase the daemon finds invalid as invalid', () => {
  const error = grpcError(grpc.status.INVALID_ARGUMENT, 'invalid BIP39 mnemonic');

  expect(mnemonicImportFailure(error)).toEqual({ type: 'invalid' });
});

// Forum topic 246: a second account on a Linux machine was told its phrase
// was invalid when the daemon had refused the account itself.
it('reports a refused account as refused, with the reason', () => {
  const error = grpcError(
    grpc.status.PERMISSION_DENIED,
    'Warren is set up by another account on this computer',
    'owned_by_another_account',
  );

  expect(mnemonicImportFailure(error)).toEqual({
    type: 'refused',
    refusal: 'ownedByAnotherAccount',
  });
});

// A valid phrase whose login under the new identity failed, a storage error,
// or a daemon that went away: none of them says anything about the words.
it('reports any other failure as an error, never as an invalid phrase', () => {
  const error = grpcError(grpc.status.INTERNAL, 'login failed');

  expect(mnemonicImportFailure(error)).toEqual({ type: 'error' });
});
