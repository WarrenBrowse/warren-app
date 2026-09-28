package com.warrenbrowse.vpn.lib.usecase

import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnDisconnectInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import java.io.IOException
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.MethodSource

@OptIn(ExperimentalCoroutinesApi::class)
class WarrenLogoutUseCaseTest {

    private val events = mutableListOf<String>()

    private val tunnel =
        object : WarrenTunnelStateProvider {
            val info = MutableStateFlow<WarrenConnectedInfo>(WarrenConnectedInfo.Disconnected)
            override val state: StateFlow<String> = MutableStateFlow("")
            override val connectedInfo: StateFlow<WarrenConnectedInfo> = info.asStateFlow()
        }

    /** Stands in for the service round trip: the teardown lands only when the test says so. */
    private val disconnect =
        object : WarrenQuinnDisconnectInvoker {
            override fun disconnect() {
                events += "disconnect"
                tunnel.info.value = WarrenConnectedInfo.Disconnecting()
            }
        }

    private var eraseFailure: Exception? = null

    private val wallet =
        object : WalletRepository {
            val flow =
                MutableStateFlow<WalletState>(
                    WalletState.Locked(
                        WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB")
                    )
                )
            override val state: StateFlow<WalletState> = flow.asStateFlow()

            override suspend fun createWallet(authorizer: SensitiveOpAuthorizer?): Mnemonic =
                error("unused")

            override suspend fun importWallet(
                mnemonic: Mnemonic,
                authorizer: SensitiveOpAuthorizer?,
            ) = error("unused")

            override suspend fun unlock(
                authorizer: SensitiveOpAuthorizer,
                reason: String,
            ): Mnemonic = error("unused")

            override suspend fun readMnemonic(): Mnemonic = error("unused")

            override suspend fun erase() {
                eraseFailure?.let { throw it }
                events += "erase while ${tunnel.info.value::class.simpleName}"
                flow.value = WalletState.Absent
            }
        }

    private fun useCase() =
        WarrenLogoutUseCase(
            tunnelState = tunnel,
            disconnect = disconnect,
            wallet = wallet,
            teardownTimeoutMs = TEARDOWN_TIMEOUT_MS,
        )

    @ParameterizedTest
    @MethodSource("liveTunnelStates")
    fun `logging out takes a live tunnel down before the wallet is erased`(
        live: WarrenConnectedInfo
    ) = runTest {
        tunnel.info.value = live

        val logout = async { useCase()() }
        runCurrent()

        // The teardown was asked for and has not landed yet: the wallet must
        // still be there, or the user is logged out with a tunnel carrying
        // their traffic (beta 1.1.38).
        assertEquals(listOf("disconnect"), events)
        assertTrue(wallet.state.value is WalletState.Locked)

        tunnel.info.value = WarrenConnectedInfo.Disconnected
        assertEquals(WarrenLogoutOutcome.LoggedOut, logout.await())
        assertEquals(listOf("disconnect", "erase while Disconnected"), events)
    }

    @Test
    fun `a teardown that never lands leaves the wallet in place`() = runTest {
        tunnel.info.value = CONNECTED

        val logout = async { useCase()() }
        advanceTimeBy(TEARDOWN_TIMEOUT_MS + 1)

        assertEquals(WarrenLogoutOutcome.TunnelStillUp, logout.await())
        assertEquals(listOf("disconnect"), events)
        assertTrue(wallet.state.value is WalletState.Locked)
    }

    @Test
    fun `logging out with no tunnel erases without starting the service`() = runTest {
        // A disconnect dispatch starts the tunnel service when nothing runs,
        // which would raise a notification for a logged-out app.
        assertEquals(WarrenLogoutOutcome.LoggedOut, useCase()())

        assertEquals(listOf("erase while Disconnected"), events)
        assertTrue(wallet.state.value is WalletState.Absent)
    }

    @Test
    fun `an erase that fails is reported rather than taken for a logout`() = runTest {
        val failure = IOException("keystore")
        eraseFailure = failure

        val outcome = useCase()()

        assertEquals(WarrenLogoutOutcome.EraseFailed(failure), outcome)
        assertFalse(wallet.state.value is WalletState.Absent)
    }

    companion object {
        private const val TEARDOWN_TIMEOUT_MS = 15_000L

        private val CONNECTED =
            WarrenConnectedInfo.Connected(
                exitEndpointHost = "198.51.100.7:443",
                entryEndpointHost = null,
                multiHop = false,
                daita = false,
                assignedNatPmpPort = 40_000,
            )

        @JvmStatic
        fun liveTunnelStates() =
            listOf(
                CONNECTED,
                WarrenConnectedInfo.Connecting(),
                WarrenConnectedInfo.Reconnecting(),
                // The kill switch holding a blackhole interface is a VPN too:
                // it keeps the device offline for an app that can no longer
                // connect.
                WarrenConnectedInfo.Blocking(reason = "flapping", flapping = true),
                WarrenConnectedInfo.Failed(reason = "exit refused"),
            )
    }
}
