package com.warrenbrowse.vpn.lib.ui.component.wallet

import androidx.fragment.app.FragmentActivity
import com.warrenbrowse.vpn.lib.model.wallet.CipherAuthorizer
import com.warrenbrowse.vpn.lib.model.wallet.SensitiveOpAuthorizer
import javax.crypto.Cipher

/**
 * Concrete [SensitiveOpAuthorizer] backed by Android `BiometricPrompt`.
 *
 * The repository layer is wired against the abstract
 * [SensitiveOpAuthorizer] interface; the UI layer assembles a
 * [BiometricPromptAuthorizer] for a given [FragmentActivity] and passes
 * it into `WalletRepository.unlock(authorizer)` whenever a cleartext
 * mnemonic read is required.
 *
 * The class holds a reference to the activity; tear-down responsibility
 * sits with the caller (don't outlive the activity, e.g. don't pass to
 * a long-running coroutine without a `LifecycleScope` cancellation
 * point).
 *
 * Behaviour matrix:
 *  - User authenticates -> `authorize` returns `true`.
 *  - User cancels / hardware error -> `false`.
 *  - Device has no biometric hardware -> `false` (the repository will
 *    throw `WalletAuthorizationDeniedException`; the caller chooses
 *    whether to fall back to a passcode prompt in a future iteration).
 */
class BiometricPromptAuthorizer(
    private val activity: FragmentActivity,
    private val title: String = "Warren wallet",
    private val negativeButton: String = "Cancel",
) : SensitiveOpAuthorizer, CipherAuthorizer {

    override suspend fun authorize(reason: String): Boolean {
        return when (val result = promptBiometric(activity, title, reason, negativeButton)) {
            BiometricResult.Success -> true
            is BiometricResult.Error -> false
            is BiometricResult.Unavailable -> {
                // TODO: fall back to a passcode prompt via
                //   KeyguardManager.createConfirmDeviceCredentialIntent
                //   when biometric hardware is missing.
                @Suppress("UNUSED_VARIABLE") val reason = result.reason
                false
            }
        }
    }

    override suspend fun authorizeCipher(cipher: Cipher, reason: String): Cipher? =
        promptBiometricForCipher(activity, title, reason, negativeButton, cipher)
}
