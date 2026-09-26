package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.lib.model.AccountBan
import com.warrenbrowse.vpn.lib.model.AccountStanding
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

class WarrenVoucherJsonTest {

    @Test
    fun `a redeemed voucher carries the new expiry`() {
        assertEquals(
            WarrenVoucherOutcome.Success(1_800_000_000),
            parseVoucherJson("""{"ok":true,"expires_at":1800000000}"""),
        )
    }

    @Test
    fun `a ban refusal is its own outcome with the lapse when known`() {
        val outcome =
            parseVoucherJson(
                """{"ok":false,"error":"banned",""" +
                    """"ban":{"reason":"port_forwarding_abuse","lapses_at_unix_secs":1821536000}}"""
            )

        assertEquals(
            WarrenVoucherOutcome.Banned(
                AccountBan(portForwarding = true, lapsesAtUnixSecs = 1_821_536_000, inForce = true)
            ),
            outcome,
        )
    }

    @Test
    fun `a ban refusal without a lapse is a ban with no known end`() {
        val outcome =
            parseVoucherJson(
                """{"ok":false,"error":"banned","ban":{"reason":"other","lapses_at_unix_secs":null}}"""
            )

        assertEquals(
            WarrenVoucherOutcome.Banned(
                AccountBan(portForwarding = false, lapsesAtUnixSecs = null, inForce = true)
            ),
            outcome,
        )
    }

    @Test
    fun `any other refusal is a failure`() {
        assertEquals(
            WarrenVoucherOutcome.Failure("register failed: server returned status 409"),
            parseVoucherJson(
                """{"ok":false,"error":"register failed: server returned status 409"}"""
            ),
        )
    }

    @Test
    fun `a ban refusal shows before any standing answer`() {
        val ban = AccountBan(portForwarding = true, lapsesAtUnixSecs = null, inForce = true)

        assertEquals(AccountStanding(emptyList(), 0, 0, ban), withRefusalBan(null, ban))
    }

    @Test
    fun `a ban refusal leaves the ban the standing answered, which knows its lapse`() {
        val known = AccountBan(portForwarding = true, lapsesAtUnixSecs = 1_821_536_000, inForce = true)
        val standing = AccountStanding(emptyList(), 3, 90, known)

        val merged =
            withRefusalBan(
                standing,
                AccountBan(portForwarding = true, lapsesAtUnixSecs = null, inForce = true),
            )

        assertEquals(standing, merged)
    }
}
