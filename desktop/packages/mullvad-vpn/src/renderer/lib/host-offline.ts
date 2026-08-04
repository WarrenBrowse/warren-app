import { useEffect, useState } from 'react';

import { useSelector } from '../redux/store';

// The macOS offline monitor synthesizes a ~1 s offline blip on every
// network switch (wifi <-> ethernet) by design; rendering it would
// flash the red banner and the interrupted phase on every routine
// handover. Rising edges are therefore held for slightly longer than
// that synthetic window before the offline UI shows; falling edges
// (back online) apply immediately.
const HOST_OFFLINE_SHOW_DELAY_MS = 1200;

/// Debounced view of the daemon's host-offline verdict, shared by the
/// banner, the connection status and the backdrop so they flip
/// together.
export function useHostOffline(): boolean {
  const raw = useSelector((state) => state.settings.warrenStatus?.hostOffline ?? false);
  const [shown, setShown] = useState(false);

  useEffect(() => {
    if (!raw) {
      setShown(false);
      return;
    }
    const timeout = setTimeout(() => setShown(true), HOST_OFFLINE_SHOW_DELAY_MS);
    return () => clearTimeout(timeout);
  }, [raw]);

  return shown;
}
