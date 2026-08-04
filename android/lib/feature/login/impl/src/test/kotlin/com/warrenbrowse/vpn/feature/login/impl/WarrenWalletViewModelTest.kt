package com.warrenbrowse.vpn.feature.login.impl

import androidx.lifecycle.viewModelScope
import app.cash.turbine.test
import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import io.mockk.unmockkAll
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class WarrenWalletViewModelTest {

    private val walletRepository: WalletRepository = mockk()
    private val walletState = MutableStateFlow<WalletState>(WalletState.Absent)

    private lateinit var viewModel: WarrenWalletViewModel

    @BeforeEach
    fun setup() {
        every { walletRepository.state } returns walletState
        viewModel = WarrenWalletViewModel(walletRepository)
    }

    @AfterEach
    fun tearDown() {
        viewModel.viewModelScope.coroutineContext.cancel()
        unmockkAll()
    }

    @Test
    fun `busy is raised while the wallet is minted and cleared once it lands`() = runTest {
        val keystore = CompletableDeferred<Mnemonic>()
        coEvery { walletRepository.createWallet(any()) } coAnswers { keystore.await() }

        viewModel.busy.test {
            assertEquals(false, awaitItem())
            viewModel.createWallet()
            assertEquals(true, awaitItem())
            keystore.complete(Mnemonic(VALID_PHRASE))
            assertEquals(false, awaitItem())
        }
    }

    @Test
    fun `busy is cleared when the mint fails`() = runTest {
        coEvery { walletRepository.createWallet(any()) } throws IllegalStateException("keystore")

        viewModel.createWallet()

        assertEquals(false, viewModel.busy.value)
    }

    @Test
    fun `a failed mint reports a typed reason rather than the engine message`() = runTest {
        coEvery { walletRepository.createWallet(any()) } throws
            IllegalStateException("/data/user/0/keystore blew up")

        viewModel.events.test {
            viewModel.createWallet()
            val event = awaitItem()
            assertIs<WarrenWalletEvent.Error>(event)
            assertEquals(WalletErrorReason.CreateFailed, event.reason)
        }
    }

    @Test
    fun `the typed phrase is held by the view model and normalized before the import`() = runTest {
        var imported: String? = null
        coEvery { walletRepository.importWallet(any(), any()) } answers
            {
                imported = firstArg<Mnemonic>().phrase
                WalletAddress(ADDRESS)
            }

        viewModel.setImportPhrase("  Abandon   ABANDON abandon abandon abandon abandon ")
        viewModel.setImportPhrase(
            viewModel.importPhrase.value +
                "abandon abandon abandon abandon abandon About  ",
        )
        viewModel.importWallet()

        assertEquals(VALID_PHRASE, imported)
    }

    @Test
    fun `a phrase with the wrong word count never reaches the repository`() = runTest {
        viewModel.setImportPhrase("abandon abandon about")

        viewModel.events.test {
            viewModel.importWallet()
            val event = awaitItem()
            assertIs<WarrenWalletEvent.Error>(event)
            assertEquals(WalletErrorReason.WrongWordCount, event.reason)
        }
        coVerify(exactly = 0) { walletRepository.importWallet(any(), any()) }
    }

    @Test
    fun `a phrase the engine rejects reports an invalid phrase`() = runTest {
        coEvery { walletRepository.importWallet(any(), any()) } throws
            IllegalArgumentException("checksum")

        viewModel.setImportPhrase(VALID_PHRASE)

        viewModel.events.test {
            viewModel.importWallet()
            val event = awaitItem()
            assertIs<WarrenWalletEvent.Error>(event)
            assertEquals(WalletErrorReason.InvalidPhrase, event.reason)
        }
    }

    @Test
    fun `a successful import clears the phrase the view model was holding`() = runTest {
        coEvery { walletRepository.importWallet(any(), any()) } returns WalletAddress(ADDRESS)

        viewModel.setImportPhrase(VALID_PHRASE)
        viewModel.importWallet()

        assertEquals("", viewModel.importPhrase.value)
    }

    private companion object {
        const val VALID_PHRASE =
            "abandon abandon abandon abandon abandon abandon " +
                "abandon abandon abandon abandon abandon about"
        const val ADDRESS = "wb7kgy8FF4rx4tamkksPfoymeeeZVXLrnSjbBxCun3XhP9DnB"
    }
}
