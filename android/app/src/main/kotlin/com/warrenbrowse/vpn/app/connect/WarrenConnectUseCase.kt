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
import com.warrenbrowse.vpn.lib.repository.WalletAuthorizationDeniedException
import com.warrenbrowse.vpn.lib.repository.ExitKeyVerdict
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.app.service.toWireJson
import com.warrenbrowse.vpn.lib.ui.component.wallet.BiometricPromptAuthorizer

/**
 * End-to-end Warren connect orchestrator.
 *
 * Flow:
 *   1. Require wallet in [WalletState.Ready]; refuse otherwise (UI should
 *      route the user to the wallet onboarding instead of calling us).
 *   2. Resolve the mnemonic via [WalletRepository.unlock] gated by a
 *      [BiometricPromptAuthorizer] bound to the supplied [FragmentActivity].
 *   3. Build the [com.warrenbrowse.vpn.app.service.WarrenTunnelConfig]
 *      from current settings (today: hardcoded warren-exit-1).
 *   4. Stash the mnemonic in [MnemonicCache] and start [WarrenVpnService]
 *      with `KEY_WARREN_CONNECT_QUINN_ACTION` + the JSON-encoded config.
 *
 * The use-case is intentionally Activity-coupled (takes a FragmentActivity
 * rather than a Context) because [BiometricPromptAuthorizer] needs a
 * fragment host. Callers from a Composable retrieve the activity via
 * `LocalContext.current as FragmentActivity`.
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
        data object ExitKeyMismatch : Outcome
        data class Failure(val message: String) : Outcome
    }

    /**
     * `WarrenQuinnConnectInvoker` impl: returns a human-readable status
     * string so UI layers in `lib/feature/<x>` modules (which cannot import
     * the app-private [Outcome] sealed type) can render result inline.
     */
    override suspend fun connect(activity: FragmentActivity): String =
        when (val outcome = invoke(activity)) {
            Outcome.Success -> "Quinn connect dispatched"
            Outcome.WalletNotReady -> "Wallet not ready"
            Outcome.AuthorizationDenied -> "Biometric authentication denied"
            Outcome.ExitKeyMismatch ->
                "Exit key changed since last use. If this is expected, reset " +
                    "pinned exit keys in Tunnel settings, then reconnect."
            is Outcome.Failure -> outcome.message
        }

    suspend fun invoke(activity: FragmentActivity): Outcome {
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

        val authorizer = BiometricPromptAuthorizer(activity)
        val mnemonic = try {
            walletRepository.unlock(
                authorizer = authorizer,
                reason = "Connect to Warren VPN",
            )
        } catch (e: WalletAuthorizationDeniedException) {
            return Outcome.AuthorizationDenied
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenConnectUseCase: unlock failed" }
            return Outcome.Failure(e.message ?: "wallet unlock failed")
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
            when (localSettings.exitKeyVerdict(exitId, built.exitPubkeyHex)) {
                is ExitKeyVerdict.Mismatch -> {
                    Logger.w("WarrenConnectUseCase: exit key mismatch for $exitId, refusing")
                    return Outcome.ExitKeyMismatch
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
        val intent = Intent(activity.applicationContext, WarrenVpnService::class.java).apply {
            action = KEY_WARREN_CONNECT_QUINN_ACTION
            putExtra(KEY_WARREN_TUNNEL_CONFIG_JSON, configJson)
        }
        val context: Context = activity.applicationContext
        context.startForegroundService(intent)
        Logger.i("WarrenConnectUseCase: dispatched Quinn connect intent")
        return Outcome.Success
    }
}
