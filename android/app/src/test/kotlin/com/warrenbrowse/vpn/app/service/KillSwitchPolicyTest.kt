package com.warrenbrowse.vpn.app.service

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class KillSwitchPolicyTest {

    private fun decide(userInitiated: Boolean, flapping: Boolean, lockdown: Boolean) =
        KillSwitchPolicy.decide(userInitiated, flapping, lockdown)

    @Test
    fun `user-initiated teardown always releases`() {
        // Regardless of flapping / lockdown the user asked to disconnect.
        for (flapping in listOf(false, true)) {
            for (lockdown in listOf(false, true)) {
                assertEquals(
                    KillSwitchAction.RELEASE,
                    decide(userInitiated = true, flapping = flapping, lockdown = lockdown),
                )
            }
        }
    }

    @Test
    fun `single unexpected drop blocks and retries regardless of lockdown`() {
        // Fail closed first: never leak while the tunnel recovers.
        assertEquals(
            KillSwitchAction.BLOCK_AND_RETRY,
            decide(userInitiated = false, flapping = false, lockdown = false),
        )
        assertEquals(
            KillSwitchAction.BLOCK_AND_RETRY,
            decide(userInitiated = false, flapping = false, lockdown = true),
        )
    }

    @Test
    fun `flapping with lockdown parks (stays blocked)`() {
        assertEquals(
            KillSwitchAction.PARK,
            decide(userInitiated = false, flapping = true, lockdown = true),
        )
    }

    @Test
    fun `flapping without lockdown releases so the user is not stranded`() {
        assertEquals(
            KillSwitchAction.RELEASE,
            decide(userInitiated = false, flapping = true, lockdown = false),
        )
    }
}
