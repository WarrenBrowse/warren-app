package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.app.forum.FakeWalletRepository
import com.warrenbrowse.vpn.lib.model.AccountBan
import com.warrenbrowse.vpn.lib.model.AccountStanding
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WarrenAccountStandingRepository
import kotlin.test.assertEquals
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test

@OptIn(ExperimentalCoroutinesApi::class)
class HeldVoucherRedeemerTest {

    private val standing = WarrenAccountStandingRepository()
    private val wallet = FakeWalletRepository()
    private var rounds = 0
    private val redeemer = HeldVoucherRedeemer(standing, wallet) { rounds++ }

    private fun banned(inForce: Boolean) =
        AccountStanding(
            emptyList(),
            threshold = 3,
            windowDays = 90,
            ban = AccountBan(portForwarding = true, lapsesAtUnixSecs = null, inForce = inForce),
        )

    @Test
    fun `a held voucher is tried at start while no ban is known`() =
        runTest(UnconfinedTestDispatcher()) {
            val job = launch { redeemer.run() }

            assertEquals(1, rounds)
            job.cancel()
        }

    @Test
    fun `nothing is tried while a ban holds, and a round runs when it lifts`() =
        runTest(UnconfinedTestDispatcher()) {
            standing.setStanding(banned(inForce = true))
            val job = launch { redeemer.run() }
            assertEquals(0, rounds)

            standing.setStanding(banned(inForce = false))

            assertEquals(1, rounds)
            job.cancel()
        }

    @Test
    fun `a standing that changes nothing about the ban starts no new round`() =
        runTest(UnconfinedTestDispatcher()) {
            val job = launch { redeemer.run() }

            standing.setStanding(banned(inForce = false))
            standing.setStanding(null)

            assertEquals(1, rounds)
            job.cancel()
        }

    @Test
    fun `nothing is tried without a wallet on the device`() =
        runTest(UnconfinedTestDispatcher()) {
            wallet.stateFlow.value = WalletState.Absent
            val job = launch { redeemer.run() }

            assertEquals(0, rounds)
            job.cancel()
        }
}
