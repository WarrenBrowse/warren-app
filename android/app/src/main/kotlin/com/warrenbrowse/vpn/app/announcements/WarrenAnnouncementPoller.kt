package com.warrenbrowse.vpn.app.announcements

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.WarrenAnnouncement
import com.warrenbrowse.vpn.lib.model.WarrenAnnouncementCta
import com.warrenbrowse.vpn.lib.model.WarrenNoticeLevel
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAnnouncementState
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import kotlin.time.Duration
import kotlin.time.Duration.Companion.minutes
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * When the next announcements fetch runs, from what the last one brought back:
 * the daemon's `warren_announcements_updater` cadence, kept here because Kotlin
 * owns the loop on Android.
 *
 * Five minutes between checks, the delay an operator waits for a publication or
 * a withdrawal to reach a running client. After a transport failure the retry
 * doubles from 20 s up to 240 s, so a client that just regained a network does
 * not sit a full interval showing nothing; a server that answered, whatever it
 * said, clears the fast retry.
 */
object WarrenAnnouncementCadence {
    val CHECK_INTERVAL: Duration = 5.minutes
    val RETRY_MIN: Duration = 20.seconds
    val RETRY_MAX: Duration = 240.seconds

    /** The delay before the next fetch and the retry state to carry over. */
    fun next(fetch: String, retry: Duration?): Pair<Duration, Duration?> =
        when (fetch) {
            FETCH_TRANSPORT,
            FETCH_DEFERRED -> {
                val armed = retry?.let { (it * 2).coerceAtMost(RETRY_MAX) } ?: RETRY_MIN
                armed to armed
            }
            else -> CHECK_INTERVAL to null
        }

    const val FETCH_TRANSPORT = "transport"

    /** The tunnel is between states: nothing was fetched, so it counts as unreachable. */
    const val FETCH_DEFERRED = "deferred"
}

/**
 * The foreground poll of the launch announcements: one fetch on every resume,
 * then one every five minutes while the app is visible.
 *
 * No WorkManager and no service wake-up carries it, for the reason the notice
 * poll has: a background cadence would make the app a periodic beacon for a
 * card nobody is looking at, and the fetch on resume already catches up on
 * whatever the operator published meanwhile.
 *
 * The announcements themselves ride a document that is byte-identical for every
 * caller, so the poll says nothing about the account. The code the offer
 * carries cannot ride that document, so it is drawn over a second,
 * wallet-signed call, and only for an announcement that actually carries a
 * campaign: an announcement with no offer never touches the wallet.
 */
class WarrenAnnouncementPoller(
    private val jni: WarrenJniBridge,
    private val state: WarrenAnnouncementState,
    private val tunnelState: WarrenTunnelStateProvider,
    private val wallet: WalletRepository,
    private val clientVersion: String,
    private val io: CoroutineDispatcher = Dispatchers.IO,
) {
    /** Runs until cancelled: the caller scopes it to the visible lifecycle. */
    suspend fun run() {
        var retry: Duration? = null
        while (true) {
            val (wait, next) = WarrenAnnouncementCadence.next(fetchOnce(), retry)
            retry = next
            delay(wait)
        }
    }

    /**
     * [run] while [wanted] is true and nothing while it is false. The gate is
     * the privacy disclosure: nothing leaves this device before the user has
     * accepted it, and an announcement is worth no exception. Accepting fetches
     * at once, so the first card is never five minutes late.
     */
    suspend fun runWhile(wanted: Flow<Boolean>) {
        wanted.distinctUntilChanged().collectLatest { if (it) run() }
    }

    /** One fetch published to [WarrenAnnouncementState]; returns the fetch class. */
    suspend fun fetchOnce(): String {
        if (ForumPreflight.of(tunnelState.connectedInfo.value) is ForumPreflight.Defer) {
            // Nothing was fetched, so nothing is published: a deferred cycle
            // must not take down a card the operator has not withdrawn.
            return WarrenAnnouncementCadence.FETCH_DEFERRED
        }
        val raw = withContext(io) { fetchRaw() }
        return if (raw == null) {
            WarrenAnnouncementCadence.FETCH_TRANSPORT
        } else {
            val (announcements, fetch) = parseAnnouncementsEnvelope(raw)
            // Published on every readable cycle, the empty list included: that
            // is what takes the card down when the announcement is withdrawn or
            // lapses. An unreadable envelope publishes nothing at all, so a
            // parsing bug can never erase a live announcement.
            announcements?.let { state.setAnnouncements(withCodes(it)) }
            fetch
        }
    }

    /**
     * The same announcements with this account's code attached to each one that
     * offers a campaign.
     *
     * The wallet is read only when an announcement actually carries an offer,
     * and only once for the whole set: the signed lookup is the one request
     * here that is tied to an account, so it is not made speculatively.
     */
    private suspend fun withCodes(
        announcements: List<WarrenAnnouncement>
    ): List<WarrenAnnouncement> {
        if (announcements.none { it.voucherCampaignId != null }) {
            return announcements
        }
        if (wallet.state.value is WalletState.Absent) {
            return announcements
        }
        val mnemonic =
            try {
                wallet.readMnemonic()
            } catch (e: Exception) {
                // A wallet that cannot be read is a card with no code, never a
                // card withheld: the operator's text still reaches the reader.
                Logger.w(throwable = e) { "WarrenAnnouncementPoller: mnemonic read failed" }
                return announcements
            }
        return mnemonic.use { m ->
            announcements.map { announcement ->
                val campaign = announcement.voucherCampaignId
                if (campaign == null) {
                    announcement
                } else {
                    announcement.copy(voucherCode = withContext(io) { claim(m.phrase, campaign) })
                }
            }
        }
    }

    /**
     * The code drawn for this account, `null` when the account is outside the
     * cohort and `null` when the lookup failed. Rust holds the answer per
     * identity, so a five minute poll is not a signed request every five
     * minutes.
     */
    // The JNI call is a system boundary: whatever crosses it as a throwable is
    // one card without its code, never a crash.
    @Suppress("TooGenericExceptionCaught")
    private fun claim(mnemonic: String, campaignId: String): String? =
        try {
            parseVoucherEnvelope(jni.campaignVoucher(mnemonic, campaignId))
        } catch (e: Exception) {
            Logger.w(throwable = e) { "WarrenJniBridge.campaignVoucher threw" }
            null
        }

    // The JNI call is a system boundary: whatever crosses it as a throwable is
    // one failed fetch, retried on the fast cadence, never a crash.
    @Suppress("TooGenericExceptionCaught")
    private fun fetchRaw(): String? =
        try {
            jni.announcementsFetch(clientVersion)
        } catch (e: Exception) {
            Logger.w(throwable = e) { "WarrenJniBridge.announcementsFetch threw" }
            null
        }
}

