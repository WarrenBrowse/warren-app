package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import androidx.fragment.app.FragmentActivity
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.repository.MnemonicCache
import com.warrenbrowse.vpn.app.service.WarrenTunnelConfig
import com.warrenbrowse.vpn.app.service.WarrenVpnService
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnConnectInvoker
import com.warrenbrowse.vpn.lib.common.constant.KEY_WARREN_CONNECT_QUINN_ACTION
import com.warrenbrowse.vpn.lib.common.constant.KEY_WARREN_TUNNEL_CONFIG_JSON
import com.warrenbrowse.vpn.lib.model.wallet.WalletAddress
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.ConnectionProxy
import com.warrenbrowse.vpn.lib.repository.ExitKeyVerdict
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectResult
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.app.service.toWireJson
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * End-to-end Warren connect orchestrator.
 *
 * Flow:
 *   1. Require a wallet on device (Locked or Ready); refuse only when Absent
 *      (UI should route the user to onboarding instead of calling us).
 *   2. Resolve the mnemonic via [WalletRepository.readMnemonic] (silent: a
 *      routine signing op shows no biometric/PIN prompt).
 *   3. Build the [com.warrenbrowse.vpn.app.service.WarrenTunnelConfig]
 *      from current settings (today: hardcoded warren-exit-1).
 *   4. Stash the mnemonic in [MnemonicCache] and start [WarrenVpnService]
 *      with `KEY_WARREN_CONNECT_QUINN_ACTION` + the JSON-encoded config.
 */
