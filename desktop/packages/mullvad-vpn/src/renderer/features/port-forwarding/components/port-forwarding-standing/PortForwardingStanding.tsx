import { strikeContestHint } from '../../../../../shared/account-standing';
import { urls } from '../../../../../shared/constants';
import { messages } from '../../../../../shared/gettext';
import { ExternalLink } from '../../../../components/ExternalLink';
import { Text } from '../../../../lib/components';
import { FlexColumn } from '../../../../lib/components/flex-column';
import { useSelector } from '../../../../redux/store';
import { standingSection } from '../../standing';

/**
 * The wallet's port-forwarding warnings and any revocation, each warning with
 * the case reference a contest quotes. Renders nothing in good standing.
 *
 * Every live warning is listed here, including those the banner was put away
 * for: this is where the reader finds the reference when writing to contest.
 */
export function PortForwardingStanding() {
  const standing = useSelector((state) => state.settings.warrenStatus?.accountStanding ?? null);
  const locale = useSelector((state) => state.userInterface.locale);
  const section = standingSection(standing, locale, Date.now());

  if (section === undefined) {
    return null;
  }

  return (
    <FlexColumn gap="small">
      <Text variant="labelTinySemiBold" color="white">
        {
          // TRANSLATORS: Title of the list of abuse warnings on the account.
          messages.pgettext('port-forwarding-view', 'Warnings')
        }
      </Text>
      {section.ban !== undefined ? (
        <Text variant="labelTiny" color="red">
          {section.ban}
        </Text>
      ) : null}
      {section.warnings.map((warning) => (
        <FlexColumn key={warning.reference} gap="tiny">
          <Text variant="labelTiny" color="yellow">
            {warning.text}
          </Text>
          <Text variant="labelTiny" color="whiteAlpha60">
            {warning.reference}
          </Text>
        </FlexColumn>
      ))}
      {section.warnings.length > 0 ? (
        <Text variant="labelTiny" color="whiteAlpha60">
          {strikeContestHint()}
        </Text>
      ) : null}
      <ExternalLink variant="labelTinySemiBold" to={urls.reports}>
        <ExternalLink.Text>
          {
            // TRANSLATORS: Link to the website page on abuse reports, warnings
            // TRANSLATORS: and how to contest them.
            messages.pgettext('port-forwarding-view', 'Warnings and how to contest them')
          }
        </ExternalLink.Text>
        <ExternalLink.Icon
          aria-description={messages.pgettext('accessibility', 'Opens externally')}
          icon="external"
        />
      </ExternalLink>
    </FlexColumn>
  );
}
