package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.lib.repository.PendingVoucher
import com.warrenbrowse.vpn.lib.repository.PendingVoucherStore
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class PurchaseVoucherKeeperTest {

    private class MemoryStore : PendingVoucherStore {
        val held = mutableListOf<PendingVoucher>()
        var diskFull = false

        override fun all(): List<PendingVoucher> = held.toList()

        override fun hold(pending: PendingVoucher) {
            if (diskFull) throw java.io.IOException("disk full")
            if (pending !in held) held += pending
        }

        override fun forget(voucher: String) {
            held.removeAll { it.voucher == voucher }
        }
    }

    /** The JNI as the keeper sees it: scripted answers, and a record of every call. */
    private class FakeBridge(
        var pull: String = PULLED,
        var redeem: (String) -> String = { REDEEMED },
    ) : PurchaseVoucherBridge {
        val calls = mutableListOf<String>()
        var heldAtRedemption: List<PendingVoucher>? = null
        var store: PendingVoucherStore? = null

        override fun pullPurchaseVoucher(claimCode: String): String {
            calls += "pull"
            return pull
        }

        // The wallet a phrase signs for: the redemption credits it, whatever the UI shows.
        override fun walletOf(mnemonic: String): String =
            if (mnemonic == PHRASE) WALLET else OTHER_WALLET

        override fun redeemVoucher(mnemonic: String, voucher: String): String {
            calls += "redeem $voucher"
            heldAtRedemption = store?.all()
            return redeem(voucher)
        }
    }

    private val store = MemoryStore()
    private val bridge = FakeBridge().also { it.store = store }
    private val keeper = PurchaseVoucherKeeper(bridge, store)

    @Test
    fun `a pulled voucher is sealed in the store before its redemption is asked for`() {
        keeper.collect(CLAIM, PHRASE)

        assertEquals(listOf(PendingVoucher(WALLET, VOUCHER)), bridge.heldAtRedemption)
    }

    @Test
    fun `a voucher that could not be sealed is not redeemed`() {
        store.diskFull = true

        assertFailsWith<java.io.IOException> { keeper.collect(CLAIM, PHRASE) }
        assertEquals(listOf("pull"), bridge.calls)
    }

    @Test
    fun `a pulled voucher is held for the wallet the phrase signs for`() {
        bridge.redeem = { """{"ok":false,"error":"register failed: server returned status 503"}""" }

        keeper.collect(CLAIM, OTHER_PHRASE)

        assertEquals(listOf(PendingVoucher(OTHER_WALLET, VOUCHER)), store.held)
    }

    @Test
    fun `nothing is redeemed while the payment has queued nothing`() {
        bridge.pull = """{"ok":false,"error":"purchase pending"}"""

        assertNull(keeper.collect(CLAIM, PHRASE))
        assertEquals(listOf("pull"), bridge.calls)
        assertTrue(store.held.isEmpty())
    }

    @Test
    fun `a redeemed voucher leaves the store`() {
        val outcome = keeper.collect(CLAIM, PHRASE)

        assertEquals(WarrenVoucherOutcome.Success(1_800_000_000), outcome)
        assertTrue(store.held.isEmpty())
    }

    @Test
    fun `a verdict on the voucher itself removes it`() {
        bridge.redeem = { """{"ok":false,"error":"voucher rejected"}""" }

        assertEquals(WarrenVoucherOutcome.Rejected, keeper.collect(CLAIM, PHRASE))
        assertTrue(store.held.isEmpty())
    }

    @Test
    fun `a ban, a throttle or a server error keeps the voucher`() {
        for (answer in
            listOf(
                """{"ok":false,"error":"banned","ban":{"reason":"port_forwarding_abuse"}}""",
                """{"ok":false,"error":"register failed: server returned status 429"}""",
                """{"ok":false,"error":"register failed: server returned status 503"}""",
            )) {
            bridge.redeem = { answer }

            keeper.collect(CLAIM, PHRASE)

            assertEquals(listOf(PendingVoucher(WALLET, VOUCHER)), store.held, answer)
        }
    }

    @Test
    fun `a held voucher is redeemed for its own wallet only, and a ban stops the round`() {
        store.hold(PendingVoucher(OTHER_WALLET, "AAAA-BBBB-CCCC-DDDD"))
        store.hold(PendingVoucher(WALLET, VOUCHER))
        store.hold(PendingVoucher(WALLET, "EEEE-FFFF-GGGG-HHHH"))
        bridge.redeem = { """{"ok":false,"error":"banned","ban":{"reason":"other"}}""" }

        val outcomes = keeper.redeemHeld(PHRASE)

        assertEquals(listOf("redeem $VOUCHER"), bridge.calls)
        assertEquals(1, outcomes.size)
        assertEquals(3, store.held.size)
    }

    @Test
    fun `held vouchers of the wallet are redeemed and leave the store`() {
        store.hold(PendingVoucher(WALLET, VOUCHER))
        store.hold(PendingVoucher(OTHER_WALLET, "AAAA-BBBB-CCCC-DDDD"))

        keeper.redeemHeld(PHRASE)

        assertEquals(listOf(PendingVoucher(OTHER_WALLET, "AAAA-BBBB-CCCC-DDDD")), store.held)
        assertTrue(keeper.holds(OTHER_WALLET))
        assertTrue(!keeper.holds(WALLET))
    }

    private companion object {
        const val WALLET = "wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"
        const val OTHER_WALLET = "wb5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
        const val CLAIM = "claim"
        const val PHRASE = "phrase"
        const val OTHER_PHRASE = "other phrase"
        const val VOUCHER = "QWRT-YPLK-JHGF-DSAZ"
        const val PULLED = """{"ok":true,"voucher":"$VOUCHER"}"""
        const val REDEEMED = """{"ok":true,"expires_at":1800000000}"""
    }
}
