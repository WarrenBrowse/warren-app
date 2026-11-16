package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import android.content.SharedPreferences
import com.warrenbrowse.vpn.lib.model.wallet.Mnemonic
import io.mockk.every
import io.mockk.mockk
import kotlin.test.assertFailsWith
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test

class AndroidKeystoreWalletRepositoryImportTest {

    private val prefs: SharedPreferences = mockk {
        every { getString(any(), any()) } returns null
    }
    private val context: Context = mockk {
        every { getSharedPreferences(any(), any()) } returns prefs
    }

    // Deriving the address is the only step that reads the words; the engine
    // fails it for a phrase BIP39 rejects. Every later failure (the Keystore,
    // the authentication prompt) happens to a valid phrase, and the screen
    // must not call those an invalid phrase.
    @Test
    fun `a phrase the engine cannot derive is reported as an invalid phrase`() = runTest {
        val jni: WarrenJniBridge = mockk {
            every { mnemonicPubkeySs58(any()) } throws RuntimeException("invalid BIP39 mnemonic")
        }
        val repository = AndroidKeystoreWalletRepository(context, jni)

        assertFailsWith<InvalidMnemonicException> {
            repository.importWallet(Mnemonic(PHRASE), authorizer = null)
        }
    }

    private companion object {
        const val PHRASE =
            "abandon abandon abandon abandon abandon abandon " +
                "abandon abandon abandon abandon abandon abandon"
    }
}
