import { DaemonAccessRefusal } from '../../../../../../../../shared/daemon-access-refusal';
import { messages } from '../../../../../../../../shared/gettext';
import { FooterText } from '../footer-text';

export type AccessDeniedFooterProps = {
  refusal: DaemonAccessRefusal;
};

export function AccessDeniedFooter({ refusal }: AccessDeniedFooterProps) {
  return (
    <FooterText>
      {refusal === 'claimNeedsConsoleUser'
        ? // TRANSLATORS: Message in launch view when Warren was set up on this computer before it recorded an owner, and this account is not the one at the computer's own screen.
          messages.pgettext(
            'launch-view',
            'Open Warren from the account signed in at this computer’s own screen to become its owner, or ask an administrator.',
          )
        : // TRANSLATORS: Message in launch view when another account on this computer set up Warren, so the daemon refuses this one.
          messages.pgettext(
            'launch-view',
            'Only the account that set Warren up, or an administrator, can use it here.',
          )}
    </FooterText>
  );
}
