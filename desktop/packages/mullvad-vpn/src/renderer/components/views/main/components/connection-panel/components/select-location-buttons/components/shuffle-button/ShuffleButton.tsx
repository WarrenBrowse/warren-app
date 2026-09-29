import { useCallback, useMemo } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../../../../../../../shared/gettext';
import log from '../../../../../../../../../../shared/logging';
import { useAppContext } from '../../../../../../../../../context';
import { useRelayLocations } from '../../../../../../../../../features/locations/hooks';
import { ButtonProps, Icon } from '../../../../../../../../../lib/components';
import { surfaces } from '../../../../../../../../../lib/foundations';
import { CardButton } from '../../../card-button';

const StyledShuffleButton = styled(CardButton)({
  flexShrink: 0,
  justifyContent: 'center',
  width: '40px',
  minWidth: '40px',
});

// The icon set paints white by default; on the card it takes the text colour,
// which is dark on the cream theme.
const StyledShuffleIcon = styled(Icon)({
  backgroundColor: surfaces.text,
});

// Picks a random exit country among those with an active relay, then connects.
// "Surprise me" also works while connected: it re-rolls the exit and reconnects.
export function ShuffleButton(props: ButtonProps) {
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
      const error = e as Error;
      log.error(`Failed to shuffle the exit location: ${error.message}`);
    }
  }, [available, selectExitRelayLocation, connectTunnel]);

  return (
    <StyledShuffleButton
      $tone="neutral"
      onClick={onShuffle}
      // Disabled until the relay list has an active exit, so a click never
      // silently no-ops before the list loads.
      disabled={available.length === 0}
      // TRANSLATORS: Accessibility label for the button that connects to a random exit.
      aria-label={messages.pgettext('tunnel-control', 'Random location')}
      {...props}>
      <StyledShuffleIcon icon="shuffle" size="small" />
    </StyledShuffleButton>
  );
}
