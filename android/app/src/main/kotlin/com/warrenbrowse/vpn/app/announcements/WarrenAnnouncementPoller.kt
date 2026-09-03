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
     * The answers this identity already has, keyed by campaign, and the wallet
     * they were drawn under. The seed is read from the Keystore and crossed
     * over the FFI once per campaign rather than on every five minute cycle:
     * Rust answers a repeat lookup from its own per-identity cache, so the
     * only thing those repeats produce is the cleartext seed materialised as a
     * JVM string twelve times an hour for the length of the campaign.
     */
    private val held = mutableMapOf<String, VoucherAnswer>()
    private var heldFor: String? = null

    /**
     * The same announcements with this account's code attached to each one that
     * offers a campaign.
     *
     * The wallet is read only when an announcement carries an offer this
     * identity has no answer for yet, and only once for the whole set: the
     * signed lookup is the one request here that is tied to an account, so it
     * is not made speculatively.
     */
    private suspend fun withCodes(
        announcements: List<WarrenAnnouncement>
    ): List<WarrenAnnouncement> {
        val campaigns = announcements.mapNotNull { it.voucherCampaignId }.distinct()
        if (campaigns.isEmpty()) {
            return announcements
        }
        val identity = walletIdentity() ?: return announcements
        if (identity != heldFor) {
            // A code belongs to the account it was drawn for.
            held.clear()
            heldFor = identity
        }
        val unanswered = campaigns.filter { held[it] == null }
        if (unanswered.isNotEmpty()) {
            val mnemonic =
                try {
                    wallet.readMnemonic()
                } catch (e: Exception) {
                    // A wallet that cannot be read is a card with no code,
                    // never a card withheld: the operator's text still reaches
                    // the reader.
                    Logger.w(throwable = e) { "WarrenAnnouncementPoller: mnemonic read failed" }
                    return announcements
                }
            mnemonic.use { m ->
                unanswered.forEach { campaign ->
                    val answer = withContext(io) { claim(m.phrase, campaign) }
                    // A lookup that did not happen is retried on the next
                    // cycle; a server answer, code or no code, is final.
                    if (answer != VoucherAnswer.Unanswered) {
                        held[campaign] = answer
                    }
                }
            }
        }
        return announcements.map { announcement ->
            when (val answer = held[announcement.voucherCampaignId]) {
                is VoucherAnswer.Drawn -> announcement.copy(voucherCode = answer.code)
                else -> announcement
            }
        }
    }

    /**
     * The account the held answers belong to, `null` when this device holds no
     * wallet at all. The address is a key in memory and never logged.
     */
    private fun walletIdentity(): String? =
        when (val state = wallet.state.value) {
            is WalletState.Locked -> state.pubkey.value
            is WalletState.Ready -> state.pubkey.value
            else -> null
        }

    /**
     * The answer to the wallet-signed lookup for this account. Rust holds it
     * per identity, so a repeat is not a signed request, and the poller holds
     * it too so a repeat is not a Keystore read either.
     */
    // The JNI call is a system boundary: whatever crosses it as a throwable is
    // one card without its code, never a crash.
    @Suppress("TooGenericExceptionCaught")
    private fun claim(mnemonic: String, campaignId: String): VoucherAnswer =
        try {
            parseVoucherEnvelope(jni.campaignVoucher(mnemonic, campaignId))
        } catch (e: Exception) {
            Logger.w(throwable = e) { "WarrenJniBridge.campaignVoucher threw" }
            VoucherAnswer.Unanswered
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
 * What the wallet-signed lookup answered for this account. A card with no code
 * either way, and the difference is only whether asking again could ever change
 * the answer.
 */
internal sealed interface VoucherAnswer {
    /** The code drawn for this account. */
    data class Drawn(val code: String) : VoucherAnswer

    /** The server answered: this account is outside the cohort, for good. */
    data object Outside : VoucherAnswer

    /** No answer came back, so the next cycle asks again. */
    data object Unanswered : VoucherAnswer
}

/** The `{"ok":..,"code":..}` envelope of the wallet-signed lookup. */
internal fun parseVoucherEnvelope(rawJson: String): VoucherAnswer =
    try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        if (root["ok"]?.jsonPrimitive?.booleanOrNull == true) {
            root["code"]?.jsonPrimitive?.contentOrNull?.let(VoucherAnswer::Drawn)
                ?: VoucherAnswer.Outside
        } else {
            VoucherAnswer.Unanswered
        }
    } catch (e: IllegalArgumentException) {
        // Only the class of the failure: a JSON decoder quotes the input it
        // choked on in its message, and that input is the envelope carrying
        // the code. The announcements and notices envelopes carry only
        // operator-authored text, so they still log the throwable.
        Logger.w { "campaignVoucher answered a malformed envelope (${e::class.simpleName})" }
        VoucherAnswer.Unanswered
    }
