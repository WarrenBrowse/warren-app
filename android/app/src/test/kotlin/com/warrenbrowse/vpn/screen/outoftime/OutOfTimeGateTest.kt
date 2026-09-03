package com.warrenbrowse.vpn.screen.outoftime

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class OutOfTimeGateTest {

    private val now = 1_000_000L

    private fun gate(
        expiryUnixSecs: Long,
        nowSecs: Long,
        inMainFlow: Boolean,
        tunnelExpired: Boolean,
    ): Boolean =
        outOfTimeGateActive(
            lapsed = subscriptionLapsed(expiryUnixSecs, nowSecs, tunnelExpired),
            inMainFlow = inMainFlow,
        )

    @Test
    fun `gate raises when a previously funded subscription has lapsed`() {
        assertTrue(
            gate(
                expiryUnixSecs = now - 60,
                nowSecs = now,
                inMainFlow = true,
                tunnelExpired = false,
            ),
        )
    }

    @Test
    fun `gate never raises during onboarding or funding flows`() {
        // The wizard handles funding itself; a fresh wallet always starts
        // unfunded, so the gate must yield to the flow (iOS rule).
        assertFalse(
            gate(
                expiryUnixSecs = now - 60,
                nowSecs = now,
                inMainFlow = false,
                tunnelExpired = true,
            ),
        )
    }

    @Test
    fun `gate stays down on an unknown expiry without a tunnel verdict`() {
        // 0 = never fetched or no subscription bound: shown as "Currently
        // unavailable" on the account page, not a lockout.
        assertFalse(
            gate(
                expiryUnixSecs = 0L,
                nowSecs = now,
                inMainFlow = true,
                tunnelExpired = false,
            ),
        )
    }

    @Test
    fun `gate raises on the exit refusing the account even with an unknown expiry`() {
        assertTrue(
            gate(
                expiryUnixSecs = 0L,
                nowSecs = now,
                inMainFlow = true,
                tunnelExpired = true,
            ),
        )
    }

    @Test
    fun `gate auto-dismisses the moment credit arrives`() {
        // A future expiry wins over a stale tunnel-expired verdict: the block
        // state persists until the next connect, but the account is paid.
        assertFalse(
            gate(
                expiryUnixSecs = now + 3_600,
                nowSecs = now,
                inMainFlow = true,
                tunnelExpired = true,
            ),
        )
    }

    @Test
    fun `expiry at exactly now is expired`() {
        assertTrue(
            gate(
                expiryUnixSecs = now,
                nowSecs = now,
                inMainFlow = true,
                tunnelExpired = false,
            ),
        )
    }

    @Test
    fun `a raised gate that is not on the stack is pushed`() {
        assertEquals(
            OutOfTimeGateAction.Push,
            outOfTimeGateAction(active = true, inStack = false),
        )
    }

    @Test
    fun `a cleared gate that is still on the stack is popped`() {
        assertEquals(OutOfTimeGateAction.Pop, outOfTimeGateAction(active = false, inStack = true))
    }

    @Test
    fun `a raised gate already on the stack is left alone`() {
        // The user may have walked from the gate into Settings or the account
        // page; re-pushing would tear that navigation down under them.
        assertEquals(OutOfTimeGateAction.None, outOfTimeGateAction(active = true, inStack = true))
    }

    @Test
    fun `a cleared gate that is not on the stack is left alone`() {
        assertEquals(OutOfTimeGateAction.None, outOfTimeGateAction(active = false, inStack = false))
    }
}