class WarrenConnectUseCase(
    private val walletRepository: WalletRepository,
    private val configBuilder: WarrenTunnelConfigBuilder,
    private val localSettings: WarrenLocalSettingsRepository,
    private val connectionProxy: ConnectionProxy,
) : WarrenQuinnConnectInvoker {

    sealed interface Outcome {
        data object Success : Outcome
        data object WalletNotReady : Outcome
        data object AuthorizationDenied : Outcome
        /** The exit presented a key different from the pinned one (TOFU). */
        data class ExitKeyMismatch(
            val exitId: String,
            val pinnedPubkeyHex: String,
            val observedPubkeyHex: String,
        ) : Outcome
        data class Failure(val message: String) : Outcome
    }

    /**
     * Result of building a [com.warrenbrowse.vpn.app.service.WarrenTunnelConfig]
     * from the current settings. Needs only the public wallet pubkey, so it
     * never reads the mnemonic: callers that already hold a live session (the
     * reconnect path) can rebuild the config with the latest settings without a
     * wallet read or a biometric prompt.
     */
    sealed interface ConfigResult {
        data class Ready(
            val config: com.warrenbrowse.vpn.app.service.WarrenTunnelConfig,
        ) : ConfigResult
        data object WalletNotReady : ConfigResult
        data class ExitKeyMismatch(
            val exitId: String,
            val pinnedPubkeyHex: String,
            val observedPubkeyHex: String,
        ) : ConfigResult
        data class Failure(val message: String) : ConfigResult
    }

    /**
     * Build a fresh tunnel config from the current settings + relay catalogue,
     * running the trust-on-first-use exit-key check (fail closed on a mismatch).
     * Pure with respect to the wallet secret: only the public pubkey is read.
     */
    fun buildFreshConfig(): ConfigResult {
        val pubkey = walletPubkey() ?: return ConfigResult.WalletNotReady

        val built = configBuilder.build(pubkey) ?: run {
            Logger.e("WarrenConnectUseCase: relay catalogue empty, no exit to connect to")
            return ConfigResult.Failure("No relay available")
        }

        // Trust-on-first-use exit key check (fail closed). A mismatch means the
        // exit's key changed since we last pinned it; refuse so the user can
        // decide (reset pins in settings to accept a rotation).
        val exitId = built.exitId
        if (exitId != null) {
            val verdict = checkExitKey(exitId, built.exitPubkeyHex)
            if (verdict is ExitKeyVerdict.Mismatch) {
                return ConfigResult.ExitKeyMismatch(
                    exitId = exitId,
                    pinnedPubkeyHex = verdict.pinned,
                    observedPubkeyHex = built.exitPubkeyHex,
                )
            }
        }

        return ConfigResult.Ready(withLocalToggles(built))
    }

    /**
     * The config an automatic retry dials after [previous] dropped: the same
     * session on an alternative exit the pin allows, or `null` when there is
     * none (the retry then redials [previous]). Desktop
     * `assemble_failover_for_attempt`, with the exit-key pin enforced the same
     * way as on a fresh connect: an alternative whose key changed since it was
     * pinned is refused rather than dialled, because the retry runs with no
     * user to decide. No request leaves here: the retry runs behind the
     * kill-switch blackhole, and the builder resolves the alternative from the
     * catalogue already in memory.
     */
    fun buildFailoverConfig(previous: WarrenTunnelConfig): WarrenTunnelConfig? {
        val alternative =
            configBuilder.buildFailover(previous)?.takeUnless { built ->
                val verdict = built.exitId?.let { checkExitKey(it, built.exitPubkeyHex) }
                verdict is ExitKeyVerdict.Mismatch
            }
        return alternative?.let(::withLocalToggles)
    }

    private fun walletPubkey(): WalletAddress? =
        when (val state = walletRepository.state.value) {
            is WalletState.Ready -> state.pubkey
            is WalletState.Locked -> state.pubkey
            WalletState.Absent -> {
                Logger.w("WarrenConnectUseCase: no wallet on device")
                null
            }
        }

    /** The pin verdict for [exitId], pinning a first sighting on the way. */
    private fun checkExitKey(exitId: String, pubkeyHex: String): ExitKeyVerdict {
        val verdict = localSettings.exitKeyVerdict(exitId, pubkeyHex)
        when (verdict) {
            is ExitKeyVerdict.Mismatch ->
                Logger.w("WarrenConnectUseCase: exit key mismatch for $exitId, refusing")
            ExitKeyVerdict.FirstSeen -> localSettings.trustExitKey(exitId, pubkeyHex)
            ExitKeyVerdict.Match -> Unit
        }
        return verdict
    }

    // Apply the local-network-sharing + MTU toggles here so they are honoured
    // regardless of the config builder (the natural place, kept minimal).
    // Both are serialized fields, so they survive the JSON round-trip through
    // the VpnService Intent down to the TUN plan.
    private fun withLocalToggles(built: WarrenTunnelConfig): WarrenTunnelConfig =
        built.copy(
            allowLan = localSettings.allowLan.value,
            mtu = localSettings.tunnelMtu.value,
        )

    /**
     * `WarrenQuinnConnectInvoker` impl: maps the app-private [Outcome] to the
     * lib-side [WarrenConnectResult] so UI layers in `lib/feature/<x>` (which
     * cannot import [Outcome]) can raise the pubkey-mismatch dialog.
     */
    override suspend fun connect(activity: FragmentActivity): WarrenConnectResult =
        when (val outcome = invoke(activity)) {
            Outcome.Success -> WarrenConnectResult.Dispatched
            Outcome.WalletNotReady -> WarrenConnectResult.WalletNotReady
            Outcome.AuthorizationDenied -> WarrenConnectResult.AuthorizationDenied
            is Outcome.ExitKeyMismatch ->
                WarrenConnectResult.ExitKeyMismatch(
                    exitId = outcome.exitId,
                    pinnedPubkeyHex = outcome.pinnedPubkeyHex,
                    observedPubkeyHex = outcome.observedPubkeyHex,
                )
            is Outcome.Failure -> WarrenConnectResult.Failure(outcome.message)
        }

    suspend fun invoke(context: Context): Outcome {
        // Everything below (config build, relay fetch, wallet read) runs before
        // the service is even started, so the card must be told the dial began
        // now rather than a network round trip later. Any outcome other than a
        // dispatched intent retracts it.
        connectionProxy.onConnectRequested(
            multiHop = localSettings.multiHopEnabled.value,
            daita = localSettings.daitaEnabled.value,
        )
        val outcome = dispatch(context)
        if (outcome !is Outcome.Success) connectionProxy.onRequestAborted()
        return outcome
    }

    private suspend fun dispatch(context: Context): Outcome {
        // Build the config from current settings first (public pubkey only, no
        // secret). This also runs the trust-on-first-use exit-key check and maps
        // its outcomes back to the connect [Outcome] surface.
        //
        // Off the main thread: the rebuild fetches the signed relay catalogue
        // and the multi-hop directory, two blocking network round trips, and
        // every caller reaches us from viewModelScope (Dispatchers.Main).
        // WarrenVpnService's reconnect path guards the same call the same way.
        val config = when (val result = withContext(Dispatchers.IO) { buildFreshConfig() }) {
            is ConfigResult.Ready -> result.config
            ConfigResult.WalletNotReady -> return Outcome.WalletNotReady
            is ConfigResult.ExitKeyMismatch -> return Outcome.ExitKeyMismatch(
                exitId = result.exitId,
                pinnedPubkeyHex = result.pinnedPubkeyHex,
                observedPubkeyHex = result.observedPubkeyHex,
            )
            is ConfigResult.Failure -> return Outcome.Failure(result.message)
        }

        // Routine signing: read the mnemonic silently (no biometric/PIN prompt).
        // The key is app-bound, the secret is never shown, and this matches the
        // desktop daemon that holds the key in memory. The prompt is reserved
        // for revealing the recovery phrase on screen.
        val mnemonic = try {
            walletRepository.readMnemonic()
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenConnectUseCase: mnemonic read failed" }
            return Outcome.Failure(e.message ?: "wallet read failed")
        }

        val configJson = config.toWireJson()

        MnemonicCache.put(mnemonic)
        val appContext = context.applicationContext
        val intent = Intent(appContext, WarrenVpnService::class.java).apply {
            action = KEY_WARREN_CONNECT_QUINN_ACTION
            putExtra(KEY_WARREN_TUNNEL_CONFIG_JSON, configJson)
        }
        appContext.startForegroundService(intent)
        Logger.i("WarrenConnectUseCase: dispatched Quinn connect intent")
        return Outcome.Success
    }
}
