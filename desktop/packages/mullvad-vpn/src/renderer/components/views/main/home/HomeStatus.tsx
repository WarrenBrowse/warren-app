import { useEffect } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../../shared/gettext';
import { IncludeOnlyLabel, LockedAppsLabel } from '../../../../features/app-routing/components';
import { Icon } from '../../../../lib/components';
import { ConnectionPhase, getConnectionPhase } from '../../../../lib/connection-phase';
import { useExitEgressDead } from '../../../../lib/exit-egress';
import { FontFamilies, homeTokens, surfaces } from '../../../../lib/foundations';
import { useHostOffline } from '../../../../lib/host-offline';
import { useSelector } from '../../../../redux/store';
import { BackAction } from '../../../keyboard-navigation';
import { ConnectedExitLoad } from '../components/connection-panel/components/connected-exit-load';
import { ConnectionDetails } from '../components/connection-panel/components/connection-details';
import { getHomeStatus } from './home-status-text';

const { panel, pill, title, subtitle, exitRow } = homeTokens;

const pillColors: Record<ConnectionPhase, { background: string; text: string }> = {
  exposed: { background: surfaces.homePillExposed, text: surfaces.homePillExposedText },
  connecting: { background: surfaces.homePillPending, text: surfaces.homePillPendingText },
  protected: { background: surfaces.homePillProtected, text: surfaces.homePillProtectedText },
  interrupted: { background: surfaces.homePillPending, text: surfaces.homePillPendingText },
  blocked: { background: surfaces.homePillBlocked, text: surfaces.homePillBlockedText },
};

const StyledStatus = styled.div`
  position: relative;
  display: flex;
  flex-direction: column;
  gap: ${panel.gap}px;
  padding: 0 ${panel.paddingHorizontal}px;
  margin-top: -${panel.overlap}px;
`;

const StyledPill = styled.div<{ $background: string; $text: string }>`
  align-self: flex-start;
  display: flex;
  align-items: center;
  gap: ${pill.gap}px;
  padding: ${pill.paddingVertical}px ${pill.paddingEnd}px ${pill.paddingVertical}px
    ${pill.paddingStart}px;
  border-radius: 99px;
  background-color: ${(props) => props.$background};
  color: ${(props) => props.$text};
  box-shadow: 0 6px 18px -6px color-mix(in srgb, ${(props) => props.$background} 60%, transparent);
  font-family: ${FontFamilies.sans};
  font-size: ${pill.textSize}px;
  font-weight: ${pill.textWeight};
  line-height: 1.2;
  transition:
    background-color 300ms ease-out,
    color 300ms ease-out;
`;

const StyledPillIcon = styled(Icon)<{ $color: string }>`
  && {
    width: ${pill.icon}px;
    height: ${pill.icon}px;
    background-color: ${(props) => props.$color};
  }
`;

const StyledTitle = styled.h1`
  margin: ${title.marginTop}px 0 0;
  font-family: ${FontFamilies.sans};
  font-size: ${title.textSize}px;
  font-weight: ${title.textWeight};
  letter-spacing: ${title.letterSpacingEm}em;
  line-height: 1.2;
  color: ${surfaces.homeText};
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
`;

const StyledSubtitle = styled.p`
  margin: ${subtitle.marginTop}px 0 0;
  padding: ${subtitle.paddingVertical}px 0;
  font-family: ${FontFamilies.sans};
  font-size: ${subtitle.textSize}px;
  line-height: ${subtitle.lineHeight}px;
  color: ${surfaces.homeTextSecondary};
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
`;

// The exit line is the button that opens the details: its hover well reaches
// past the text on both sides, as the mockup draws it.
const StyledExitRow = styled.button`
  display: flex;
  align-items: center;
  gap: ${exitRow.gap}px;
  min-width: 0;
  margin: ${exitRow.marginTop}px -${exitRow.outset}px 0;
  padding: ${exitRow.paddingVertical}px ${exitRow.outset}px;
  border: none;
  border-radius: ${exitRow.radius}px;
  background: none;
  color: ${surfaces.homeTextMono};
  font-family: ${FontFamilies.sans};
  font-size: ${exitRow.textSize}px;
  text-align: start;
  cursor: pointer;
  transition: background-color 150ms ease-out;
  --load-ring-low: ${surfaces.homeLoad};
  --load-ring-moderate: ${surfaces.homeBadge};
  --load-ring-high: ${surfaces.homePillPending};
  --load-ring-saturated: ${surfaces.homePillExposed};
  --load-ring-track: ${surfaces.homeLoadTrack};
  &:hover,
  &:focus-visible {
    outline: none;
    background-color: ${surfaces.homeDetails};
  }
  & [data-testid='exit-load-badge'] {
    gap: ${exitRow.gap}px;
    color: inherit;
    font-size: ${exitRow.textSize}px;
    font-weight: normal;
  }
`;

