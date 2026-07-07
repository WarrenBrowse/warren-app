package com.warrenbrowse.vpn.app.connect

import android.content.Context
import android.content.Intent
import androidx.fragment.app.FragmentActivity
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.repository.MnemonicCache
import com.warrenbrowse.vpn.app.service.WarrenVpnService
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnConnectInvoker
import com.warrenbrowse.vpn.lib.common.constant.KEY_WARREN_CONNECT_QUINN_ACTION
import com.warrenbrowse.vpn.lib.common.constant.KEY_WARREN_TUNNEL_CONFIG_JSON
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.ExitKeyVerdict
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenConnectResult
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.app.service.toWireJson

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
        // The pubkey (public, used to address the relay config) is available
        // whether the wallet is Locked at rest or transiently Ready; only Absent
        // blocks. Gating on Ready alone made Connect a no-op at rest, since the
        // resting state is Locked (Ready only happens right after create/import).
        val pubkey = when (val state = walletRepository.state.value) {
            is WalletState.Ready -> state.pubkey
            is WalletState.Locked -> state.pubkey
            WalletState.Absent -> {
                Logger.w("WarrenConnectUseCase: no wallet on device")
                return Outcome.WalletNotReady
            }
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

        val built = configBuilder.build(pubkey) ?: run {
            Logger.e("WarrenConnectUseCase: relay catalogue empty, no exit to connect to")
            return Outcome.Failure("No relay available")
        }
        // Apply the local-network-sharing toggle here so it is honoured
        // regardless of the config builder (which is the natural place but is
        // kept minimal). allowLan is a serialized field, so it survives the
        // JSON round-trip through the VpnService Intent down to the TUN plan.
        // Trust-on-first-use exit key check (fail closed). A mismatch means
        // the exit's key changed since we last pinned it; refuse to connect so
        // the user can decide (reset pins in settings to accept a rotation).
        built.exitId?.let { exitId ->
            when (val verdict = localSettings.exitKeyVerdict(exitId, built.exitPubkeyHex)) {
                is ExitKeyVerdict.Mismatch -> {
                    Logger.w("WarrenConnectUseCase: exit key mismatch for $exitId, refusing")
                    return Outcome.ExitKeyMismatch(
                        exitId = exitId,
                        pinnedPubkeyHex = verdict.pinned,
                        observedPubkeyHex = built.exitPubkeyHex,
                    )
                }
                ExitKeyVerdict.FirstSeen ->
                    localSettings.trustExitKey(exitId, built.exitPubkeyHex)
                ExitKeyVerdict.Match -> Unit
            }
        }

        val config = built.copy(
            allowLan = localSettings.allowLan.value,
            mtu = localSettings.tunnelMtu.value,
        )
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
