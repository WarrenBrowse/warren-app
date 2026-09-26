package com.warrenbrowse.vpn.app.connect

import com.warrenbrowse.vpn.app.standing.accountBanOf
import com.warrenbrowse.vpn.lib.model.AccountBan
import com.warrenbrowse.vpn.lib.model.AccountStanding
import com.warrenbrowse.vpn.lib.repository.WarrenVoucherOutcome
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long

/**
 * The outcome of `WarrenJni.redeemVoucher`: `{"ok":true,"expires_at":N}`, the
 * ban refusal `{"ok":false,"error":"banned","ban":{..}}`, the verdict on the
 * voucher `{"ok":false,"error":"voucher rejected"}`, or
 * `{"ok":false,"error":"<class>"}`.
 */
@Suppress("TooGenericExceptionCaught")
internal fun parseVoucherJson(rawJson: String): WarrenVoucherOutcome =
    try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        val error = root["error"]?.jsonPrimitive?.contentOrNull
        val ban = root["ban"] as? JsonObject
        when {
            root["ok"]?.jsonPrimitive?.boolean == true ->
                root["expires_at"]?.jsonPrimitive?.long?.let { WarrenVoucherOutcome.Success(it) }
                    ?: WarrenVoucherOutcome.Failure("missing expires_at")
            error == "banned" && ban != null -> WarrenVoucherOutcome.Banned(accountBanOf(ban))
            error == VOUCHER_REJECTED -> WarrenVoucherOutcome.Rejected
            else -> WarrenVoucherOutcome.Failure(error ?: "redeem failed")
        }
    } catch (e: Exception) {
        WarrenVoucherOutcome.Failure("invalid JNI response: ${e.message}")
    }

/**
 * The standing to show once a redemption was refused for [ban], the Kotlin twin
 * of `StandingTracker::on_issuance_ban`: a ban the standing already answered and
 * that still holds stays, since it knows when it lapses, and the unsigned
 * refusal may not.
 */
internal fun withRefusalBan(standing: AccountStanding?, ban: AccountBan): AccountStanding =
    when {
        standing == null -> AccountStanding(emptyList(), threshold = 0, windowDays = 0, ban = ban)
        standing.ban?.inForce == true -> standing
        else -> standing.copy(ban = ban)
    }

/** The error `redeemVoucher` answers for an unknown, spent, cancelled or expired voucher. */
private const val VOUCHER_REJECTED = "voucher rejected"
