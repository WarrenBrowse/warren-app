package com.warrenbrowse.vpn.lib.common.util

import android.content.Context
import com.warrenbrowse.vpn.lib.model.AbuseCategory
import com.warrenbrowse.vpn.lib.model.AccountBan
import com.warrenbrowse.vpn.lib.model.AccountStrike
import com.warrenbrowse.vpn.lib.model.StrikeNotice
import com.warrenbrowse.vpn.lib.ui.resource.R
import java.text.DateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

/**
 * How the app words the wallet's port-forward abuse standing (warren-core doc
 * 105), in the desktop's words (`src/shared/account-standing.ts`): the warning
 * a strike raises, the day it was recorded, and the date a ban lapses. Shared
 * by the system notification, the banner and the port-forwarding screen, so
 * the three always say the same thing.
 */
object AccountStandingText {
    /**
     * A day as the reader writes it. A strike is recorded at day precision, as
     * midnight UTC, and a ban lapses on a day too, so both are formatted in
     * UTC: in local time a strike would move to the day before for everyone
     * west of Greenwich.
     */
    fun day(unixSecs: Long, locale: Locale): String =
        DateFormat.getDateInstance(DateFormat.LONG, locale)
            .apply { timeZone = TimeZone.getTimeZone("UTC") }
            .format(Date(unixSecs * MILLIS_PER_SECOND))

    fun category(context: Context, category: AbuseCategory): String =
        context.getString(
            when (category) {
                AbuseCategory.Copyright -> R.string.abuse_category_copyright
                AbuseCategory.MalwareC2 -> R.string.abuse_category_malware
                AbuseCategory.Spam -> R.string.abuse_category_spam
                AbuseCategory.Scanning -> R.string.abuse_category_scanning
                AbuseCategory.Phishing -> R.string.abuse_category_phishing
                AbuseCategory.Csam -> R.string.abuse_category_csam
                AbuseCategory.Other -> R.string.abuse_category_other
            }
        )

    /** "Warning 1 of 3: port N was closed on DAY after an abuse report (category)." */
    fun warning(context: Context, notice: StrikeNotice, locale: Locale): String {
        val strike = notice.strike
        val day = day(strike.dayUnixSecs, locale)
        val category = category(context, strike.category)
        return if (notice.threshold <= 0) {
            context.getString(
                R.string.account_strike_warning_no_threshold,
                notice.ordinal,
                strike.port,
                day,
                category,
            )
        } else {
            context.getString(
                R.string.account_strike_warning,
                notice.ordinal,
                notice.threshold,
                strike.port,
                day,
                category,
            )
        }
    }

    /** The reference a contest quotes. */
    fun caseReference(context: Context, strike: AccountStrike): String =
        context.getString(R.string.account_strike_case_reference, strike.caseReference)

    /** How to contest a warning: write to the abuse desk quoting the reference. */
    fun contest(context: Context): String =
        context.getString(
            R.string.account_strike_contest,
            context.getString(R.string.abuse_contact_email),
        )

    /** The ban as one line: what for, and until when when that is known. */
    fun ban(context: Context, ban: AccountBan, locale: Locale): String {
        val until = ban.lapsesAtUnixSecs?.let { day(it, locale) }
        return when {
            ban.portForwarding && until != null ->
                context.getString(R.string.account_ban_port_forwarding_until, until)
            ban.portForwarding -> context.getString(R.string.account_ban_port_forwarding)
            until != null -> context.getString(R.string.account_ban_until, until)
            else -> context.getString(R.string.account_ban)
        }
    }

    private const val MILLIS_PER_SECOND = 1000L
}
