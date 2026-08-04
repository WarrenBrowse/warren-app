import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { isBetaBuild } from '../../../shared/constants/product-env';
import { urls } from '../../../shared/constants/urls';
import { messages } from '../../../shared/gettext';
import { Button, Text } from '../../lib/components';
import { colors } from '../../lib/foundations';
import { useBoolean } from '../../lib/utility-hooks';
import { useSelector } from '../../redux/store';
import { ExternalLink } from '../ExternalLink';
import { ModalAlert, ModalAlertType } from '../Modal';

const BPS_PER_MBPS = 1_000_000;

const Chip = styled.span`
  display: inline-flex;
  align-items: center;
  padding: 1px 8px;
  border-radius: 8px;
  background-color: ${colors.yellow};
`;

// Glass pill matching the NotificationBanner card language, for the
// map/connect view where it floats over the scenery backdrop.
const OverlayCard = styled.button`
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border-radius: 14px;
  border: 1px solid ${colors.whiteAlpha20};
  background-color: ${colors.blackAlpha60};
  backdrop-filter: blur(10px);
  cursor: pointer;
`;

// Full-width flat card matching the settings/account card surfaces.
const RowCard = styled.button`
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 12px 16px;
  border-radius: 12px;
  border: none;
  background-color: ${colors.blue10};
  cursor: pointer;
  text-align: left;
`;

export interface BetaBadgeProps {
  // 'overlay' floats over the connect view scenery; 'row' is a flat
  // full-width card for settings-style lists.
  variant: 'overlay' | 'row';
}

// The beta identity badge. Compiled out of non-beta builds: the gate is
// the build-time env (never runtime data), so a prod build can never
// show it and a beta build always does.
export function BetaBadge({ variant }: BetaBadgeProps) {
  const [dialogVisible, showDialog, hideDialog] = useBoolean(false);

  if (!isBetaBuild) {
    return null;
  }

  const Card = variant === 'overlay' ? OverlayCard : RowCard;
  return (
    <>
      <Card
        onClick={showDialog}
        aria-label={
          // TRANSLATORS: Accessibility label of the beta badge button.
          messages.pgettext('beta-badge', 'About the Warren beta')
        }>
        <Chip>
          <Text variant="labelTinySemiBold" color="darkBlue">
            {
              // TRANSLATORS: Label of the beta badge shown in beta builds.
              messages.pgettext('beta-badge', 'BETA')
            }
          </Text>
        </Chip>
        <Text variant="footnoteMini" color="whiteAlpha60">
          <BetaCapLine />
        </Text>
      </Card>
      <BetaInfoDialog visible={dialogVisible} onClose={hideDialog} />
    </>
  );
}

// Short degraded-service line, fed by the live cap from the daemon's
// network-info feed when available.
function BetaCapLine() {
  const networkInfo = useSelector((state) => state.settings.warrenStatus?.networkInfo);
  const capMbps = networkInfo?.defaultRateBps
    ? Math.round(networkInfo.defaultRateBps / BPS_PER_MBPS)
    : undefined;
  return capMbps !== undefined
    ? sprintf(
        // TRANSLATORS: Short beta banner line. Available placeholders:
        // TRANSLATORS: %(mbps)d - the bandwidth cap in Mbps
        messages.pgettext('beta-badge', 'Free beta network, speed capped at %(mbps)d Mbps'),
        { mbps: capMbps },
      )
    : // TRANSLATORS: Short beta banner line when the cap figure is not known yet.
      messages.pgettext('beta-badge', 'Free beta network, limited bandwidth');
}

function BetaInfoDialog({ visible, onClose }: { visible: boolean; onClose: () => void }) {
  const networkInfo = useSelector((state) => state.settings.warrenStatus?.networkInfo);
  const capMbps = networkInfo?.defaultRateBps
    ? Math.round(networkInfo.defaultRateBps / BPS_PER_MBPS)
    : undefined;

  const capSentence =
    capMbps !== undefined
      ? sprintf(
          // TRANSLATORS: Beta info dialog sentence. Available placeholders:
          // TRANSLATORS: %(mbps)d - the bandwidth cap in Mbps
          messages.pgettext(
            'beta-badge',
            'It runs on a separate network with bandwidth capped at %(mbps)d Mbps.',
          ),
          { mbps: capMbps },
        )
      : // TRANSLATORS: Beta info dialog sentence when the cap figure is not known yet.
        messages.pgettext('beta-badge', 'It runs on a separate network with limited bandwidth.');

  return (
    <ModalAlert
      isOpen={visible}
      type={ModalAlertType.info}
      title={
        // TRANSLATORS: Title of the beta info dialog.
        messages.pgettext('beta-badge', 'Warren beta')
      }
      message={[
        // TRANSLATORS: First paragraph of the beta info dialog.
        messages.pgettext(
          'beta-badge',
          'This app uses the free Warren beta, here to help us validate Warren in real conditions.',
        ),
        capSentence,
        // TRANSLATORS: Paragraph of the beta info dialog clarifying that the beta terms are temporary.
        messages.pgettext(
          'beta-badge',
          'These are not the final service conditions: the full-speed network is a separate, paid product.',
        ),
      ]}
      buttons={[
        <Button key="close" onClick={onClose}>
          <Button.Text>{messages.gettext('Got it!')}</Button.Text>
        </Button>,
      ]}
      close={onClose}>
      <ExternalLink variant="labelTinySemiBold" to={urls.forum}>
        <ExternalLink.Text>
          {
            // TRANSLATORS: Link to the community forum in the beta info dialog.
            messages.pgettext('beta-badge', 'Share your feedback on the forum')
          }
        </ExternalLink.Text>
        <ExternalLink.Icon icon="external" />
      </ExternalLink>
    </ModalAlert>
  );
}
