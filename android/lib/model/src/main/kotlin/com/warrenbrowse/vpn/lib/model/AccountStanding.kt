package com.warrenbrowse.vpn.lib.model

/**
 * The wallet's port-forward abuse standing (warren-core doc 105 §5.4), as the
 * Rust store answers it: the live strikes, the number that bans the account,
 * and the ban in force.
 *
 * A strike names the port that was closed and the case reference to quote
 * when contesting it. Both belong on the account's own screens and nowhere
 * else, so [AccountStrike.toString] renders neither: a log line that prints a
 * standing prints no case.
 */
data class AccountStanding(
    /** Strikes still inside the window, oldest first. */
    val strikes: List<AccountStrike>,
    /** Live strikes that ban the account, `0` while only a ban is known. */
    val threshold: Int,
    /** Length of the sliding window in days, `0` while unknown. */
    val windowDays: Int,
    val ban: AccountBan?,
) {
    /** The newest strike with its rank among the live ones, from 1. */
    fun latestStrike(): StrikeNotice? =
        strikes.lastOrNull()?.let { StrikeNotice(it, ordinal = strikes.size, threshold = threshold) }
}

/** One strike on the account. */
data class AccountStrike(
    /** Day of the strike, midnight UTC, in Unix seconds. */
    val dayUnixSecs: Long,
    val category: AbuseCategory,
    /** ISO country of the exit that held the port, when known. */
    val exitCountry: String?,
    /** The forwarded public port that was closed. */
    val port: Int,
    /** The reference a contest quotes. */
    val caseReference: String,
) {
    /**
     * A digest of the case reference, the key a dismissed banner is
     * remembered by in the preferences, so the preferences name no case.
     */
    val dismissalKey: String
        get() = "strike:" + Integer.toHexString(caseReference.hashCode())

    override fun toString(): String = "AccountStrike(day=$dayUnixSecs, category=$category)"
}

/** A ban on the wallet. */
data class AccountBan(
    /** Banned for port-forwarding abuse, rather than any other reason. */
    val portForwarding: Boolean,
    /** When the ban lapses on its own, Unix seconds; `null` when unknown. */
    val lapsesAtUnixSecs: Long?,
    /** Whether the ban held when the standing was read. */
    val inForce: Boolean,
)

/** A strike to warn about: "warning [ordinal] of [threshold]". */
data class StrikeNotice(val strike: AccountStrike, val ordinal: Int, val threshold: Int)

/** Category of the abuse report behind a strike. */
enum class AbuseCategory {
    Copyright,
    MalwareC2,
    Spam,
    Scanning,
    Phishing,
    Csam,
    Other;

    companion object {
        /**
         * The category the API names, `Other` for one this build does not
         * know: a new category must never drop a warning.
         */
        fun of(wire: String?): AbuseCategory =
            when (wire) {
                "copyright" -> Copyright
                "malware_c2" -> MalwareC2
                "spam" -> Spam
                "scanning" -> Scanning
                "phishing" -> Phishing
                "csam" -> Csam
                else -> Other
            }
    }
}
