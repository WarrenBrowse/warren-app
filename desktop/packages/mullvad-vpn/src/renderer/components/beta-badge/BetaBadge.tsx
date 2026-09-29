import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { isBetaBuild } from '../../../shared/constants/product-env';
import { urls } from '../../../shared/constants/urls';
import { messages } from '../../../shared/gettext';
import { Button, Text } from '../../lib/components';
import { colors, FontFamilies, surfaces } from '../../lib/foundations';
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

// The overlay banner is an opaque surface of the theme, like the connection
// card: over the scenery a translucent one took the landscape's hue.
const OverlayChip = styled.span`
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
  height: 21px;
  padding: 0 8px;
  border-radius: 6px;
  background-color: ${surfaces.pill};
  color: ${surfaces.pillText};
  font-family: ${FontFamilies.openSans};
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.5px;
`;

const OverlayLine = styled.span`
  color: ${surfaces.text};
  font-family: ${FontFamilies.openSans};
  font-size: 11px;
  font-weight: 600;
`;

// One line whatever the language: the banner widens to its text rather than
// wrapping it.
const OverlayCard = styled.button`
  display: flex;
  align-items: center;
  gap: 8px;
  width: max-content;
  height: 36.5px;
  padding: 0 12px 0 8px;
  white-space: nowrap;
  border-radius: 12px;
  border: 0.5px solid ${surfaces.line};
  background-color: ${surfaces.card};
  box-shadow: 0 2px 8px ${surfaces.shadowSoft};
  cursor: pointer;

  &:focus-visible {
    outline: 2px solid ${surfaces.text};
    outline-offset: 2px;
  }
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
  text-align: start;
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

  // TRANSLATORS: Accessibility label of the beta badge button.
  const ariaLabel = messages.pgettext('beta-badge', 'About the Warren beta');
  // TRANSLATORS: Label of the beta badge shown in beta builds.
  const betaLabel = messages.pgettext('beta-badge', 'BETA');
  return (
    <>
      {variant === 'overlay' ? (
        <OverlayCard onClick={showDialog} aria-label={ariaLabel}>
          <OverlayChip>{betaLabel}</OverlayChip>
          <OverlayLine>
            <BetaCapLine />
          </OverlayLine>
        </OverlayCard>
      ) : (
        <RowCard onClick={showDialog} aria-label={ariaLabel}>
          <Chip>
            <Text variant="labelTinySemiBold" color="darkBlue">
              {betaLabel}
            </Text>
          </Chip>
          <Text variant="footnoteMini" color="whiteAlpha60">
            <BetaCapLine />
          </Text>
        </RowCard>
      )}
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
