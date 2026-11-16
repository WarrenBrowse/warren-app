import { useCallback, useMemo } from 'react';
import { sprintf } from 'sprintf-js';
import styled, { css } from 'styled-components';

import { messages } from '../../../../../shared/gettext';
import log from '../../../../../shared/logging';
import { RoutePath } from '../../../../../shared/routes';
import { useAppContext } from '../../../../context';
import { useRelayLocations } from '../../../../features/locations/hooks';
import { Icon } from '../../../../lib/components';
import { connectButtonDisabled } from '../../../../lib/env-yield';
import { FontFamilies, homeTokens, surfaces } from '../../../../lib/foundations';
import { TransitionType, useHistory } from '../../../../lib/history';
import { useSelector } from '../../../../redux/store';
import { exitPlaceName, getRelayName, selectedCountryCode } from './location-name';
import { LocationFlag } from './LocationFlag';

const { panel, controls } = homeTokens;

const StyledControls = styled.div`
  display: flex;
  flex-direction: column;
  gap: ${controls.gap}px;
  padding: ${controls.marginTop}px ${panel.paddingHorizontal}px 0;
`;

const StyledRow = styled.div`
  display: flex;
  gap: ${controls.gap}px;
`;

const field = css`
  display: flex;
  align-items: center;
  height: ${controls.rowHeight}px;
  border: none;
  border-radius: ${controls.radius}px;
  background-color: ${surfaces.homeField};
  color: ${surfaces.homeText};
  cursor: pointer;
  transition: background-color 150ms ease-out;
  &:hover:not(:disabled) {
    background-color: ${surfaces.homeFieldHover};
  }
  &:active:not(:disabled) {
    background-color: ${surfaces.homeFieldPressed};
  }
  &:disabled {
    cursor: default;
    opacity: 0.5;
  }
  &:focus-visible {
    outline: 2px solid ${surfaces.homeText};
    outline-offset: 1px;
  }
`;

const StyledLocationButton = styled.button`
  ${field}
  flex: 1;
  min-width: 0;
  gap: ${controls.rowGap}px;
  padding: 0 ${controls.paddingEnd}px 0 ${controls.paddingStart}px;
`;

const StyledLocationName = styled.span`
  flex: 1;
  min-width: 0;
  font-family: ${FontFamilies.sans};
  font-size: ${controls.labelSize}px;
  font-weight: ${controls.labelWeight};
  text-align: start;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
`;

const StyledChange = styled.span`
  flex-shrink: 0;
  font-family: ${FontFamilies.sans};
  font-size: ${controls.changeSize}px;
  color: ${surfaces.homeTextSecondary};
`;

const StyledShuffleButton = styled.button`
  ${field}
  flex-shrink: 0;
  justify-content: center;
  width: ${controls.rowHeight}px;
`;

const StyledShuffleIcon = styled(Icon)`
  && {
    background-color: ${surfaces.homeFieldIcon};
  }
`;

type ActionTone = 'connect' | 'disconnect' | 'cancel' | 'neutral';

const actionTones: Record<
  ActionTone,
  { background: string; hover: string; pressed: string; line: string; text: string }
> = {
  connect: {
    background: surfaces.homeConnect,
    hover: surfaces.homeConnectHover,
    pressed: surfaces.homeConnectPressed,
    line: 'transparent',
    text: surfaces.homeConnectText,
  },
  disconnect: {
    background: 'transparent',
    hover: surfaces.homeDisconnectHover,
    pressed: surfaces.homeDisconnectHover,
    line: surfaces.homeDisconnectLine,
    text: surfaces.homeDisconnectText,
  },
  cancel: {
    background: 'transparent',
    hover: surfaces.homeCancelHover,
    pressed: surfaces.homeCancelHover,
    line: surfaces.homeCancelLine,
    text: surfaces.homeCancelText,
  },
  neutral: {
    background: surfaces.homeField,
    hover: surfaces.homeFieldHover,
    pressed: surfaces.homeFieldPressed,
    line: 'transparent',
    text: surfaces.homeText,
  },
};

// The fill says what the click does: a solid go, an outlined stop.
const StyledActionButton = styled.button<{ $tone: ActionTone }>(({ $tone }) => {
  const tone = actionTones[$tone];
  return css`
    display: flex;
    align-items: center;
    justify-content: center;
    height: ${controls.buttonHeight}px;
    border: none;
    border-radius: ${controls.radius}px;
    background-color: ${tone.background};
    box-shadow: inset 0 0 0 ${controls.outlineWidth}px ${tone.line};
    color: ${tone.text};
    font-family: ${FontFamilies.sans};
    font-size: ${controls.buttonTextSize}px;
    font-weight: ${controls.buttonTextWeight};
    cursor: pointer;
    transition: background-color 150ms ease-out;
    &:hover:not(:disabled) {
      background-color: ${tone.hover};
    }
    &:active:not(:disabled) {
      background-color: ${tone.pressed};
    }
    &:disabled {
      cursor: default;
      opacity: 0.5;
    }
    &:focus-visible {
      outline: 2px solid ${surfaces.homeText};
      outline-offset: 1px;
    }
  `;
});

