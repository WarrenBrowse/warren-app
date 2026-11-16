package com.warrenbrowse.vpn.lib.repository

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

/**
 * The label an exit is shown by, the same rule as the desktop daemon's
 * `exit_label` (`mullvad-daemon/src/warren_relay_list_view.rs`).
 */
class ExitLabelTest {

    private val pubkeyHex = "3cb7f0e1d2c3b4a5968778695a4b3c2d1e0f1a2b3c4d5e6f708192a3b4c5d6e7"

    @Test
    fun `a named exit is shown by its fleet name`() {
        assertEquals("fi-hel-bveh1", relay(name = "fi-hel-bveh1").label)
    }

    @Test
    fun `an unnamed exit is shown by the first sixteen hex characters of its key`() {
        assertEquals("warren-3cb7f0e1d2c3b4a5", relay(name = null).label)
    }

    private fun relay(name: String?) =
        WarrenRelaySummary(
            exitId = "2921abad869e94064b56cf48c8da3631",
            exitPubkeyHex = pubkeyHex,
            endpoint = "198.51.100.5:443",
            country = "FI",
            city = "Helsinki",
            active = true,
            weight = 100,
            name = name,
        )
}
