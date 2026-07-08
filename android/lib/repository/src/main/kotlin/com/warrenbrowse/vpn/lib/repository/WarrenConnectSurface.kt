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
/**
 * Typed outcome of a Warren connect attempt, so the UI can react to a TOFU
 * exit-key mismatch with a warning dialog instead of swallowing a string.
 */
sealed interface WarrenConnectResult {
    /** Connect intent dispatched (or the wallet is busy); nothing to surface. */
    data object Dispatched : WarrenConnectResult
    data object WalletNotReady : WarrenConnectResult
    data object AuthorizationDenied : WarrenConnectResult

    /**
     * The exit advertises a public key different from the one pinned on first
     * use (TOFU). The tunnel was NOT dispatched; the user must explicitly trust
     * the new key (operator rotation) or reject it.
     *
     * NOTE: the compared key comes from the signed relay directory, not the live
     * QUIC handshake, so this is a directory-level advisory (catches operator
     * rotations / a tampered directory), not handshake-time pinning.
     */
    data class ExitKeyMismatch(
        val exitId: String,
        val pinnedPubkeyHex: String,
        val observedPubkeyHex: String,
    ) : WarrenConnectResult

    /** Build/IO failure; [message] is loggable + may be shown inline. */
    data class Failure(val message: String) : WarrenConnectResult
}

interface WarrenQuinnConnectInvoker {
    /**
     * Authenticate, build config, stash mnemonic, dispatch the Quinn connect
     * intent. Returns a typed [WarrenConnectResult] so the caller can raise the
     * pubkey-mismatch warning instead of silently dropping the outcome.
     */
    suspend fun connect(activity: FragmentActivity): WarrenConnectResult
}

/**
 * Lossless, typed projection of the live Warren tunnel state, exposed to
 * lib modules without importing the app-private `WarrenTunnelState` type.
 *
 * Unlike the `String` projection on [WarrenTunnelStateProvider.state] (kept
 * for legacy banner display), this carries the structured fields the main
 * connection card needs - the real endpoint hosts, the feature flags, and a
 * distinct [Failed] vs [Blocking] (kill-switch) status - so the card can show
 * the true state instead of collapsing everything to "Disconnected".
 */
sealed interface WarrenConnectedInfo {
    data object Disconnected : WarrenConnectedInfo
    data object Connecting : WarrenConnectedInfo
    data object Reconnecting : WarrenConnectedInfo

    /**
     * Tunnel up. [exitEndpointHost] / [entryEndpointHost] are "host:port"
     * literals from the active `WarrenTunnelConfig` ([entryEndpointHost] is
     * null for single-hop).
     */
    data class Connected(
        val exitEndpointHost: String,
        val entryEndpointHost: String?,
        val multiHop: Boolean,
        val daita: Boolean,
        val assignedNatPmpPort: Int?,
    ) : WarrenConnectedInfo

    /**
     * Tunnel failed (no kill switch). Surfaced as a non-blocking error.
     * [expired] is set when the exit refused the account (lapsed / revoked
     * subscription), so the card shows a "subscription expired" error.
     */
    data class Failed(val reason: String, val expired: Boolean = false) : WarrenConnectedInfo

    /**
     * Tunnel down but the kill switch (lockdown) is keeping a blocking
     * interface in place, so traffic is blocked rather than leaking.
     * Surfaced as a blocking error ("BLOCKED CONNECTION").
     *
     * [flapping] is set when the block is the result of the reconnect loop
     * giving up after too many drops in a short window (network flapping):
     * the kill switch stays up but the loop stopped, so the error reads as a
     * flap rather than a generic firewall block.
     *
     * [expired] is set when the block is the result of the exit refusing
     * the account (lapsed / revoked subscription): the kill switch stays up
     * but the error reads as "subscription expired" rather than a flap.
     */
    data class Blocking(
        val reason: String,
        val flapping: Boolean = false,
        val expired: Boolean = false,
    ) : WarrenConnectedInfo
}

/**
 * Lib-side surface for the live Warren tunnel state. The concrete
 * impl is `app/service/WarrenQuinnStateProxy` which mirrors the
 * service-owned [com.warrenbrowse.vpn.app.service.WarrenQuinnAdapter.state]
 * into a process-wide StateFlow.
 *
 * [state] is a legacy `String` projection (used by notification/banner
 * plumbing that only needs a label). [connectedInfo] is the lossless typed
 * projection that drives the main connection card.
 */
interface WarrenTunnelStateProvider {
    val state: StateFlow<String>
    val connectedInfo: StateFlow<WarrenConnectedInfo>
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
 * Lib-side surface for the host-offline verdict: true while the device has
 * no usable network path. Independent of the tunnel state on purpose: the
 * native session holds Connected through its transparent redial window, so
 * without this flag the user would see a green "Connected" with no working
 * network. The concrete impl is `app/connectivity/WarrenConnectivityMonitor`;
 * the flow is already debounced on the rising edge so consumers can render
 * it directly without flashing on routine network handovers.
 */
interface WarrenHostOfflineProvider {
    val hostOffline: StateFlow<Boolean>
}

/**
 * Lib-side surface for the automatic-recovery counter (redials or retry
 * loops that reconnected the tunnel without user action, since process
 * start). The concrete impl is `app/service/WarrenQuinnStateProxy`, fed by
 * the adapter which sums the native redial count and its own retry-loop
 * accounting. Drives the "Reconnections" connection-details row.
 */
interface WarrenAutoRecoveryProvider {
    val autoRecoveryCount: StateFlow<Int>
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
 * Lib-side surface for submitting a problem report. The concrete
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
    /** Network / server / signature error - `message` is loggable, not user-facing. */
    data class Failure(val message: String) : WarrenSubscriptionOutcome
}

/**
 * Lib-side surface for fetching the wallet's subscription status. The
 * concrete impl lives in `app/connect/WarrenSubscriptionUseCase` and is
 * bound to this interface in `di/AppModule`. Activity-coupled because it
 * raises a biometric prompt to unlock the mnemonic that signs the request.
 */
/**
 * Outcome of redeeming a voucher. [Success.expiresAtUnixSecs] is the new
 * subscription expiry returned by the server.
 */
sealed interface WarrenVoucherOutcome {
    data class Success(val expiresAtUnixSecs: Long) : WarrenVoucherOutcome
    data object AuthorizationDenied : WarrenVoucherOutcome
    data object WalletNotReady : WarrenVoucherOutcome
    data class Failure(val message: String) : WarrenVoucherOutcome
}

interface WarrenSubscriptionInvoker {
    suspend fun fetch(activity: FragmentActivity): WarrenSubscriptionOutcome

    /** Redeem a Crockford-32 voucher and bind it to the wallet. */
    suspend fun redeemVoucher(activity: FragmentActivity, voucher: String): WarrenVoucherOutcome

    /**
     * Auto-credit an app-initiated purchase (warren-core doc 35): unlock once,
     * then poll the signed redeem of the 32-hex purchase id [wpid] until the
     * payment is credited or [deadlineMs] elapses. Fire-and-forget: it runs in
     * an app-scoped coroutine that survives the browser round-trip and screen
     * navigation, and on success writes the credited expiry to
     * [WarrenLocalSettingsRepository.cachedSubscriptionExpiry] so the whole app
     * refreshes reactively.
     */
    fun startPurchasePoll(
        activity: FragmentActivity,
        wpid: String,
        intervalMs: Long = 5_000,
        deadlineMs: Long = 10 * 60 * 1000,
    )
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
