package com.warrenbrowse.vpn.app.standing

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.AbuseCategory
import com.warrenbrowse.vpn.lib.model.AccountBan
import com.warrenbrowse.vpn.lib.model.AccountStanding
import com.warrenbrowse.vpn.lib.model.AccountStrike
import com.warrenbrowse.vpn.lib.model.StrikeNotice
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.AccountStrikeAlerts
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenAccountStandingState
import com.warrenbrowse.vpn.lib.repository.WarrenStandingBridge
import kotlin.time.ComparableTimeMark
import kotlin.time.Duration
import kotlin.time.Duration.Companion.minutes
import kotlin.time.TimeSource
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.longOrNull

/**
 * Keeps the wallet's port-forward abuse standing current (warren-core doc 105
 * §5.4): the Android twin of the desktop daemon's standing monitor, on the
 * same ten minute cadence as the token refresh, so the request pattern the
 * API sees stays the one it already sees.
 *
 * It runs for as long as the VPN service lives, which is while the app is on
 * screen and while a tunnel is up: a strike lands on a forwarded port, and a
 * forwarded port only exists while the tunnel does. Each strike raises one
 * system notification, because Rust hands each one over exactly once and
 * remembers across process deaths which it already did.
 */
class WarrenAccountStandingPoller(
    private val jni: WarrenStandingBridge,
    private val state: WarrenAccountStandingState,
    private val alerts: AccountStrikeAlerts,
    private val wallet: WalletRepository,
    private val io: CoroutineDispatcher = Dispatchers.IO,
    private val clock: TimeSource.WithComparableMarks = TimeSource.Monotonic,
    /** Drops the strike banner dismissals of a wallet that left the device. */
    private val forgetDismissedStrikes: suspend () -> Unit = {},
) {
    private val polling = Mutex()
    private var lastAttemptAt: ComparableTimeMark? = null

    /** Runs until cancelled; the caller scopes it. */
    suspend fun run() {
        while (true) {
            pollIfDue()
            delay(RETRY)
        }
    }

    /**
     * [run] while [wanted] is true (the privacy disclosure accepted: nothing
     * leaves the device before it), and forgets the standing of a wallet that
     * leaves the device, whatever [wanted] says.
     */
    suspend fun runWhile(wanted: Flow<Boolean>) = coroutineScope {
        launch { forgetTheStandingOfADepartedWallet() }
        wanted.distinctUntilChanged().collectLatest { if (it) run() }
    }

    /**
     * Polls unless an attempt is younger than the cadence, so two owners of the
     * loop (a service that restarts, a resume) cost one request, not two, and
     * an API that keeps failing is asked no more often than one that answers.
     */
    suspend fun pollIfDue() =
        polling.withLock {
            val due = lastAttemptAt?.let { it.elapsedNow() >= INTERVAL } ?: true
            if (due && identityOf(wallet.state.value) != null) {
                lastAttemptAt = clock.markNow()
                pollOnce()
            }
        }

    /**
     * One poll published to the state; true when the API answered (a `404`
     * included: it will say the same thing for the next ten minutes).
     */
    suspend fun pollOnce(): Boolean {
        val poll =
            if (identityOf(wallet.state.value) == null) null
            else withContext(io) { readAndFetch() }?.let(::parseStandingEnvelope)
        // A failed poll keeps what is shown: the last answer still holds better
        // than nothing, and the next tick asks again.
        val answered = poll?.ok == true
        if (poll != null && answered) {
            state.setStanding(poll.standing)
            // Bounded: a hostile or broken answer must not flood the shade.
            poll.newStrikes.take(MAX_STRIKES).forEach(alerts::announce)
        }
        return answered
    }

    // The Keystore read is a system boundary: whatever it throws is one poll
    // skipped, retried on the next tick, never a crash.
    @Suppress("TooGenericExceptionCaught")
    private suspend fun readAndFetch(): String? {
        val mnemonic =
            try {
                wallet.readMnemonic()
            } catch (e: Exception) {
                Logger.w { "WarrenAccountStandingPoller: wallet unreadable (${e::class.simpleName})" }
                null
            }
        return mnemonic?.use { m -> fetch(m.phrase) }
    }

    private suspend fun forgetTheStandingOfADepartedWallet() {
        wallet.state
            .map { identityOf(it) }
            .distinctUntilChanged()
            .drop(1)
            .collect {
                polling.withLock {
                    lastAttemptAt = null
                    state.setStanding(null)
                    alerts.clear()
                    withContext(io) { forget() }
                    forgetDismissedStrikes()
                }
            }
    }

    private fun identityOf(state: WalletState): String? =
        when (state) {
            is WalletState.Locked -> state.pubkey.value
            is WalletState.Ready -> state.pubkey.value
            else -> null
        }

    // The JNI call is a system boundary: whatever crosses it as a throwable is
    // one failed poll, retried on the next tick, never a crash.
    @Suppress("TooGenericExceptionCaught")
    private fun fetch(phrase: String): String? =
        try {
            jni.accountStanding(phrase)
        } catch (e: Exception) {
            Logger.w { "WarrenStandingBridge.accountStanding threw (${e::class.simpleName})" }
            null
        }

    @Suppress("TooGenericExceptionCaught")
    private fun forget() {
        try {
            jni.forgetAccountStanding()
        } catch (e: Exception) {
            Logger.w { "WarrenStandingBridge.forgetAccountStanding threw (${e::class.simpleName})" }
        }
    }

    companion object {
        /** The token refresh cadence, which the desktop daemon polls on too. */
        val INTERVAL: Duration = 10.minutes

        /** How often the loop checks whether a poll is due. */
        val RETRY: Duration = 1.minutes

        /**
         * Strikes the app shows and announces from one answer, at most. Three
         * ban the account, so a longer list is a broken or hostile answer.
         */
        const val MAX_STRIKES = 10
    }
}

