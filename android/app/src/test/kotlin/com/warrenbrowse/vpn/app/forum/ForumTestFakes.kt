package com.warrenbrowse.vpn.app.forum

import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.ForumIdentityRepository
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** A real Warren SS58 vector (49 chars, prefix 13295), the wallet the fakes hold. */
internal const val TEST_ADDRESS = "wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"

internal const val TEST_PHRASE =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

/** A wallet on disk whose silent reads are counted, so a test can prove none happened. */
internal class FakeWalletRepository(initial: WalletState = WalletState.Locked(WalletAddress(TEST_ADDRESS))) :
    WalletRepository {
    val stateFlow = MutableStateFlow(initial)
    override val state: StateFlow<WalletState> = stateFlow.asStateFlow()
    var mnemonicReads = 0

    override suspend fun createWallet(authorizer: SensitiveOpAuthorizer?): Mnemonic = error("unused")

    override suspend fun importWallet(mnemonic: Mnemonic, authorizer: SensitiveOpAuthorizer?): WalletAddress =
        error("unused")

    override suspend fun unlock(authorizer: SensitiveOpAuthorizer, reason: String): Mnemonic = error("unused")

    override suspend fun readMnemonic(): Mnemonic {
        mnemonicReads++
        return Mnemonic(TEST_PHRASE)
    }

    override suspend fun erase() {
        stateFlow.value = WalletState.Absent
    }
}

internal class FakeForumIdentityRepository : ForumIdentityRepository {
    private val _identity = MutableStateFlow<ForumIdentity?>(null)
    override val identity: StateFlow<ForumIdentity?> = _identity.asStateFlow()

    override fun save(identity: ForumIdentity) {
        _identity.value = identity
    }

    override fun clear() {
        _identity.value = null
    }
}

internal class FakeTunnelStateProvider(initial: WarrenConnectedInfo = WarrenConnectedInfo.Disconnected) :
    WarrenTunnelStateProvider {
    val info = MutableStateFlow(initial)
    override val state: StateFlow<String> = MutableStateFlow("").asStateFlow()
    override val connectedInfo: StateFlow<WarrenConnectedInfo> = info.asStateFlow()
}

/**
 * The JNI seam with every network export answering from the test, and counted:
 * the point of most tests here is that a call did or did not cross into Rust.
 */
internal class FakeJniBridge(
    private val loginAnswer: () -> String = { """{"ok":true}""" },
    private val reportAnswer: () -> String = { """{"ok":true,"topic_id":1,"topic_url":"","logs":"none"}""" },
    private val collectAnswer: () -> String = { """{"ok":true,"bytes":7}""" },
) : WarrenJniBridge {
    var loginCalls = 0
    var cancelCalls = 0
    var reportCalls = 0
    val collectedMetadata = mutableListOf<String>()

    override fun generateMnemonic(): String = error("unused")

    override fun mnemonicPubkeySs58(mnemonic: String): String = error("unused")

    override fun checkVersionSupported(currentVersion: String): Boolean = error("unused")

    override fun latestAvailableVersion(currentVersion: String): String? = error("unused")

    override fun fetchNetworkInfo(): String = error("unused")

    override fun forumLogin(mnemonic: String, sid: String, host: String): String {
        loginCalls++
        return loginAnswer()
    }

    override fun forumLoginCancel(sid: String, host: String) {
        cancelCalls++
    }

    override fun forumReport(mnemonic: String, reportJson: String, logGz: ByteArray?): String {
        reportCalls++
        return reportAnswer()
    }

    override fun collectProblemReport(
        metadataJson: String,
        redactJson: String,
        appLogDir: String,
        outputPath: String,
    ): String {
        collectedMetadata += metadataJson
        return collectAnswer()
    }
}
