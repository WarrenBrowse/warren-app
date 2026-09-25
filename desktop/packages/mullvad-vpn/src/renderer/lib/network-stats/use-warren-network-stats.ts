import { useEffect, useRef, useState, useSyncExternalStore } from 'react';

import { NetworkStats, snapshotAgeSecs, snapshotIsStale } from '../../../shared/network-stats';
import { useSelector } from '../../redux/store';
import { getReduceMotion } from '../functions';
import { tweenAt } from './helpers';
import { NetworkStatsState, NetworkStatsStore } from './network-stats-store';

// IPC goes through the contextBridge-exposed `window.ipc`, read lazily so the
// store can be imported where no window exists.
const store = new NetworkStatsStore(() => window.ipc.warrenNetworkStats.get());

const TWEEN_MS = 800;

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
  // The last good snapshot is older than three windows: shown greyed with its
  // age, never replaced by zeros.
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

/** Seconds since the snapshot's window closed, re-read every second while shown. */
export function useSnapshotAge(stats: NetworkStats | undefined): number | undefined {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  return stats === undefined ? undefined : snapshotAgeSecs(stats, now);
}

/**
 * Moves from the previous value to `target` over 800 ms, easing out, and never
 * past it. With reduced motion the new value shows at once.
 */
export function useTweenedNumber(target: number): number {
  const [value, setValue] = useState(target);
  const shown = useRef(target);

  useEffect(() => {
    const from = shown.current;
    if (from === target || getReduceMotion()) {
      shown.current = target;
      setValue(target);
      return;
    }
    const start = performance.now();
    let frame = 0;
    const step = (time: number) => {
      const next = tweenAt(from, target, time - start, TWEEN_MS);
      shown.current = next;
      setValue(next);
      if (time - start < TWEEN_MS) {
        frame = requestAnimationFrame(step);
      }
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [target]);

  return value;
}