function LocationRow() {
  const { push } = useHistory();
  const connection = useSelector((state) => state.connection);
  const relaySettings = useSelector((state) => state.settings.relaySettings);
  const relayLocations = useSelector((state) => state.settings.relayLocations);
  const customLists = useSelector((state) => state.settings.customLists);

  const tunneled =
    connection.status.state === 'connected' || connection.status.state === 'connecting';
  const selectedName = useMemo(
    () => getRelayName(relaySettings, customLists, relayLocations),
    [relaySettings, customLists, relayLocations],
  );
  const exitName = tunneled ? exitPlaceName(connection.country, connection.city) : '';
  const name = exitName || selectedName;

  const selectedCode = selectedCountryCode(relaySettings);
  const exitCode = relayLocations.find((country) => country.name === connection.country)?.code;
  const code = (exitName ? exitCode : undefined) ?? selectedCode;

  const onSelectLocation = useCallback(() => {
    push(RoutePath.selectLocation, { transition: TransitionType.show });
  }, [push]);

  return (
    <StyledLocationButton
      onClick={onSelectLocation}
      aria-label={sprintf(
        messages.pgettext('accessibility', 'Select location. Current location is %(location)s'),
        { location: name },
      )}>
      <LocationFlag code={code} />
      <StyledLocationName>{name}</StyledLocationName>
      <StyledChange>
        {
          // TRANSLATORS: Trailing hint on the location row of the home screen,
          // TRANSLATORS: which opens the location list.
          messages.pgettext('tunnel-control', 'Change')
        }
      </StyledChange>
    </StyledLocationButton>
  );
}

// Picks a random exit country among those with an active relay, then connects.
// It also works while connected: it re-rolls the exit and reconnects.
function ShuffleButton() {
  const { relayLocations, selectExitRelayLocation } = useRelayLocations();
  const { connectTunnel } = useAppContext();

  const available = useMemo(
    () =>
      relayLocations.filter((country) =>
        country.cities.some((city) => city.relays.some((relay) => relay.active)),
      ),
    [relayLocations],
  );

  const onShuffle = useCallback(async () => {
    if (available.length === 0) {
      return;
    }
    const pick = available[Math.floor(Math.random() * available.length)];
    try {
      await selectExitRelayLocation({ country: pick.code });
      await connectTunnel();
    } catch (e) {
      log.error(`Failed to shuffle the exit location: ${(e as Error).message}`);
    }
  }, [available, selectExitRelayLocation, connectTunnel]);

  return (
    <StyledShuffleButton
      onClick={onShuffle}
      // Disabled until the relay list has an active exit, so a click never
      // silently no-ops before the list loads.
      disabled={available.length === 0}
      // TRANSLATORS: Accessibility label for the button that connects to a random exit.
      aria-label={messages.pgettext('tunnel-control', 'Random location')}>
      <StyledShuffleIcon icon="shuffle" size="small" />
    </StyledShuffleButton>
  );
}

function ActionButton() {
  const { connectTunnel, disconnectTunnel } = useAppContext();
  const tunnelState = useSelector((state) => state.connection.status.state);
  const envYield = useSelector((state) => state.settings.warrenStatus?.envYield);

  const onConnect = useCallback(async () => {
    try {
      await connectTunnel();
    } catch (e) {
      log.error(`Failed to connect the tunnel: ${(e as Error).message}`);
    }
  }, [connectTunnel]);

  const onDisconnect = useCallback(async () => {
    try {
      await disconnectTunnel('gui-disconnect-button');
    } catch (e) {
      log.error(`Failed to disconnect the tunnel: ${(e as Error).message}`);
    }
  }, [disconnectTunnel]);

  if (tunnelState === 'disconnected' || tunnelState === 'disconnecting') {
    return (
      <StyledActionButton
        $tone="connect"
        onClick={onConnect}
        disabled={connectButtonDisabled(tunnelState, envYield)}>
        {messages.pgettext('tunnel-control', 'Connect')}
      </StyledActionButton>
    );
  }

  // While connecting the click aborts the attempt, so the button reads Cancel;
  // in the blocked state the action is turning the switch off, a neutral one.
  const tone: ActionTone =
    tunnelState === 'connected'
      ? 'disconnect'
      : tunnelState === 'connecting'
        ? 'cancel'
        : 'neutral';
  return (
    <StyledActionButton $tone={tone} onClick={onDisconnect}>
      {tunnelState === 'connecting' ? messages.gettext('Cancel') : messages.gettext('Disconnect')}
    </StyledActionButton>
  );
}

export function HomeControls() {
  return (
    <StyledControls>
      <StyledRow>
        <LocationRow />
        <ShuffleButton />
      </StyledRow>
      <ActionButton />
    </StyledControls>
  );
}
