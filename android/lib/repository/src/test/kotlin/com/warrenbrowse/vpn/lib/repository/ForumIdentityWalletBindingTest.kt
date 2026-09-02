package com.warrenbrowse.vpn.lib.repository

import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

@OptIn(ExperimentalCoroutinesApi::class)
class ForumIdentityWalletBindingTest {

    private val address = WalletAddress("wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB")
    private val handle = ForumIdentity("lusab-babad-dovok", 3)

    private class FakeWallet(initial: WalletState) : WalletRepository {
        val flow = MutableStateFlow(initial)
        override val state: StateFlow<WalletState> = flow.asStateFlow()

        override suspend fun createWallet(authorizer: SensitiveOpAuthorizer?): Mnemonic = error("unused")

        override suspend fun importWallet(mnemonic: Mnemonic, authorizer: SensitiveOpAuthorizer?) =
            error("unused")

        override suspend fun unlock(authorizer: SensitiveOpAuthorizer, reason: String): Mnemonic =
            error("unused")

        override suspend fun readMnemonic(): Mnemonic = error("unused")

        override suspend fun erase() {
            flow.value = WalletState.Absent
        }
    }

    private class FakeForumIdentities : ForumIdentityRepository {
        private val flow = MutableStateFlow<ForumIdentity?>(null)
        override val identity: StateFlow<ForumIdentity?> = flow.asStateFlow()
        var clears = 0

        override fun save(identity: ForumIdentity) {
            flow.value = identity
        }

        override fun clear() {
            clears++
            flow.value = null
        }
    }

    @Test
    fun erasing_the_wallet_clears_the_forum_identity() = runTest {
        val wallet = FakeWallet(WalletState.Locked(address))
        val identities = FakeForumIdentities().apply { save(handle) }
        ForumIdentityWalletBinding(wallet, identities, backgroundScope).start()
        runCurrent()
        assertEquals(handle, identities.identity.value)

        wallet.erase()
        runCurrent()

        assertNull(identities.identity.value)
    }

    @Test
    fun the_identity_survives_while_a_wallet_is_on_the_device() = runTest {
        val wallet = FakeWallet(WalletState.Locked(address))
        val identities = FakeForumIdentities().apply { save(handle) }
        ForumIdentityWalletBinding(wallet, identities, backgroundScope).start()
        runCurrent()

        wallet.flow.value = WalletState.Ready(address)
        runCurrent()

        assertEquals(handle, identities.identity.value)
        assertEquals(0, identities.clears)
    }

    @Test
    fun an_identity_left_behind_by_an_earlier_erase_is_cleared_at_start() = runTest {
        // Installs erased before this binding existed still hold the handle.
        val wallet = FakeWallet(WalletState.Absent)
        val identities = FakeForumIdentities().apply { save(handle) }

        ForumIdentityWalletBinding(wallet, identities, backgroundScope).start()
        runCurrent()

        assertNull(identities.identity.value)
    }

    @Test
    fun a_fresh_install_writes_nothing() = runTest {
        val wallet = FakeWallet(WalletState.Absent)
        val identities = FakeForumIdentities()

        ForumIdentityWalletBinding(wallet, identities, backgroundScope).start()
        runCurrent()

        assertEquals(0, identities.clears)
    }
}
