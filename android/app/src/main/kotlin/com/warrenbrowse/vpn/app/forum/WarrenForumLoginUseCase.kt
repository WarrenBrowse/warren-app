package com.warrenbrowse.vpn.app.forum

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.jni.WarrenJni
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/** The result of a forum-login attempt, mirroring the desktop's three outcomes. */
sealed interface WarrenForumLoginOutcome {
    /** The provider accepted the signature; the browser completes the login. */
    data object Approved : WarrenForumLoginOutcome

    /** The wallet has never subscribed to Warren; forum access is refused. */
    data object SubscriptionRequired : WarrenForumLoginOutcome

    /** No wallet on device: the user must set one up before signing in. */
    data object WalletNotReady : WarrenForumLoginOutcome

    /** Any other failure (bad request, provider error, transport error). */
    data class Failure(val reason: String) : WarrenForumLoginOutcome
}

/**
 * Signs a community-forum login challenge and submits it (warren-core doc 55).
 *
 * Mirrors [WarrenSubscriptionUseCase]: require a wallet, read the mnemonic
 * silently (routine signing, no prompt), then hand it to `WarrenJni.forumLogin`,
 * which signs AND POSTs `/v1/forum/login` inside Rust (the wallet signature
 * never surfaces here). Only the opaque `sid` and the allowlisted `host` are
 * passed across. Must be called only after the user approves the consent prompt.
 */
class WarrenForumLoginUseCase(private val walletRepository: WalletRepository) {

    suspend fun signIn(link: ForumLoginLink): WarrenForumLoginOutcome {
        if (walletRepository.state.value is WalletState.Absent) {
            Logger.w("WarrenForumLoginUseCase: no wallet on device")
            return WarrenForumLoginOutcome.WalletNotReady
        }

        val mnemonic = try {
            walletRepository.readMnemonic()
        } catch (e: Exception) {
            Logger.e(throwable = e) { "WarrenForumLoginUseCase: mnemonic read failed" }
            return WarrenForumLoginOutcome.Failure(e.message ?: "wallet read failed")
        }

        return withContext(Dispatchers.IO) {
            mnemonic.use { m ->
                val rawJson = try {
                    WarrenJni.forumLogin(m.phrase, link.sid, link.host)
                } catch (e: Exception) {
                    Logger.e(throwable = e) { "WarrenJni.forumLogin threw" }
                    return@use WarrenForumLoginOutcome.Failure(e.message ?: "JNI forumLogin threw")
                }
                parseForumLoginOutcome(rawJson)
            }
        }
    }
}

/**
 * Map the `{"ok":..}` JNI envelope to an outcome. Pure (no JNI, no I/O) so it is
 * unit-testable off-device. Never surfaces the raw error string to the user.
 */
internal fun parseForumLoginOutcome(rawJson: String): WarrenForumLoginOutcome =
    try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        if (root["ok"]?.jsonPrimitive?.boolean == true) {
            WarrenForumLoginOutcome.Approved
        } else {
            when (root["error"]?.jsonPrimitive?.content) {
                "subscription-required" -> WarrenForumLoginOutcome.SubscriptionRequired
                else -> WarrenForumLoginOutcome.Failure("sign-in failed")
            }
        }
    } catch (e: Exception) {
        WarrenForumLoginOutcome.Failure("invalid JNI response: ${e.message}")
    }
