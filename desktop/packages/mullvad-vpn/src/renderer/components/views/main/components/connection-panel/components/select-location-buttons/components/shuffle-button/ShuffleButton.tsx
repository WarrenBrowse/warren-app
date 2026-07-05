import { useCallback } from 'react';
import styled from 'styled-components';

import { messages } from '../../../../../../../../../../shared/gettext';
import log from '../../../../../../../../../../shared/logging';
import { useAppContext } from '../../../../../../../../../context';
import { useRelayLocations } from '../../../../../../../../../features/locations/hooks';
import { Button, ButtonProps } from '../../../../../../../../../lib/components';

const StyledShuffleButton = styled(Button)({
  minWidth: '40px',
});

const ShuffleGlyph = styled.svg({
  width: '20px',
  height: '20px',
});

// Picks a random exit country among those with an active relay, then connects.
// "Surprise me" also works while connected: it re-rolls the exit and reconnects.
export function ShuffleButton(props: ButtonProps) {
  const { relayLocations, selectExitRelayLocation } = useRelayLocations();
  const { connectTunnel } = useAppContext();

  const onShuffle = useCallback(async () => {
    const available = relayLocations.filter((country) =>
      country.cities.some((city) => city.relays.some((relay) => relay.active)),
    );
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
  }, [relayLocations, selectExitRelayLocation, connectTunnel]);

  return (
    <StyledShuffleButton
      onClick={onShuffle}
      width="fit"
      // TRANSLATORS: Accessibility label for the button that connects to a random exit.
      aria-label={messages.pgettext('tunnel-control', 'Random location')}
      {...props}>
      <ShuffleGlyph
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
        role="img"
        aria-hidden>
        <path d="M16 3h5v5" />
        <path d="M21 3 13 11" />
        <path d="M16 21h5v-5" />
        <path d="m15 15 6 6" />
        <path d="M3 4l6 6" />
      </ShuffleGlyph>
    </StyledShuffleButton>
  );
}
