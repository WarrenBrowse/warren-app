import { messages } from '../../../../../../../../shared/gettext';
import { FooterText } from '../footer-text';

export function AccessDeniedFooter() {
  return (
    <FooterText>
      {
        // TRANSLATORS: Message in launch view when another account on this computer set up Warren, so the daemon refuses this one.
        messages.pgettext(
          'launch-view',
          'Only the account that set Warren up, or an administrator, can use it here. If you set it up yourself, open Warren from that account at the computer’s own screen.',
        )
      }
    </FooterText>
  );
}
