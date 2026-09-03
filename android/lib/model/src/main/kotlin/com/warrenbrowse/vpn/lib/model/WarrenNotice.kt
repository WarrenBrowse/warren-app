package com.warrenbrowse.vpn.lib.model

/**
 * One operator broadcast notice, as Rust hands it over: already verified
 * against the pinned server key and already filtered for the envelope expiry,
 * the notice's own TTL and this app's version.
 *
 * [message] is the operator's own words and is rendered verbatim as plain
 * text. It is never translated and never parsed as markup: the whole point of
 * the signed channel is that what the operator wrote is what the user reads.
 */
data class WarrenNotice(val id: String, val message: String, val level: WarrenNoticeLevel) {
    /**
     * Key a dismissal is recorded under. The wording is part of it, not just
     * the id: an operator who rewrites a notice in place keeps its id, and a
     * key on the id alone would bury the new words for everyone who had put
     * the old ones away.
     */
    val dismissalKey: String
        get() = "$id:${message.hashCode()}"
}

/** Severity of a [WarrenNotice], which picks the banner's title and colour. */
enum class WarrenNoticeLevel {
    INFO,
    WARNING,
    ERROR;

    companion object {
        /**
         * The level for a wire token, [INFO] for anything this build does not
         * know. A newer severity must degrade to the calmest banner rather
         * than drop the message: the operator still has something to say.
         */
        fun of(token: String): WarrenNoticeLevel =
            when (token) {
                "error" -> ERROR
                "warning" -> WARNING
                else -> INFO
            }
    }
}
