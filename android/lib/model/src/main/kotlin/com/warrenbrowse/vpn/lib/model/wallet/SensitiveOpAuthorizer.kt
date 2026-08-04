package com.warrenbrowse.vpn.lib.model.wallet

/**
 * Pluggable authorizer for sensitive wallet operations.
 *
 * The repository layer cannot reference Android `Activity` / Compose
 * UI types directly (cross-platform contract). Instead, the UI layer
 * supplies an instance of this interface, typically backed by
 * `lib/ui/component/wallet/BiometricGate.promptBiometric`. The
 * repository invokes [authorize] before any cleartext mnemonic read.
 *
 * Implementations should:
 *   - Show a `BiometricPrompt` (or equivalent) with the supplied
 *     `reason`.
 *   - Return `true` on successful user authentication.
 *   - Return `false` on user cancel, repeated failure, or hardware
 *     unavailability (the caller decides whether to proceed insecurely
 *     in dev builds or to refuse the operation).
 *
 * A `NoOpAuthorizer` that always returns `true` is appropriate **only**
 * for unit tests and ephemeral dev flows.
 */
fun interface SensitiveOpAuthorizer {
    suspend fun authorize(reason: String): Boolean
}

/** Test / dev-only authorizer that bypasses the prompt. */
object AlwaysAuthorize : SensitiveOpAuthorizer {
    override suspend fun authorize(reason: String): Boolean = true
}

/**
 * Authorizes a [javax.crypto.Cipher] against a hardware-bound
 * `BiometricPrompt.CryptoObject`, returning the same cipher once the Keystore
 * op is authorized at the secure boundary (or `null` if the user cancels /
 * hardware is unavailable). This is what an auth-required Keystore key
 * (`setUserAuthenticationRequired(true)`) needs: the crypto op itself is
 * gated, not just a separate boolean prompt, so an in-process caller cannot
 * use the cipher without a fresh user authentication.
 *
 * Separate from [SensitiveOpAuthorizer] so existing boolean-gate callers stay
 * unchanged; the Keystore repository uses this only when hardware-bound auth
 * is enabled.
 */
interface CipherAuthorizer {
    suspend fun authorizeCipher(
        cipher: javax.crypto.Cipher,
        reason: String,
    ): javax.crypto.Cipher?
}
