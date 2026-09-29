import React, { useCallback, useEffect } from 'react';
import styled from 'styled-components';

import {
  AppCountriesIndicator,
  IncludeOnlyLabel,
} from '../../../../../features/app-routing/components';
import { PortForwardingIndicator } from '../../../../../features/port-forwarding/components';
import { IconButton } from '../../../../../lib/components';
import { surfaces } from '../../../../../lib/foundations';
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

const PANEL_MARGIN = '14px';

// The one gap between the blocks of the card (status, location, buttons), so the
// space the card holds is shared between them rather than pooling under the
// last button.
const CARD_GAP = '10.5px';

const StyledAccordion = styled(ConnectionPanelAccordion)({
  flexShrink: 0,
});

// Bottom-anchored column holding the floating feature badges and, below them, the
// connection card. The badges sit over the scenery; the card is the glass box.
const StyledOuter = styled.div({
  display: 'flex',
  flexDirection: 'column',
  justifyContent: 'flex-end',
  gap: '4px',
  maxHeight: `calc(100% - 2 * ${PANEL_MARGIN})`,
  // Hug the very bottom so the card sits low and uncovers more of the scenery
  // (Bula + the burrow) above it.
  margin: `auto ${PANEL_MARGIN} 6px`,
});

// Feature pills stacked vertically, left-aligned, just above the card.
const StyledFeatureBadges = styled.div({
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'flex-start',
  gap: '2px',
  minHeight: 0,
  overflow: 'hidden',
});

// Opaque, in the theme's paper. A translucent card let each landscape tint it,
// so the same card read olive on one country and slate on the next.
const StyledCard = styled.div({
  position: 'relative',
  display: 'flex',
  flexDirection: 'column',
  // Shrinks in a short window so the details scroll inside it: a card that
  // cannot shrink grows over the top edge and takes its chevron with it.
  flexShrink: 1,
  minHeight: 0,
  padding: '20px 20px',
  borderRadius: '16px',
  overflow: 'hidden',
  backgroundColor: surfaces.card,
  border: `0.5px solid ${surfaces.line}`,
  boxShadow: `0 5px 16px ${surfaces.shadowStrong}`,
});

const StyledConnectionButtonContainer = styled.div({
  transition: 'margin-top 300ms ease-out',
  flexShrink: 0,
  display: 'flex',
  flexDirection: 'column',
  gap: CARD_GAP,
  marginTop: CARD_GAP,
});

const StyledCustomScrollbars = styled(CustomScrollbars)({
  flexShrink: 1,
});

// Sits in the status row before the country flag, which owns the end of the
// row in every state so it never appears to move when the chevron comes and goes.
const StyledConnectionPanelChevron = styled(IconButton)({
  '--background': surfaces.text,
  '--hover': surfaces.textMuted,
  '--pressed': surfaces.textSecondary,
  display: 'grid',
  placeItems: 'center',
  width: '22px',
  height: '22px',
  borderRadius: '6px',
  '&&:focus-visible': {
    outline: `2px solid ${surfaces.text}`,
    outlineOffset: '0',
  },
});

const StyledConnectionStatusContainer = styled.div<{ $expanded: boolean }>((props) => ({
  flexShrink: 0,
  paddingBottom: props.$expanded ? '16px' : 0,
  borderBottom: props.$expanded ? `1px ${surfaces.line} solid` : 'none',
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

  // The row it sits in expands the card on click as well, so the chevron's own
  // click stops there instead of toggling twice.
  const onChevronClick = useCallback(
    (event: React.MouseEvent) => {
      event.stopPropagation();
      toggleExpanded();
    },
    [toggleExpanded],
  );

  return (
    <BackAction disabled={!expanded} action={collapse}>
      <StyledOuter>
        <StyledFeatureBadges>
          <FeatureIndicators />
          <MultihopIndicator />
          <PortForwardingIndicator />
          <AppCountriesIndicator />
        </StyledFeatureBadges>
        <StyledCard>
          <StyledConnectionStatusContainer $expanded={expanded} onClick={toggleExpanded}>
            <ConnectionStatus
              trailing={
                allowExpand && (
                  <StyledConnectionPanelChevron
                    size="small"
                    onClick={onChevronClick}
                    data-testid="connection-panel-chevron">
                    <IconButton.Icon icon={expanded ? 'chevron-down' : 'chevron-up'} />
                  </StyledConnectionPanelChevron>
                )
              }
            />
            <IncludeOnlyLabel />
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
