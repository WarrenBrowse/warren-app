import { useCallback, useEffect } from 'react';
import styled from 'styled-components';

import { PortForwardingIndicator } from '../../../../../features/port-forwarding/components';
import { IconButton } from '../../../../../lib/components';
import { colors } from '../../../../../lib/foundations';
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
  margin: `auto ${PANEL_MARGIN} ${PANEL_MARGIN}`,
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

const StyledCard = styled.div<{ $expanded: boolean }>((props) => ({
  position: 'relative',
  display: 'flex',
  flexDirection: 'column',
  flexShrink: 0,
  minHeight: 0,
  padding: '16px',
  borderRadius: '16px',
  // Denser glass than upstream: the card floats over full-bleed daylight artwork,
  // so it needs more body to keep text legible.
  backgroundColor: props.$expanded ? colors.blackAlpha60 : colors.blackAlpha50,
  backdropFilter: 'blur(10px)',
  border: `1px solid ${colors.whiteAlpha20}`,
  boxShadow: '0 8px 24px rgba(0, 0, 0, 0.35)',

  transition: 'background-color 300ms ease-out',
}));

const StyledConnectionButtonContainer = styled.div({
  transition: 'margin-top 300ms ease-out',
  display: 'flex',
  flexDirection: 'column',
  gap: '16px',
  marginTop: '16px',
});

const StyledCustomScrollbars = styled(CustomScrollbars)({
  flexShrink: 1,
});

const StyledConnectionPanelChevron = styled(IconButton)({
  position: 'absolute',
  top: '16px',
  right: '16px',
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
        <StyledCard $expanded={expanded}>
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
