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
  const { mappings, statusReceivedAt } = usePortForwarding();

  // Anchor = when THIS snapshot actually arrived from the daemon (stored
  // in redux), NOT when this component mounted. This is the whole fix for
  // the "warning flickers on every navigation / persists 10 min after the
  // last change" bug: the exit only pushes a fresh status on a mapping
  // event (~30 min apart between renewals), so between events the snapshot
  // is stale. Anchoring to the real arrival time means a stale snapshot's
  // window has already elapsed → no block, no warning, no flicker. The
  // window self-clears purely from the clock, with zero new events.
  //
  // The exit's `windowResetSecs` is "seconds until ONE rate-limit slot
  // frees" (the sliding 60 s window's oldest entry falls out). So:
  //   - `attemptsRemaining === 0`: blocked until one slot frees, after
  //     which exactly one change is allowed again.
  //   - `attemptsRemaining === 1`: a heads-up ("last chance"); it stops
  //     being the last chance once a slot frees, i.e. after the SAME
  //     window. Both therefore expire at `arrival + windowResetSecs`.
  //
  // Multi-port: the rate-limit budget is SHARED across all of a client's
  // mappings (per-source on the exit), so we aggregate across mappings —
  // a single rate-limited mapping blocks every port control, and the
  // budget is the minimum `attemptsRemaining` any mapping reports.
  const anchor = statusReceivedAt ?? Date.now();
  const [now, setNow] = React.useState(() => Date.now());

  let windowSecs = 0;
  let reason: NatPmpPortBlockReason = null;
  let isBlock = false;

  // A live rate-limit on ANY mapping blocks everything (longest wait wins).
  const rateLimited = mappings.map((m) => m.status).filter((s) => s.state === 'rate-limited');
  if (rateLimited.length > 0) {
    windowSecs = Math.max(
      ...rateLimited.map((s) => (s.state === 'rate-limited' ? s.retryAfterSecs : 0)),
    );
    reason = 'rate-limited';
    isBlock = true;
  } else {
    // Otherwise look at the shared budget reported on the mapped entries.
    // `attemptsRemaining` is undefined on a pre-trailer exit — ignore
    // those (cannot reason about budget). Take the most constrained.
    let minAttempts: number | undefined;
    let budgetWindowSecs = 0;
    for (const m of mappings) {
      if (m.status.state === 'mapped' && m.status.attemptsRemaining !== undefined) {
        if (minAttempts === undefined || m.status.attemptsRemaining < minAttempts) {
          minAttempts = m.status.attemptsRemaining;
          budgetWindowSecs = m.status.windowResetSecs;
        }
      }
    }
    if (minAttempts === 0) {
      windowSecs = budgetWindowSecs;
      reason = 'budget-exhausted';
      isBlock = true;
    } else if (minAttempts === 1) {
      windowSecs = budgetWindowSecs;
      reason = 'last-chance';
    }
  }

  // A 0-second window (or a missing anchor) carries no live information —
  // never start a ticking/blocking state from it.
  const counting = reason !== null && windowSecs > 0;

  React.useEffect(() => {
    if (!counting) {
      return undefined;
    }
    const intervalId = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(intervalId);
  }, [counting, anchor, windowSecs]);

  const elapsedSecs = Math.floor((now - anchor) / 1000);
  const remainingSecs = counting ? Math.max(0, windowSecs - elapsedSecs) : 0;
  // The whole rate-limit state (block AND warning) is live only while the
  // window has time left. Past 0 the exit's budget has recovered at least
  // one slot; keeping the control disabled — or the warning up — would
  // strand the user (and is exactly the stale-snapshot bug). A fresh event
  // re-arms it with accurate numbers if the user keeps changing ports.
  const active = counting && remainingSecs > 0;
  const blocked = active && isBlock;

  return { blocked, remainingSecs, reason: active ? reason : null };
}

/** Formats a whole-second count as `mm:ss` for countdown display. */
export function formatCountdown(totalSecs: number): string {
  const mm = Math.floor(totalSecs / 60)
    .toString()
    .padStart(2, '0');
  const ss = (totalSecs % 60).toString().padStart(2, '0');
  return `${mm}:${ss}`;
}
