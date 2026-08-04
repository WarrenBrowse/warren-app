import { urls } from '../../../../../shared/constants';
import { messages } from '../../../../../shared/gettext';
import { ExternalLink } from '../../../ExternalLink';

// Shown under onboarding and login error messages. At this stage the
// wallet has usually never paid, and forum login is gated on ever-paid,
// so the link goes to the help page (warren.ro/aide): public triage plus
// a guest form that needs no account at all.
export function OnboardingForumHint() {
  return (
    <ExternalLink variant="labelTinySemiBold" to={urls.help}>
      <ExternalLink.Text>
        {
          // TRANSLATORS: Link shown under onboarding errors pointing to the public help page.
          messages.pgettext('warren-onboarding', 'Having trouble? Visit our help page')
        }
      </ExternalLink.Text>
      <ExternalLink.Icon
        aria-description={messages.pgettext('accessibility', 'Opens externally')}
        icon="external"
      />
    </ExternalLink>
  );
}
