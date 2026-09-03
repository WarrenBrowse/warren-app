package com.warrenbrowse.vpn.screen.outoftime

/**
 * Whether the subscription has lapsed, the verdict behind the out-of-time
 * gate (desktop `ExpiredAccountErrorView`, iOS `.outOfTime` route).
 *
 * The rules mirror the iOS coordinator:
 *   - A cached expiry in the future dismisses the gate the moment credit
 *     arrives (voucher redeem, purchase poll, manual re-check all write the
 *     shared cached expiry).
 *   - An unknown expiry (0: never fetched, or no subscription bound yet) is
 *     NOT "expired": it renders as "Currently unavailable" on the account
 *     screen instead of locking the user out. The verdict still lapses when
 *     the exit itself refused the account ([tunnelExpired]), which is the
 *     authoritative signal.
 */
internal fun subscriptionLapsed(expiryUnixSecs: Long, nowSecs: Long, tunnelExpired: Boolean): Boolean {
    if (expiryUnixSecs > nowSecs) return false
    return expiryUnixSecs > 0L || tunnelExpired
}

/**
 * Whether the full-screen out-of-time gate replaces the UI: the onboarding
 * and funding flows always win over it ([inMainFlow] is false there), because
 * a fresh wallet starts unfunded by design and the wizard itself handles
 * funding.
 */
internal fun outOfTimeGateActive(lapsed: Boolean, inMainFlow: Boolean): Boolean =
    inMainFlow && lapsed

/** What the host has to do to the back stack to match the gate verdict. */
internal enum class OutOfTimeGateAction {
    Push,
    Pop,
    None,
}

/**
 * Reconciles the gate verdict with the back stack.
 *
 * The gate is a destination rather than a branch that swaps the whole UI out, so
 * Settings and the account page stay reachable from it (desktop keeps the main
 * header up on `ExpiredAccountErrorView`). Reconciling on both inputs also makes
 * the gate self-healing: popped out of the stack while it is still raised, it
 * pushes itself straight back.
 */
internal fun outOfTimeGateAction(active: Boolean, inStack: Boolean): OutOfTimeGateAction =
    when {
        active && !inStack -> OutOfTimeGateAction.Push
        !active && inStack -> OutOfTimeGateAction.Pop
        else -> OutOfTimeGateAction.None
    }
