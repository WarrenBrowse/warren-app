package com.warrenbrowse.vpn.feature.settings.impl

import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale

/**
 * How the account "Paid until" row renders, mirroring the desktop
 * `FormattedAccountExpiry`: a known-past expiry is the red OUT OF TIME state,
 * a known-future expiry shows the date, and an unknown expiry (never fetched,
 * or no subscription bound to the wallet yet, both cached as 0) reads as
 * "Currently unavailable" rather than a bogus 1970 date.
 */
sealed interface PaidUntilDisplay {
    data object OutOfTime : PaidUntilDisplay

    data class Date(val expiryUnixSecs: Long) : PaidUntilDisplay

    data object Unavailable : PaidUntilDisplay
}

internal fun paidUntilDisplay(expiryUnixSecs: Long, nowSecs: Long): PaidUntilDisplay = when {
    expiryUnixSecs <= 0L -> PaidUntilDisplay.Unavailable
    expiryUnixSecs <= nowSecs -> PaidUntilDisplay.OutOfTime
    else -> PaidUntilDisplay.Date(expiryUnixSecs)
}

/**
 * The big remaining-time headline on the subscription card: whole days
 * (floored), "less than a day" under 24 h, and the coarser unit above a month
 * and above a year. Only rendered while the subscription is active ([None]
 * otherwise). The home header names the same unit for the same expiry
 * (`accountTimeLeftLabel`), so the two thresholds must stay in step.
 */
sealed interface RemainingTime {
    data object None : RemainingTime

    data object LessThanADay : RemainingTime

    data class Days(val days: Long) : RemainingTime

    data class Months(val months: Long) : RemainingTime

    data class Years(val years: Long) : RemainingTime
}

internal fun remainingTime(expiryUnixSecs: Long, nowSecs: Long): RemainingTime {
    if (expiryUnixSecs <= 0L || expiryUnixSecs <= nowSecs) return RemainingTime.None
    val days = (expiryUnixSecs - nowSecs) / 86_400
    return when {
        days == 0L -> RemainingTime.LessThanADay
        days >= 365L -> RemainingTime.Years(days / 365)
        days >= 30L -> RemainingTime.Months(days / 30)
        else -> RemainingTime.Days(days)
    }
}

/**
 * How much time a voucher just credited, for the success line "%1$s was added,
 * account paid until %2$s." The JNI voucher result carries the new expiry only,
 * so the duration is the distance the expiry moved from where it would have
 * lapsed: the previous expiry when the account was still paid, now when it had
 * already lapsed (the server credits from the same boundary).
 *
 * [RemainingTime.None] when the expiry did not move, in which case the caller
 * falls back to the expiry-only line rather than claiming a zero credit.
 */
internal fun creditedTime(
    previousExpiryUnixSecs: Long,
    newExpiryUnixSecs: Long,
    nowSecs: Long,
): RemainingTime = remainingTime(newExpiryUnixSecs, maxOf(previousExpiryUnixSecs, nowSecs))

/**
 * Expiry rendering for the paid-until row and the voucher success line:
 * medium date + short time, like the desktop `formatDate`
 * (`Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' })`).
 */
internal fun formatExpiryDateTime(
    expiryUnixSecs: Long,
    zone: ZoneId = ZoneId.systemDefault(),
    locale: Locale = Locale.getDefault(),
): String =
    DateTimeFormatter.ofLocalizedDateTime(FormatStyle.MEDIUM, FormatStyle.SHORT)
        .withLocale(locale)
        .format(Instant.ofEpochSecond(expiryUnixSecs).atZone(zone))

/** Raw voucher length: 16 Crockford-32 chars, displayed as XXXX-XXXX-XXXX-XXXX. */
internal const val VOUCHER_RAW_LENGTH = 16

// Crockford-32 alphabet uppercase: 0-9 + A-Z minus I/L/O/U. Must stay in sync
// with `warren_api::vouchers::VOUCHER_ALPHABET` (same class as the desktop
// input filter; pasted I/L/O/U are dropped, not aliased, mirroring the
// backend's strict no-aliasing gate).
private val CROCKFORD_32: Set<Char> =
    (('0'..'9') + ('A'..'H') + 'J' + 'K' + ('M'..'N') + ('P'..'T') + ('V'..'Z')).toSet()

/**
 * Live input filter for the voucher field: uppercase, drop every char outside
 * the Crockford-32 alphabet (separators, spaces, ambiguous letters), cap at
 * [VOUCHER_RAW_LENGTH]. The stored value is always the raw 16-char secret; the
 * 4-4-4-4 dashes are display-only (see the dialog's visual transformation).
 */
internal fun filterVoucherInput(raw: String): String =
    raw.uppercase(Locale.ROOT).filter { it in CROCKFORD_32 }.take(VOUCHER_RAW_LENGTH)

internal fun isCompleteVoucher(code: String): Boolean =
    code.length == VOUCHER_RAW_LENGTH && code.all { it in CROCKFORD_32 }
