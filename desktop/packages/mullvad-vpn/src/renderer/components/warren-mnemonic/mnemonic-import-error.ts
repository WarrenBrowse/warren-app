import { messages } from '../../../shared/gettext';
import { MnemonicImportResult } from '../../../shared/mnemonic-import';

/** The sentence a failed recovery phrase import shows, `null` on success. */
export function mnemonicImportError(result: MnemonicImportResult): string | null {
  switch (result.type) {
    case 'ok':
      return null;
    case 'invalid':
      return messages.pgettext(
        'login-view',
        'Invalid recovery phrase. Check the words and their order, then try again.',
      );
    // The launch view's own sentences, so the two screens never word one
    // refusal differently.
    case 'refused':
      if (result.refusal === 'claimNeedsConsoleUser') {
        return messages.pgettext(
          'launch-view',
          'Open Warren from the account signed in at this computer’s own screen to become its owner, or ask an administrator.',
        );
      }
      return messages.pgettext(
        'launch-view',
        'Only the account that set Warren up, or an administrator, can use it here.',
      );
    case 'error':
      // TRANSLATORS: Error when a recovery phrase could not be restored for a reason that has nothing to do with its words, such as no connection to Warren.
      return messages.pgettext(
        'login-view',
        'Warren could not restore your account right now. Check your internet connection and try again.',
      );
  }
}
