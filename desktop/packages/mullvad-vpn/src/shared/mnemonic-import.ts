import { DaemonAccessRefusal } from './daemon-access-refusal';

/**
 * How a recovery phrase import ended, as far as the app words it:
 * - `invalid`: the words fail BIP39 validation;
 * - `refused`: the daemon refuses this account, whatever the words;
 * - `error`: anything else (the login under the new identity, the storage,
 *   the daemon going away), which says nothing about the words.
 */
export type MnemonicImportResult =
  | { type: 'ok' }
  | { type: 'invalid' }
  | { type: 'refused'; refusal: DaemonAccessRefusal }
  | { type: 'error' };