/**
 * The `{"announcements":[..],"fetch":..}` JNI envelope: the announcements to
 * display and the fetch class. Pure, unit-tested.
 *
 * A null list means the envelope could not be read at all, which is a failed
 * fetch rather than an empty set: taking the card down because the boundary
 * answered nonsense would let a parsing bug erase a live announcement. A single
 * unreadable ROW is dropped and the rest of the set still shows, for the same
 * reason in the other direction.
 */
internal fun parseAnnouncementsEnvelope(rawJson: String): Pair<List<WarrenAnnouncement>?, String> =
    try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        val fetch =
            root["fetch"]?.jsonPrimitive?.contentOrNull
                ?: WarrenAnnouncementCadence.FETCH_TRANSPORT
        val announcements =
            root["announcements"]?.jsonArray.orEmpty().mapNotNull { element ->
                val row = element.jsonObject
                val id = row["id"]?.jsonPrimitive?.contentOrNull
                val headline = row["headline"]?.jsonPrimitive?.contentOrNull
                if (id.isNullOrEmpty() || headline.isNullOrEmpty()) {
                    null
                } else {
                    WarrenAnnouncement(
                        id = id,
                        headline = headline,
                        body = row["body"]?.jsonPrimitive?.contentOrNull.orEmpty(),
                        level =
                            WarrenNoticeLevel.of(
                                row["level"]?.jsonPrimitive?.contentOrNull.orEmpty()
                            ),
                        cta = cta(row["cta"]),
                        voucherCampaignId =
                            row["voucher_campaign_id"]?.jsonPrimitive?.contentOrNull,
                    )
                }
            }
        announcements to fetch
    } catch (e: IllegalArgumentException) {
        Logger.w(throwable = e) { "announcementsFetch answered a malformed envelope" }
        null to WarrenAnnouncementCadence.FETCH_TRANSPORT
    }

/**
 * The call to action of one row, `null` when there is none. Rust has already
 * refused a URL that is not safe to render as a link, so nothing is checked
 * again here beyond the two fields being there.
 */
private fun cta(element: kotlinx.serialization.json.JsonElement?): WarrenAnnouncementCta? {
    val row = (element as? kotlinx.serialization.json.JsonObject) ?: return null
    val label = row["label"]?.jsonPrimitive?.contentOrNull
    val url = row["url"]?.jsonPrimitive?.contentOrNull
    return if (label.isNullOrEmpty() || url.isNullOrEmpty()) {
        null
    } else {
        WarrenAnnouncementCta(label, url)
    }
}

/**
 * The `{"ok":..,"code":..}` envelope of the wallet-signed lookup: the code, or
 * null both for an account outside the cohort and for a failed lookup. The two
 * differ for the caller only in that nothing is cached on a failure, which Rust
 * owns; on this side both are simply a card with no code.
 */
internal fun parseVoucherEnvelope(rawJson: String): String? =
    try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        if (root["ok"]?.jsonPrimitive?.booleanOrNull == true) {
            root["code"]?.jsonPrimitive?.contentOrNull
        } else {
            null
        }
    } catch (e: IllegalArgumentException) {
        Logger.w(throwable = e) { "campaignVoucher answered a malformed envelope" }
        null
    }
