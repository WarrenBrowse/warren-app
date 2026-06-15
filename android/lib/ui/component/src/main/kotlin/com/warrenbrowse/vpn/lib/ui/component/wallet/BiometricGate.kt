package com.warrenbrowse.vpn.lib.ui.component.wallet

import androidx.biometric.BiometricManager
import androidx.biometric.BiometricPrompt
import androidx.fragment.app.FragmentActivity
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.suspendCancellableCoroutine

/**
 * Result of a [promptBiometric] interaction.
 */
sealed interface BiometricResult {
    data object Success : BiometricResult
    data class Error(val code: Int, val message: String) : BiometricResult

    /**
     * Device has no biometric hardware or no enrolled credential. The
     * caller can either fall back to a passcode prompt or skip the gate
     * for development builds.
     */
    data class Unavailable(val reason: Int) : BiometricResult
}

/**
 * Show the system [BiometricPrompt] with the supplied reason text and
 * suspend until the user authenticates, cancels, or the system reports
 * a permanent unavailability.
 *
 * `activity` must be a [FragmentActivity] because that is what
 * `BiometricPrompt` requires; the typical caller is `MainActivity`. The
 * `reason` should be a short i18n string lifted from the relevant
 * `wallet_biometric_*` resource (D.5).
 *
 * The function is cancellable: tear-down on the calling coroutine
 * `cancel()` propagates to `BiometricPrompt.cancelAuthentication()`.
 *
 * On devices without a fingerprint sensor / face unlock and without a
 * lockscreen PIN we return [BiometricResult.Unavailable] rather than
 * throwing: the caller chooses whether to refuse the operation (the
 * secure default) or to fall through (development mode).
 */
suspend fun promptBiometric(
    activity: FragmentActivity,
    title: String,
    reason: String,
    negativeButton: String = "Cancel",
): BiometricResult {
    // Allow the device PIN / pattern / password as a fallback alongside strong
    // biometrics: a user with no enrolled fingerprint/face must still be able to
    // unlock their wallet (view recovery phrase, sign to connect). Without this
    // fallback such a device is permanently locked out of its own wallet.
    val allowed =
        BiometricManager.Authenticators.BIOMETRIC_STRONG or
            BiometricManager.Authenticators.DEVICE_CREDENTIAL
    val canAuthenticate = BiometricManager.from(activity).canAuthenticate(allowed)
    if (canAuthenticate != BiometricManager.BIOMETRIC_SUCCESS) {
        return BiometricResult.Unavailable(canAuthenticate)
    }

    return suspendCancellableCoroutine { cont ->
        val prompt = BiometricPrompt(
            activity,
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(
                    result: BiometricPrompt.AuthenticationResult,
                ) {
                    if (cont.isActive) cont.resume(BiometricResult.Success)
                }

                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                    if (cont.isActive) {
                        cont.resume(BiometricResult.Error(errorCode, errString.toString()))
                    }
                }

                override fun onAuthenticationFailed() {
                    // No-op: the user gets to retry. The prompt only
                    // dismisses on explicit cancel / hardware error.
                }
            },
        )
        // When DEVICE_CREDENTIAL is allowed a negative button MUST NOT be set
        // (BiometricPrompt throws): the system provides its own cancel control.
        val info = BiometricPrompt.PromptInfo.Builder()
            .setTitle(title)
            .setSubtitle(reason)
            .setAllowedAuthenticators(allowed)
            .build()

        cont.invokeOnCancellation {
            // Propagate coroutine cancel into the prompt teardown so
            // the system UI dismisses promptly.
            runCatching { prompt.cancelAuthentication() }
                .onFailure { e -> if (cont.isActive) cont.resumeWithException(e) }
        }

        prompt.authenticate(info)
    }
}

/**
 * Like [promptBiometric] but binds the authentication to a [cipher] via
 * `BiometricPrompt.CryptoObject`, so a `setUserAuthenticationRequired(true)`
 * Keystore key can be used only after the user authenticates. Returns the
 * authorized cipher on success, or `null` on cancel / error / hardware
 * unavailability (the caller fails closed).
 */
suspend fun promptBiometricForCipher(
    activity: FragmentActivity,
    title: String,
    reason: String,
    negativeButton: String,
    cipher: javax.crypto.Cipher,
): javax.crypto.Cipher? {
    val canAuthenticate = BiometricManager.from(activity)
        .canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_STRONG)
    if (canAuthenticate != BiometricManager.BIOMETRIC_SUCCESS) {
        return null
    }

    return suspendCancellableCoroutine { cont ->
        val prompt = BiometricPrompt(
            activity,
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(
                    result: BiometricPrompt.AuthenticationResult,
                ) {
                    if (cont.isActive) cont.resume(result.cryptoObject?.cipher)
                }

                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                    if (cont.isActive) cont.resume(null)
                }

                override fun onAuthenticationFailed() {
                    // No-op: the user retries; the prompt stays up.
                }
            },
        )
        val info = BiometricPrompt.PromptInfo.Builder()
            .setTitle(title)
            .setSubtitle(reason)
            .setNegativeButtonText(negativeButton)
            .setAllowedAuthenticators(BiometricManager.Authenticators.BIOMETRIC_STRONG)
            .build()

        cont.invokeOnCancellation {
            runCatching { prompt.cancelAuthentication() }
        }

        prompt.authenticate(info, BiometricPrompt.CryptoObject(cipher))
    }
}
