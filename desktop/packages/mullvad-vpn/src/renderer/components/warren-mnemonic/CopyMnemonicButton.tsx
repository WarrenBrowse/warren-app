import { useCallback } from 'react';

import { messages } from '../../../shared/gettext';
import log from '../../../shared/logging';
import { useScheduler } from '../../../shared/scheduler';
import { Button } from '../../lib/components';
import { useBoolean } from '../../lib/utility-hooks';

const COPIED_FEEDBACK_DURATION = 2000;
// Auto-clear the clipboard a minute after copying so the seed phrase does not
// linger indefinitely (and is less likely to sync to other devices via OS
// clipboard history). We only clear if our value is still on the clipboard.
const CLIPBOARD_CLEAR_DURATION = 60000;

export type CopyMnemonicButtonProps = {
  mnemonic: string;
  'data-testid'?: string;
};

// Copies the 12/24-word mnemonic to the clipboard and shows a transient
// "Copied" confirmation (checkmark) for a couple of seconds. Shared
// between the onboarding wallet step (`OnboardingWalletView`) and the
// settings backup view (`KeysView`) so both screens expose an identical,
// app-consistent copy affordance. Using a `Button` guarantees the label
// renders in white (`Button.Text`) instead of relying on inherited text
// color, which previously rendered black-on-dark.
export function CopyMnemonicButton({ mnemonic, ...rest }: CopyMnemonicButtonProps) {
  const [copied, setCopied, resetCopied] = useBoolean(false);
  const scheduler = useScheduler();
  const clearScheduler = useScheduler();

  const onCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(mnemonic);
      setCopied();
      scheduler.schedule(resetCopied, COPIED_FEEDBACK_DURATION);
      // Best-effort auto-clear in the main process. The renderer cannot read
      // the clipboard back (the permission handler only grants
      // `clipboard-sanitized-write`), so the compare-and-clear has to run in
      // main, where it only wipes the clipboard if it still holds our mnemonic.
      clearScheduler.schedule(() => {
        window.ipc.app.clearMnemonicFromClipboard(mnemonic).catch((e) => {
          const err = e as Error;
          log.error(`Failed to clear mnemonic from clipboard: ${err.message}`);
        });
      }, CLIPBOARD_CLEAR_DURATION);
    } catch (e) {
      const err = e as Error;
      log.error(`Failed to copy mnemonic to clipboard: ${err.message}`);
    }
  }, [mnemonic, scheduler, clearScheduler, setCopied, resetCopied]);

  return (
    <Button variant="primary" onClick={onCopy} data-testid={rest['data-testid']}>
      <Button.Icon icon={copied ? 'checkmark' : 'copy'} />
      <Button.Text>
        {copied
          ? messages.pgettext('warren-mnemonic', 'Copied to clipboard')
          : messages.pgettext('warren-mnemonic', 'Copy to clipboard')}
      </Button.Text>
    </Button>
  );
}
