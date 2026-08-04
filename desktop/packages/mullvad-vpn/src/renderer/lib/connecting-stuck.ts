import { useEffect, useState } from 'react';

import { useSelector } from '../redux/store';

// A healthy connect completes in a few seconds; the daemon's own retry
// backoff caps at 15 s. Past this window the attempt is almost certainly
// stuck (captive portal, blocked UDP, dead exit), so the UI switches from
// the neutral "Connecting" banner to a help hint pointing at the forum.
const CONNECTING_STUCK_DELAY_MS = 45_000;

/// True once the tunnel has been continuously in the connecting state for
/// longer than the stuck window. Detail updates within the connecting
/// state (endpoint changes, retry attempts) do not reset the timer; any
/// state transition does.
export function useConnectingStuck(): boolean {
  const connecting = useSelector((state) => state.connection.status.state === 'connecting');
  const [stuck, setStuck] = useState(false);

  useEffect(() => {
    if (!connecting) {
      setStuck(false);
      return;
    }
    const timeout = setTimeout(() => setStuck(true), CONNECTING_STUCK_DELAY_MS);
    return () => clearTimeout(timeout);
  }, [connecting]);

  return stuck;
}
