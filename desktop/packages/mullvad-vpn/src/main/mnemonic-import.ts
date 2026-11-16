import * as grpc from '@grpc/grpc-js';

import { MnemonicImportResult } from '../shared/mnemonic-import';
import { daemonAccessRefusal } from './daemon-access';

/**
 * What a failed `SetWarrenMnemonic` call stands for. The daemon answers
 * INVALID_ARGUMENT only for a phrase that fails BIP39 validation, so every
 * other failure must not be shown as a wrong phrase.
 */
export function mnemonicImportFailure(error: unknown): MnemonicImportResult {
  const refusal = daemonAccessRefusal(error);
  if (refusal !== null) {
    return { type: 'refused', refusal };
  }
  if (
    typeof error === 'object' &&
    error !== null &&
    (error as { code?: unknown }).code === grpc.status.INVALID_ARGUMENT
  ) {
    return { type: 'invalid' };
  }
  return { type: 'error' };
}
