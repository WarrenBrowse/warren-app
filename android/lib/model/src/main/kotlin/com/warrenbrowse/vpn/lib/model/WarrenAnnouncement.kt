package com.warrenbrowse.vpn.lib.model

/**
 * One launch announcement, as Rust hands it over: already verified against the
 * pinned server key and already filtered for the envelope expiry, the
 * announcement's own TTL and this app's version.
 *
 * [headline] and [body] are the operator's own words and are rendered verbatim
 * as plain text. They are never translated and never parsed as markup: the
 * whole point of the signed channel is that what the operator wrote is what the
 * user reads.
 *
 * [voucherCampaignId] is the campaign this account may hold a code for, and its
 * presence IS the offer. [voucherCode] is that code once the wallet-signed
 * lookup has answered, `null` while it has not, and `null` for good when this
 * account is outside the cohort. It is a bearer token worth a month of service:
 * it belongs on the account's own screen and nowhere else, never in a log, an
 * error or a problem report.
 */
data class WarrenAnnouncement(
    val id: String,
    val headline: String,
    val body: String,
    val level: WarrenNoticeLevel,
    val cta: WarrenAnnouncementCta? = null,
    val voucherCampaignId: String? = null,
    val voucherCode: String? = null,
)

/**
 * The call to action an announcement carries, present only when the URL passed
 * the contract's own check in Rust. [label] is the operator's caption, [url] an
 * `https` destination opened in the system browser.
 */
data class WarrenAnnouncementCta(val label: String, val url: String)
