import { useCallback, useEffect } from 'react';
import { sprintf } from 'sprintf-js';
import styled from 'styled-components';

import { messages } from '../../../../../shared/gettext';
import { IncludeOnlyLabel, LockedAppsLabel } from '../../../../features/app-routing/components';
import { Icon, IconButton } from '../../../../lib/components';
import { ConnectionPhase, getConnectionPhase } from '../../../../lib/connection-phase';
import { useExitEgressDead } from '../../../../lib/exit-egress';
import { FontFamilies, homeTokens, surfaces } from '../../../../lib/foundations';
import { useHostOffline } from '../../../../lib/host-offline';
import { useBoolean } from '../../../../lib/utility-hooks';
import { useSelector } from '../../../../redux/store';
import CustomScrollbars from '../../../CustomScrollbars';
import { BackAction } from '../../../keyboard-navigation';
import { ConnectedExitLoad } from '../components/connection-panel/components/connected-exit-load';
import { ConnectionDetails } from '../components/connection-panel/components/connection-details';
import { getHomeStatus } from './home-status-text';

const { panel, pill, title, subtitle } = homeTokens;

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

const StyledSubtitleRow = styled.div`
  display: flex;
  align-items: center;
  gap: 8px;
  /* Fixed whether or not the details button is there, so the hero above keeps
     its frame across every state. */
  height: 18px;
  --load-ring-low: ${surfaces.homePillProtected};
  --load-ring-moderate: ${surfaces.homeBadge};
  --load-ring-high: ${surfaces.homePillPending};
  --load-ring-saturated: ${surfaces.homePillExposed};
  --load-ring-track: color-mix(in srgb, ${surfaces.homeTextSecondary} 30%, transparent);
  color: ${surfaces.homeTextSecondary};
`;

const StyledSubtitle = styled.span<{ $mono: boolean }>`
  flex: 0 1 auto;
  min-width: 0;
  font-family: ${(props) => (props.$mono ? FontFamilies.mono : FontFamilies.sans)};
  font-size: ${(props) => (props.$mono ? subtitle.monoSize : subtitle.textSize)}px;
  color: ${(props) => (props.$mono ? surfaces.homeTextMono : surfaces.homeTextSecondary)};
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
`;

const StyledDetailsButton = styled(IconButton)`
  --background: ${surfaces.homeTextSecondary};
  --hover: ${surfaces.homeText};
  --pressed: ${surfaces.homeText};
  flex-shrink: 0;
  margin-inline-start: auto;
`;

// The connection details open over the hero rather than pushing the controls:
// the window is fixed, and the art is what can give way.
const StyledDetailsSheet = styled.div<{ $open: boolean }>`
  position: absolute;
  left: 0;
  right: 0;
  bottom: calc(100% - ${panel.overlap}px);
  z-index: 3;
  max-height: 300px;
  display: flex;
  flex-direction: column;
  padding: 4px ${panel.paddingHorizontal}px 0;
  border-radius: 16px 16px 0 0;
  background-color: ${surfaces.homePage};
  box-shadow: 0 -12px 24px -12px rgba(0, 0, 0, 0.45);
  opacity: ${(props) => (props.$open ? 1 : 0)};
  transform: translateY(${(props) => (props.$open ? 0 : 12)}px);
  pointer-events: ${(props) => (props.$open ? 'auto' : 'none')};
  transition:
    opacity 200ms ease-out,
    transform 200ms ease-out;
`;

export function HomeStatus() {
  const tunnelState = useSelector((state) => state.connection.status);
  const connection = useSelector((state) => state.connection);
  const includeOnly = useSelector(
    (state) => state.settings.appRouting.splitMode === 'include-only',
  );
  const hostOffline = useHostOffline();
  const exitEgressDead = useExitEgressDead();
  const [detailsOpen, , closeDetails, toggleDetails] = useBoolean();

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
  const hasDetails = tunnelState.state === 'connected' || tunnelState.state === 'connecting';

  useEffect(closeDetails, [tunnelState.state, closeDetails]);
  const onToggleDetails = useCallback(() => {
    if (hasDetails) toggleDetails();
  }, [hasDetails, toggleDetails]);

  return (
    <BackAction disabled={!detailsOpen} action={closeDetails}>
      <StyledStatus role="status">
        {hasDetails && (
          <StyledDetailsSheet $open={detailsOpen} aria-hidden={!detailsOpen}>
            <CustomScrollbars>
              <ConnectionDetails />
            </CustomScrollbars>
          </StyledDetailsSheet>
        )}
        <StyledPill $background={colors.background} $text={colors.text} data-testid="home-pill">
          <StyledPillIcon icon={eye} $color={colors.text} />
          {status.pill}
        </StyledPill>
        <StyledTitle>{status.title}</StyledTitle>
        <StyledSubtitleRow>
          {status.subtitle && (
            <StyledSubtitle
              $mono={status.subtitleIsHostname}
              dir={status.subtitleIsHostname ? 'ltr' : undefined}
              data-testid="hostname-line">
              {status.subtitle}
            </StyledSubtitle>
          )}
          {tunnelState.state === 'connected' && <ConnectedExitLoad />}
          {hasDetails && (
            <StyledDetailsButton
              size="small"
              onClick={onToggleDetails}
              aria-expanded={detailsOpen}
              // TRANSLATORS: Accessible name of the button that shows the
              // TRANSLATORS: connection details on the home screen.
              aria-label={messages.pgettext('connect-view', 'Connection details')}
              data-testid="connection-panel-chevron">
              <IconButton.Icon icon={detailsOpen ? 'chevron-down' : 'chevron-up'} />
            </StyledDetailsButton>
          )}
        </StyledSubtitleRow>
        <IncludeOnlyLabel />
        <LockedAppsLabel />
      </StyledStatus>
    </BackAction>
  );
}
