package com.warrenbrowse.vpn.lib.repository

import androidx.fragment.app.FragmentActivity
import kotlinx.coroutines.flow.StateFlow

/**
 * Lib-side surface for the Warren Connect orchestrator. The concrete
 * implementation lives in `app/connect/WarrenConnectUseCase` and is
 * bound to this interface in `di/AppModule`. The interface lives here
 * (in `lib/repository`, alongside [WalletRepository]) so any feature
 * module - `lib/feature/home/impl`, `lib/feature/settings/impl`, etc. -
 * can consume the surface without depending on the `app` module
 * (forbidden dependency arrow).
 */
interface WarrenQuinnConnectInvoker {
    /**
     * Authenticate, build config, stash mnemonic, dispatch the Quinn
     * connect intent. Returns a human-readable status string suitable
     * for inline display.
     */
    suspend fun connect(activity: FragmentActivity): String
}

/**
 * Lib-side surface for the live Warren tunnel state. The concrete
 * impl is `app/service/WarrenQuinnStateProxy` which mirrors the
 * service-owned [com.warrenbrowse.vpn.app.service.WarrenQuinnAdapter.state]
 * into a process-wide StateFlow. Consumers in feature modules subscribe
 * here (with a `String` projection so they don't need to import the
 * app-private `WarrenTunnelState` sealed type).
 */
interface WarrenTunnelStateProvider {
    val state: StateFlow<String>
}

/**
 * Lib-side surface for the live NAT-PMP port-forwarding status. The
 * concrete impl is `app/service/WarrenQuinnStateProxy`, fed by the
 * adapter which polls `WarrenJni.getNatPmpStatus()`. Consumers receive a
 * raw JSON `String` projection (parsed UI-side). `{"state":"idle"}` when
 * port forwarding is off or no mapping is active.
 */
interface WarrenNatPmpStatusProvider {
    val natPmpStatus: StateFlow<String>
}

/**
 * Lib-side surface for the Warren disconnect path. The concrete impl
 * lives in `app/connect/WarrenDisconnectUseCase` and is bound to this
 * interface in `di/AppModule`. The disconnect path does not need
 * biometric authorisation (it tears down a running session); a plain
 * [android.content.Context] is sufficient because no UI dialog is
 * raised.
 */
interface WarrenQuinnDisconnectInvoker {
    fun disconnect()
}

/**
 * Lib-side surface for the Warren reconnect path. Reuses the cached
 * config + mnemonic from the running session (no biometric re-prompt).
 * No-op if there is no active session. Implementation in
 * `app/connect/WarrenReconnectUseCase`.
 */
interface WarrenQuinnReconnectInvoker {
    fun reconnect()
}

/**
 * Wire shape for a relay entry exposed by [WarrenRelayProvider]. Lives
 * here so feature modules can render the list without depending on the
 * app module. The schema mirrors the JSON shape returned by
 * `WarrenJni.listRelays`.
 */
data class WarrenRelaySummary(
    val exitId: String,
    val exitPubkeyHex: String,
    val endpoint: String,
    val country: String,
    val city: String,
    val active: Boolean,
    val weight: Long,
)

/**
 * Lib-side surface for the Warren relay catalogue. The concrete impl
 * lives in `app/connect/RelayCatalog` and is bound to this interface in
 * `di/AppModule`. The location picker UI in `lib/feature/settings/impl`
 * consumes this surface to render the available relays.
 */
interface WarrenRelayProvider {
    /** Snapshot of the available relays. Empty list = catalogue unreachable. */
    fun list(): List<WarrenRelaySummary>
}

/**
 * Outcome of a Warren support-report submission. Returned to the
 * feature-module UI so it can render the right success / failure
 * state without depending on the app-private use-case type.
 */
sealed interface WarrenSupportReportOutcome {
    /** Server accepted the report. `referenceId` is a 32-hex tracker id. */
    data class Success(val referenceId: String) : WarrenSupportReportOutcome
    /** Wallet not unlocked or user cancelled the biometric prompt. */
    data object AuthorizationDenied : WarrenSupportReportOutcome
    /** Wallet has no mnemonic yet (the onboarding hasn't run). */
    data object WalletNotReady : WarrenSupportReportOutcome
    /** Network / server / signature error - `message` is loggable, not user-facing. */
    data class Failure(val message: String) : WarrenSupportReportOutcome
}

/**
 * Lib-side surface for submitting a D.6 problem report. The concrete
 * impl lives in `app/connect/WarrenSendProblemReportUseCase` and is
 * bound to this interface in `di/AppModule`. The submission flow is
 * Activity-coupled (mirror of [WarrenQuinnConnectInvoker]) because it
 * needs to raise a biometric prompt to unlock the BIP39 mnemonic
 * that signs the request.
 */
/**
 * Outcome of a subscription-status fetch. Returned to the feature-module
 * UI so it can render expiry without depending on the app-private use case.
 */
sealed interface WarrenSubscriptionOutcome {
    /** Server returned an expiry. [expiresAtUnixSecs] is Unix epoch seconds. */
    data class Success(val expiresAtUnixSecs: Long) : WarrenSubscriptionOutcome
    /** Wallet not unlocked or the user cancelled the biometric prompt. */
    data object AuthorizationDenied : WarrenSubscriptionOutcome
    /** Wallet has no mnemonic yet (onboarding hasn't run). */
    data object WalletNotReady : WarrenSubscriptionOutcome
    /** Network / server / signature error — `message` is loggable, not user-facing. */
    data class Failure(val message: String) : WarrenSubscriptionOutcome
}

/**
 * Lib-side surface for fetching the wallet's subscription status. The
 * concrete impl lives in `app/connect/WarrenSubscriptionUseCase` and is
 * bound to this interface in `di/AppModule`. Activity-coupled because it
 * raises a biometric prompt to unlock the mnemonic that signs the request.
 */
interface WarrenSubscriptionInvoker {
    suspend fun fetch(activity: FragmentActivity): WarrenSubscriptionOutcome
}

interface WarrenSupportReportInvoker {
    /**
     * Sign + ship a support report.
     *
     * @param activity Fragment host for the biometric prompt.
     * @param userMessage Free-form user description.
     * @param redactedLogs Pre-redacted log bundle (empty when the
     *  user unchecked "Include logs").
     */
    suspend fun submit(
        activity: FragmentActivity,
        userMessage: String,
        redactedLogs: String,
    ): WarrenSupportReportOutcome
}
