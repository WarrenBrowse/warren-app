import React from 'react';

import { usePortForwarding } from './use-port-forwarding';

/**
 * Why the port control is (or is about to be) blocked.
 *
 * - `rate-limited`: the exit already rate-limited the last change; the
 *   daemon is auto-retrying and the user must wait `remainingSecs`.
 * - `budget-exhausted`: the mapping is active but no rate-limit slots
 *   remain — the *next* change would trigger a ban, so we pre-emptively
 *   block until a slot frees (`remainingSecs`).
 * - `last-chance`: exactly one slot remains. Not blocked, but the UI
 *   warns so the user slows down.
 * - `null`: plenty of budget, no warning.
 */
export type NatPmpPortBlockReason = 'rate-limited' | 'budget-exhausted' | 'last-chance' | null;

export interface NatPmpPortBlock {
  /** The port/protocol controls must be disabled while this is true. */
  blocked: boolean;
  /** Whole seconds left on the countdown (0 when not counting down). */
  remainingSecs: number;
  /** Why, for the message the UI shows. */
  reason: NatPmpPortBlockReason;
}

/**
 * Derives the live "can the user change the port right now?" state from
 * the NAT-PMP status the exit reports.
 *
 * The exit rate-limits port allocations per source (sliding window). It
 * now reports the remaining budget on every successful mapping and a
 * retry-after on a rate-limit rejection; this hook turns that into a
 * single ticking countdown the port input and the status readout share,
 * so the UI can warn before a ban and block (with a countdown) during
 * one. The block clears on its own when the countdown reaches 0 — the
 * daemon's refresh loop retries automatically and pushes a fresh status.
 */
export function useNatPmpPortBlock(): NatPmpPortBlock {
  const { status } = usePortForwarding();

  // Wall-clock instant the current status snapshot arrived. The exit's
  // retry-after / window-reset values are relative to that moment.
  const anchorRef = React.useRef(Date.now());
  const [now, setNow] = React.useState(() => Date.now());
  React.useEffect(() => {
    anchorRef.current = Date.now();
    setNow(Date.now());
  }, [status]);

  let targetSecs = 0;
  let reason: NatPmpPortBlockReason = null;
  let counting = false;
  if (status.state === 'rate-limited') {
    targetSecs = status.retryAfterSecs;
    reason = 'rate-limited';
    counting = true;
  } else if (status.state === 'mapped') {
    // `attemptsRemaining` is undefined on a pre-trailer exit — in that
    // case we cannot warn proactively, so leave the control unblocked.
    if (status.attemptsRemaining === 0) {
      targetSecs = status.windowResetSecs;
      reason = 'budget-exhausted';
      counting = true;
    } else if (status.attemptsRemaining === 1) {
      reason = 'last-chance';
    }
  }

  React.useEffect(() => {
    if (!counting) {
      return undefined;
    }
    const intervalId = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(intervalId);
  }, [counting, status]);

  const elapsedSecs = Math.floor((now - anchorRef.current) / 1000);
  const remainingSecs = counting ? Math.max(0, targetSecs - elapsedSecs) : 0;
  // Stop blocking the moment the countdown elapses: the daemon is
  // retrying and a fresh `mapped` status will land shortly. Keeping the
  // control disabled past 0 would strand the user.
  const blocked = counting && remainingSecs > 0;

  return { blocked, remainingSecs, reason: blocked || reason === 'last-chance' ? reason : null };
}

/** Formats a whole-second count as `mm:ss` for countdown display. */
export function formatCountdown(totalSecs: number): string {
  const mm = Math.floor(totalSecs / 60)
    .toString()
    .padStart(2, '0');
  const ss = (totalSecs % 60).toString().padStart(2, '0');
  return `${mm}:${ss}`;
}