/** One poll as the Rust envelope answers it. */
internal data class StandingPoll(
    /** False on a failure other than a `404`: nothing is published. */
    val ok: Boolean,
    /** False on a `404`, an API that does not report the standing yet. */
    val reported: Boolean,
    val standing: AccountStanding?,
    val newStrikes: List<StrikeNotice>,
)

/**
 * Reads the `{"ok","reported","standing","new_strikes"}` envelope, `null` when
 * it is not one. A malformed strike is dropped rather than failing the whole
 * standing. The decoder's message is never logged: it quotes the input, which
 * carries case references.
 */
internal fun parseStandingEnvelope(rawJson: String): StandingPoll? =
    try {
        val root = Json.parseToJsonElement(rawJson) as? JsonObject
        root?.let {
            StandingPoll(
                ok = it.bool("ok") ?: false,
                reported = it.bool("reported") ?: true,
                standing = (it["standing"] as? JsonObject)?.let(::standingOf),
                newStrikes =
                    (it["new_strikes"] as? JsonArray)
                        .orEmpty()
                        .mapNotNull { element ->
                            val row = element as? JsonObject ?: return@mapNotNull null
                            val strike = (row["strike"] as? JsonObject)?.let(::strikeOf)
                            val ordinal = row.int("ordinal")
                            if (strike == null || ordinal == null) {
                                null
                            } else {
                                StrikeNotice(strike, ordinal, row.int("threshold") ?: 0)
                            }
                        },
            )
        }
    } catch (e: IllegalArgumentException) {
        Logger.w { "accountStanding answered a malformed envelope (${e::class.simpleName})" }
        null
    }

private fun standingOf(obj: JsonObject): AccountStanding =
    AccountStanding(
        strikes =
            (obj["strikes"] as? JsonArray)
                .orEmpty()
                .mapNotNull { (it as? JsonObject)?.let(::strikeOf) }
                .take(WarrenAccountStandingPoller.MAX_STRIKES),
        threshold = obj.int("threshold") ?: 0,
        windowDays = obj.int("window_days") ?: 0,
        ban =
            (obj["ban"] as? JsonObject)?.let { ban ->
                AccountBan(
                    portForwarding = ban.string("reason") == "port_forwarding_abuse",
                    lapsesAtUnixSecs = ban.long("lapses_at_unix_secs"),
                    inForce = ban.bool("in_force") ?: true,
                )
            },
    )

private fun strikeOf(obj: JsonObject): AccountStrike? {
    val reference = obj.string("case_reference")
    val port = obj.int("port")
    val day = obj.long("day_unix_secs")
    return if (reference.isNullOrEmpty() || port == null || day == null) {
        null
    } else {
        AccountStrike(
            dayUnixSecs = day,
            category = AbuseCategory.of(obj.string("category")),
            exitCountry = obj.string("exit_country"),
            port = port,
            caseReference = reference,
        )
    }
}

private fun JsonObject.primitive(key: String): JsonPrimitive? = this[key] as? JsonPrimitive

private fun JsonObject.string(key: String): String? = primitive(key)?.contentOrNull

private fun JsonObject.int(key: String): Int? = primitive(key)?.intOrNull

private fun JsonObject.long(key: String): Long? = primitive(key)?.longOrNull

private fun JsonObject.bool(key: String): Boolean? = primitive(key)?.booleanOrNull
