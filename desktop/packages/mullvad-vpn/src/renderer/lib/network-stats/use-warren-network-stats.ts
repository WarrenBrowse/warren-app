import { useEffect, useState, useSyncExternalStore } from 'react';

import { snapshotIsStale } from '../../../shared/network-stats';
import { useSelector } from '../../redux/store';
import { NetworkStatsState, NetworkStatsStore } from './network-stats-store';

// IPC goes through the contextBridge-exposed `window.ipc`, read lazily so the
// store can be imported where no window exists.
const store = new NetworkStatsStore(() => window.ipc.warrenNetworkStats.get());

function useWindowHasFocus(): boolean {
  const reduxFocused = useSelector((state) => state.userInterface.windowFocused);
  const [domFocused, setDomFocused] = useState(() => document.hasFocus());

  useEffect(() => {
    const onFocus = () => setDomFocused(true);
    const onBlur = () => setDomFocused(false);
    window.addEventListener('focus', onFocus);
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('focus', onFocus);
      window.removeEventListener('blur', onBlur);
    };
  }, []);

  return reduxFocused || domFocused;
}

export interface WarrenNetworkStatsView extends NetworkStatsState {
  // The last good snapshot is older than three windows: shown greyed, never
  // replaced by zeros.
  stale: boolean;
}

/**
 * The live network figures, shared by every surface that shows them. Asks the
 * daemon once per window, and only while the window has focus: the window is
 * created with background throttling off, so page visibility cannot tell a
 * hidden window from a shown one.
 */
export function useWarrenNetworkStats(): WarrenNetworkStatsView {
  const focused = useWindowHasFocus();
  useEffect(() => store.setVisible(focused), [focused]);
  const state = useSyncExternalStore(store.subscribe, store.getState);
  const stale = state.stats !== undefined && snapshotIsStale(state.stats, Date.now());
  return { ...state, stale };
}