const StyledExitText = styled.span<{ $mono: boolean }>`
  flex: 0 1 auto;
  min-width: 0;
  font-family: ${(props) => (props.$mono ? FontFamilies.mono : FontFamilies.sans)};
  color: ${(props) => (props.$mono ? surfaces.homeTextMono : surfaces.homeTextSecondary)};
  font-size: ${(props) => (props.$mono ? exitRow.textSize : subtitle.textSize)}px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
`;

const StyledChevron = styled(Icon)<{ $open: boolean }>`
  && {
    flex-shrink: 0;
    width: ${exitRow.chevron}px;
    height: ${exitRow.chevron}px;
    margin-inline-start: auto;
    background-color: currentColor;
    transform: rotate(${(props) => (props.$open ? 180 : 0)}deg);
    transition: transform ${exitRow.chevronMs}ms ease;
  }
`;

interface HomeStatusProps {
  detailsOpen: boolean;
  toggleDetails: () => void;
  closeDetails: () => void;
}

export function HomeStatus({ detailsOpen, toggleDetails, closeDetails }: HomeStatusProps) {
  const tunnelState = useSelector((state) => state.connection.status);
  const connection = useSelector((state) => state.connection);
  const includeOnly = useSelector(
    (state) => state.settings.appRouting.splitMode === 'include-only',
  );
  const hostOffline = useHostOffline();
  const exitEgressDead = useExitEgressDead();

  const phase = getConnectionPhase(tunnelState, hostOffline, exitEgressDead);
  const status = getHomeStatus(
    tunnelState,
    phase,
    {
      country: connection.country,
      city: connection.city,
      hostname:
        connection.hostname && connection.entryHostname
          ? sprintf(messages.pgettext('connection-info', '%(relay)s via %(entry)s'), {
              relay: connection.hostname,
              entry: connection.entryHostname,
            })
          : connection.hostname,
    },
    includeOnly,
  );
  const colors = pillColors[phase];
  const eye = phase === 'exposed' || phase === 'connecting' ? 'show' : 'hide';
  const connected = tunnelState.state === 'connected';

  useEffect(closeDetails, [tunnelState.state, closeDetails]);

  return (
    <BackAction disabled={!detailsOpen} action={closeDetails}>
      <StyledStatus role="status">
        <StyledPill $background={colors.background} $text={colors.text} data-testid="home-pill">
          <StyledPillIcon icon={eye} $color={colors.text} />
          {status.pill}
        </StyledPill>
        <StyledTitle>{status.title}</StyledTitle>
        {connected ? (
          <StyledExitRow
            type="button"
            onClick={toggleDetails}
            aria-expanded={detailsOpen}
            // TRANSLATORS: Accessible name of the button that shows the
            // TRANSLATORS: connection details on the home screen.
            aria-label={messages.pgettext('connect-view', 'Connection details')}
            data-testid="connection-panel-chevron">
            {status.subtitle && (
              <StyledExitText
                $mono={status.subtitleIsHostname}
                dir={status.subtitleIsHostname ? 'ltr' : undefined}
                data-testid="hostname-line">
                {status.subtitle}
              </StyledExitText>
            )}
            <ConnectedExitLoad />
            <StyledChevron icon="expand-more" $open={detailsOpen} aria-hidden />
          </StyledExitRow>
        ) : (
          <StyledSubtitle>{status.subtitle || '\u00a0'}</StyledSubtitle>
        )}
        {connected && detailsOpen && <ConnectionDetails />}
        <IncludeOnlyLabel />
        <LockedAppsLabel />
      </StyledStatus>
    </BackAction>
  );
}
