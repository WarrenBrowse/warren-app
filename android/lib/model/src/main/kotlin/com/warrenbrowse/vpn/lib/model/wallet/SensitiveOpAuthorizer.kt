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
