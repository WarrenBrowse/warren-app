package com.warrenbrowse.vpn.app.forum

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.ForumIdentityRepository
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/** The result of a forum-login attempt, mirroring the desktop's outcomes. */
sealed interface WarrenForumLoginOutcome {
    /**
     * The provider accepted the signature; the browser completes the login.
     * Carries the forum identity when the provider handed one back.
     */
    data class Approved(val identity: ForumIdentity?) : WarrenForumLoginOutcome

    /** The wallet has never subscribed to Warren; forum access is refused. */
    data object SubscriptionRequired : WarrenForumLoginOutcome

    /**
     * The provider refused the signature because this device's clock is off by
     * more than its accepted window. The one failure the user repairs
     * themselves, so it must not collapse into the generic message.
     */
    data object ClockSkew : WarrenForumLoginOutcome

    /** The browser session is gone: expired, cancelled or already consumed. */
    data object Expired : WarrenForumLoginOutcome

    /** No wallet on device: the user must set one up before signing in. */
    data object WalletNotReady : WarrenForumLoginOutcome

    /**
     * Not attempted: the tunnel is between states ([ForumPreflight]), so the
     * connect host could not be resolved. Nothing was signed and the session
     * is untouched; the prompt stays open for a retry once the tunnel is
     * connected or off.
     */
    data class Deferred(val tunnelClass: String) : WarrenForumLoginOutcome

    /**
     * Any other failure, with its class (`transport`, `build`, `runtime`,
     * `http-<status>`, `wallet-read`, `jni`). Rendered generically; the class
     * goes to the log and the events journal, where a report can carry it.
     */
    data class Failure(val reason: String) : WarrenForumLoginOutcome
}

/**
 * Signs a community-forum login challenge and submits it (warren-core doc 55).
 *
 * Mirrors [WarrenSubscriptionUseCase]: require a wallet, check the tunnel is
 * in a state the connect host can be resolved in ([ForumPreflight]), read the
 * mnemonic silently (routine signing, no prompt), then hand it to
 * [WarrenJniBridge.forumLogin], which preflights the session, signs AND POSTs
 * `/v1/forum/login` inside Rust (the wallet signature never surfaces here).
 * Only the opaque `sid` and the allowlisted `host` are passed across. Must be
 * called only after the user approves the consent prompt. An approved login
 * records the identity the provider returned, which is what the account page
 * and the activity badge read.
 */
class WarrenForumLoginUseCase(
    private val walletRepository: WalletRepository,
    private val forumIdentityRepository: ForumIdentityRepository,
    private val journal: ForumEventsJournal,
    private val jni: WarrenJniBridge,
    private val tunnelState: WarrenTunnelStateProvider,
) {

    // Fire-and-forget scope for the cancel notify so it survives the consent
    // prompt being dismissed (a composition scope would cancel it mid-flight).
    private val cancelScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    suspend fun signIn(link: ForumLoginLink): WarrenForumLoginOutcome {
        if (walletRepository.state.value is WalletState.Absent) {
            Logger.w("WarrenForumLoginUseCase: no wallet on device")
            journal.record("login.result", "class" to "wallet-absent")
            return WarrenForumLoginOutcome.WalletNotReady
        }

        // Before the mnemonic is read: a deferred attempt must touch nothing.
        val preflight = ForumPreflight.of(tunnelState.connectedInfo.value)
        if (preflight is ForumPreflight.Defer) {
            Logger.w("WarrenForumLoginUseCase: deferred, tunnel ${preflight.tunnelClass}")
            journal.record("login.deferred", "class" to preflight.tunnelClass)
            return WarrenForumLoginOutcome.Deferred(preflight.tunnelClass)
        }

        val mnemonic = try {
            walletRepository.readMnemonic()
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenForumLoginUseCase: mnemonic read failed" }
            journal.record("login.result", "class" to "wallet-read")
            return WarrenForumLoginOutcome.Failure("wallet-read")
        }

        val started = System.currentTimeMillis()
        journal.record("login.signing", "cross_device" to link.crossDevice.toString())
        return withContext(Dispatchers.IO) {
            mnemonic.use { m ->
                val rawJson = try {
                    jni.forumLogin(m.phrase, link.sid, link.host)
                } catch (e: Exception) {
                    Logger.e(throwable = e) { "WarrenJniBridge.forumLogin threw" }
                    journal.record("login.result", "class" to "jni")
                    return@use WarrenForumLoginOutcome.Failure("jni")
                }
                val outcome = parseForumLoginOutcome(rawJson)
                journal.record(
                    "login.result",
                    "class" to outcomeClass(outcome),
                    "elapsed_ms" to (System.currentTimeMillis() - started).toString(),
                )
                if (outcome is WarrenForumLoginOutcome.Approved) {
                    outcome.identity?.let(forumIdentityRepository::save)
                } else {
                    Logger.w("WarrenForumLoginUseCase: sign-in not approved: ${outcomeClass(outcome)}")
                }
                outcome
            }
        }
    }

    /**
     * Best-effort: notify the provider the user declined so the waiting browser
     * page unblocks. Fire-and-forget on [cancelScope] so it is not cancelled
     * when the consent prompt leaves composition; nothing to report back.
     */
    fun cancel(link: ForumLoginLink) {
        journal.record("login.declined")
        cancelScope.launch {
            try {
                jni.forumLoginCancel(link.sid, link.host)
            } catch (e: Exception) {
                Logger.w(throwable = e) { "WarrenJniBridge.forumLoginCancel threw" }
            }
        }
    }
}

/** The coarse class of an outcome, for the log and the journal. */
internal fun outcomeClass(outcome: WarrenForumLoginOutcome): String =
    when (outcome) {
        is WarrenForumLoginOutcome.Approved ->
            if (outcome.identity != null) "approved-with-identity" else "approved"
        WarrenForumLoginOutcome.SubscriptionRequired -> "subscription-required"
        WarrenForumLoginOutcome.ClockSkew -> "clock-skew"
        WarrenForumLoginOutcome.Expired -> "expired"
        WarrenForumLoginOutcome.WalletNotReady -> "wallet-absent"
        is WarrenForumLoginOutcome.Deferred -> "deferred-${outcome.tunnelClass}"
        is WarrenForumLoginOutcome.Failure -> outcome.reason
    }

/**
 * Map the `{"ok":..}` JNI envelope to an outcome. Pure (no JNI, no I/O) so it is
 * unit-testable off-device. Never surfaces the raw error string to the user:
 * the `reason` of a failure is a fixed class token, kept for the log only.
 */
internal fun parseForumLoginOutcome(rawJson: String): WarrenForumLoginOutcome =
    try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        if (root["ok"]?.jsonPrimitive?.boolean == true) {
            val handle = root["handle"]?.jsonPrimitive?.content
            val slot = root["notify_slot"]?.jsonPrimitive?.int
            WarrenForumLoginOutcome.Approved(
                identity = handle?.let { ForumIdentity(handle = it, notifySlot = slot) }
            )
        } else {
            when (root["error"]?.jsonPrimitive?.content) {
                "subscription-required" -> WarrenForumLoginOutcome.SubscriptionRequired
                "clock-skew" -> WarrenForumLoginOutcome.ClockSkew
                "expired" -> WarrenForumLoginOutcome.Expired
                else ->
                    WarrenForumLoginOutcome.Failure(
                        root["reason"]?.jsonPrimitive?.content?.takeIf { it.isNotBlank() }
                            ?: "unknown"
                    )
            }
        }
    } catch (e: Exception) {
        WarrenForumLoginOutcome.Failure("invalid-envelope")
    }
