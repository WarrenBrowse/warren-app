import { useCallback, useEffect } from 'react';
import styled from 'styled-components';

import { PortForwardingIndicator } from '../../../../../features/port-forwarding/components';
import { IconButton } from '../../../../../lib/components';
import { getConnectionPhase, getPhaseAccentColor } from '../../../../../lib/connection-phase';
import { useExitEgressDead } from '../../../../../lib/exit-egress';
import { colors } from '../../../../../lib/foundations';
import { useHostOffline } from '../../../../../lib/host-offline';
import { useBoolean } from '../../../../../lib/utility-hooks';
import { useSelector } from '../../../../../redux/store';
import CustomScrollbars from '../../../../CustomScrollbars';
import { BackAction } from '../../../../keyboard-navigation';
import { ConnectionPanelAccordion } from '../../styles';
import {
  ConnectionActionButton,
  ConnectionDetails,
  ConnectionStatus,
  FeatureIndicators,
  Hostname,
  Location,
  MultihopIndicator,
  SelectLocationButtons,
} from './components';

const PANEL_MARGIN = '16px';

const StyledAccordion = styled(ConnectionPanelAccordion)({
  flexShrink: 0,
});

// Bottom-anchored column holding the floating feature badges and, below them, the
// connection card. The badges sit over the scenery; the card is the glass box.
const StyledOuter = styled.div({
  display: 'flex',
  flexDirection: 'column',
  justifyContent: 'flex-end',
  gap: '8px',
  maxHeight: `calc(100% - 2 * ${PANEL_MARGIN})`,
  // Hug the very bottom so the card sits low and uncovers more of the scenery
  // (Bula + the burrow) above it.
  margin: `auto ${PANEL_MARGIN} 8px`,
});

// Feature pills stacked vertically, left-aligned, just above the card.
const StyledFeatureBadges = styled.div({
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'flex-start',
  gap: '5px',
  minHeight: 0,
  overflow: 'hidden',
});

// The card is the largest surface on the screen, so a plain translucent scrim
// let the landscape's hue flood it: over the watercolor grass it measured
// (77, 73, 41), a solid olive, which dropped the status title to 2.9:1 and the
// detail labels to 2.3:1 and broke the palette's own rule that neutrals stay
// neutral. Desaturating and darkening the backdrop BEFORE the scrim keeps the
// glass depth while pinning the surface neutral on every plate.
const StyledCard = styled.div<{ $accent: string }>((props) => ({
  position: 'relative',
  display: 'flex',
  flexDirection: 'column',
  flexShrink: 0,
  minHeight: 0,
  padding: '14px 16px',
  borderRadius: '16px',
  overflow: 'hidden',
  backgroundColor: colors.darkerBlue50Alpha80,
  backdropFilter: 'blur(14px) saturate(0.35) brightness(0.5)',
  border: `1px solid ${colors.whiteAlpha20}`,
  boxShadow: '0 10px 28px rgba(0, 0, 0, 0.45)',

  // The phase rail. It states connected vs exposed before a word is read, and
  // unlike the tinted title it survives at a glance and for a colour-blind
  // reader it still moves with the icon and the action button.
  '&::before': {
    content: '""',
    position: 'absolute',
    left: 0,
    top: 0,
    bottom: 0,
    width: '3px',
    backgroundColor: props.$accent,
    transition: 'background-color 300ms ease-out',
  },
}));

const StyledConnectionButtonContainer = styled.div({
  transition: 'margin-top 300ms ease-out',
  display: 'flex',
  flexDirection: 'column',
  gap: '12px',
  marginTop: '12px',
});

const StyledCustomScrollbars = styled(CustomScrollbars)({
  flexShrink: 1,
});

// Sits to the LEFT of the country flag, which owns the top-right corner in
// every state so it never appears to move when the chevron comes and goes.
const StyledConnectionPanelChevron = styled(IconButton)({
  position: 'absolute',
  top: '16px',
  right: '46px',
  width: 'fit-content',
});

const StyledConnectionStatusContainer = styled.div<{ $expanded: boolean }>((props) => ({
  paddingBottom: props.$expanded ? '16px' : 0,
  borderBottom: props.$expanded ? `1px ${colors.whiteAlpha20} solid` : 'none',
  transitionProperty: 'padding-bottom',
  transitionDuration: '300ms',
  transitionTimingFunction: 'ease-out',
}));

export function ConnectionPanel() {
  const [expanded, , collapse, toggleExpandedImpl] = useBoolean();
  const tunnelState = useSelector((state) => state.connection.status);

  const hostOffline = useHostOffline();
  const exitEgressDead = useExitEgressDead();
  const accent = getPhaseAccentColor(getConnectionPhase(tunnelState, hostOffline, exitEgressDead));

  const allowExpand = tunnelState.state === 'connected' || tunnelState.state === 'connecting';

  const toggleExpanded = useCallback(() => {
    if (allowExpand) {
      toggleExpandedImpl();
    }
  }, [allowExpand, toggleExpandedImpl]);

  useEffect(collapse, [tunnelState.state, collapse]);

  return (
    <BackAction disabled={!expanded} action={collapse}>
      <StyledOuter>
        <StyledFeatureBadges>
          <FeatureIndicators />
          <MultihopIndicator />
          <PortForwardingIndicator />
        </StyledFeatureBadges>
        <StyledCard $accent={accent}>
          {allowExpand && (
            <StyledConnectionPanelChevron
              onClick={toggleExpanded}
              data-testid="connection-panel-chevron">
              <IconButton.Icon icon={expanded ? 'chevron-down' : 'chevron-up'} />
            </StyledConnectionPanelChevron>
          )}
          <StyledConnectionStatusContainer $expanded={expanded} onClick={toggleExpanded}>
            <ConnectionStatus />
            <Location />
            <Hostname />
          </StyledConnectionStatusContainer>
          <StyledCustomScrollbars>
            <StyledAccordion expanded={expanded}>
              <ConnectionDetails />
            </StyledAccordion>
          </StyledCustomScrollbars>
          <StyledConnectionButtonContainer>
            <SelectLocationButtons />
            <ConnectionActionButton />
          </StyledConnectionButtonContainer>
        </StyledCard>
      </StyledOuter>
    </BackAction>
  );
}
