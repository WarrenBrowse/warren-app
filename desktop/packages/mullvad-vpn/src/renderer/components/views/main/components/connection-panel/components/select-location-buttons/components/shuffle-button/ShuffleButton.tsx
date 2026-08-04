import { useCallback, useMemo } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../../../../../../../shared/gettext';
import log from '../../../../../../../../../../shared/logging';
import { useAppContext } from '../../../../../../../../../context';
import { useRelayLocations } from '../../../../../../../../../features/locations/hooks';
import { Button, ButtonProps, Icon } from '../../../../../../../../../lib/components';

const StyledShuffleButton = styled(Button)({
  minWidth: '40px',
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
      onClick={onShuffle}
      // Disabled until the relay list has an active exit, so a click never
      // silently no-ops before the list loads.
      disabled={available.length === 0}
      width="fit"
      // TRANSLATORS: Accessibility label for the button that connects to a random exit.
      aria-label={messages.pgettext('tunnel-control', 'Random location')}
      {...props}>
      <Icon icon="shuffle" />
    </StyledShuffleButton>
  );
}
