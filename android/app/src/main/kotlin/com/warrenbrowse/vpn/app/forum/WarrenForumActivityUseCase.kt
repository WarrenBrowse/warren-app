package com.warrenbrowse.vpn.app.forum

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.forum.ForumNotification
import com.warrenbrowse.vpn.lib.model.forum.ForumNotificationKind
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.ForumActivityState
import com.warrenbrowse.vpn.lib.repository.ForumIdentityRepository
import com.warrenbrowse.vpn.lib.repository.ForumNotificationsReader
import com.warrenbrowse.vpn.lib.repository.ForumNotificationsResult
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull

/**
 * The two account-bound forum calls (warren-core doc 55): the panel read and
 * the mark-seen. Both are made only because the user asked to see or clear
 * the content, never on a timer: the badge comes from the broadcast digest,
 * which asks the server nothing about anybody.
 *
 * Same shape as [WarrenForumLoginUseCase]: require a wallet, check the tunnel
 * is in a state the connect host can be resolved in ([ForumPreflight]), read
 * the mnemonic silently, and hand it to the JNI bridge, which signs AND sends
 * inside Rust. What the answer proves about the unread count is pushed into
 * the activity state at once, so the badge is right the moment the user acts
 * rather than at the next poll.
 */
class WarrenForumActivityUseCase(
    private val walletRepository: WalletRepository,
    private val forumIdentityRepository: ForumIdentityRepository,
    private val activity: ForumActivityState,
    private val jni: WarrenJniBridge,
    private val tunnelState: WarrenTunnelStateProvider,
) : ForumNotificationsReader {

    override suspend fun list(): ForumNotificationsResult {
        refusal()?.let {
            return it
        }
        val result =
            signed("forumNotifications") { m -> jni.forumNotifications(m) }
                .fold(::parseForumNotificationsEnvelope) { ForumNotificationsResult.Error(it) }
        if (result is ForumNotificationsResult.Ok) {
            activity.setObservedUnread(result.notifications.count { it.unread })
        }
        return result
    }

    override suspend fun markSeen(): Boolean {
        if (refusal() != null) return false
        // The user has already decided: the badge follows now. A failed write
        // leaves the list marked here while the next digest puts it back,
        // which is the harmless direction.
        activity.setObservedUnread(0)
        return signed("forumNotificationsSeen") { m -> jni.forumNotificationsSeen(m) }
            .fold(::parseSeenEnvelope) { false }
    }

    /** Why a call must not leave now, or null when it may. */
    private fun refusal(): ForumNotificationsResult.Error? {
        val preflight = ForumPreflight.of(tunnelState.connectedInfo.value)
        val reason =
            when {
                walletRepository.state.value is WalletState.Absent -> "wallet-absent"
                forumIdentityRepository.identity.value == null -> "no-forum-account"
                preflight is ForumPreflight.Defer -> {
                    Logger.w("WarrenForumActivityUseCase: deferred, tunnel ${preflight.tunnelClass}")
                    "deferred-${preflight.tunnelClass}"
                }
                else -> null
            }
        return reason?.let(ForumNotificationsResult::Error)
    }

    /**
     * The raw envelope of one signed call, or the class of what stopped it.
     * The keystore read and the JNI call are system boundaries: whatever they
     * throw is classed for the log and the journal, never propagated to a
     * screen.
     */
    @Suppress("TooGenericExceptionCaught")
    private suspend fun signed(flow: String, call: (String) -> String): Result<String> {
        val mnemonic =
            try {
                walletRepository.readMnemonic()
            } catch (e: Exception) {
                Logger.e(throwable = e) { "$flow: mnemonic read failed" }
                return Result.failure(IllegalStateException("wallet-read"))
            }
        return withContext(Dispatchers.IO) {
            mnemonic.use { m ->
                try {
                    Result.success(call(m.phrase))
                } catch (e: Exception) {
                    Logger.e(throwable = e) { "WarrenJniBridge.$flow threw" }
                    Result.failure(IllegalStateException("jni"))
                }
            }
        }
    }

    private inline fun <T> Result<String>.fold(ok: (String) -> T, failed: (String) -> T): T =
        getOrNull()?.let(ok) ?: failed(exceptionOrNull()?.message ?: "unknown")
}

/**
 * Map the `{"ok":..}` JNI envelope of the panel read to rows. The rows were
 * validated in the shared crate (kinds, pinned paths, bounded text), so this
 * only reads them back; a malformed envelope is an error, never a panel that
 * silently shows less than it could. Pure (no JNI, no I/O) so it is
 * unit-testable off-device.
 */
internal fun parseForumNotificationsEnvelope(rawJson: String): ForumNotificationsResult =
    try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        if (root["ok"]?.jsonPrimitive?.booleanOrNull == true) {
            val rows =
                root["notifications"]?.jsonArray.orEmpty().mapNotNull { element ->
                    val row = element.jsonObject
                    val id = row["id"]?.jsonPrimitive?.longOrNull ?: return@mapNotNull null
                    val createdAt =
                        row["created_at"]?.jsonPrimitive?.longOrNull ?: return@mapNotNull null
                    ForumNotification(
                        id = id,
                        kind = ForumNotificationKind.fromToken(row["kind"]?.jsonPrimitive?.contentOrNull),
                        unread = row["unread"]?.jsonPrimitive?.booleanOrNull == true,
                        createdAt = createdAt,
                        title = row["title"]?.jsonPrimitive?.contentOrNull,
                        actor = row["actor"]?.jsonPrimitive?.contentOrNull,
                        excerpt = row["excerpt"]?.jsonPrimitive?.contentOrNull,
                        path = row["path"]?.jsonPrimitive?.contentOrNull,
                    )
                }
            ForumNotificationsResult.Ok(rows)
        } else {
            ForumNotificationsResult.Error(
                root["reason"]?.jsonPrimitive?.contentOrNull?.takeIf { it.isNotBlank() } ?: "unknown"
            )
        }
    } catch (e: IllegalArgumentException) {
        ForumNotificationsResult.Error("invalid-envelope")
    }

/** True iff the mark-seen envelope says the write landed. */
internal fun parseSeenEnvelope(rawJson: String): Boolean =
    try {
        Json.parseToJsonElement(rawJson).jsonObject["ok"]?.jsonPrimitive?.booleanOrNull == true
    } catch (e: IllegalArgumentException) {
        false
    }
