package com.warrenbrowse.vpn.app.forum

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.CollectedReport
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenSupportReporter
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/** The result of an attach-logs attempt, mirroring the desktop's `ForumAttachResult`. */
sealed interface WarrenForumAttachOutcome {
    /** Attached to the topic, or parked for a report still being composed. */
    data object Attached : WarrenForumAttachOutcome

    /** The wallet is not the author of the topic; the provider refused. */
    data object NotAuthor : WarrenForumAttachOutcome

    /**
     * The attach session is gone: expired, cancelled, already served, or
     * bound to another topic than the one sent (the provider answers the
     * same 404 for all four).
     */
    data object Expired : WarrenForumAttachOutcome

    /** The gzipped report is over the cap, here or at the provider. */
    data object TooLarge : WarrenForumAttachOutcome

    /** The signature was refused for a device clock outside the window. */
    data object ClockSkew : WarrenForumAttachOutcome

    /** The provider failed on its own side (5xx); nothing to fix here. */
    data object ServerError : WarrenForumAttachOutcome

    /** No wallet on device: there is no key to sign the report with. */
    data object WalletNotReady : WarrenForumAttachOutcome

    /**
     * Not attempted: the tunnel is between states ([ForumPreflight]), so the
     * connect host could not be resolved. Nothing was collected and the
     * session is untouched; the prompt stays open for a retry.
     */
    data class Deferred(val tunnelClass: String) : WarrenForumAttachOutcome

    /**
     * Any other failure, with its class (`collect-failed`, `gzip`,
     * `wallet-read`, `jni`, `transport`, `upload-timeout`, `http-<status>`).
     * Rendered generically; the class goes to the log and the journal.
     */
    data class Failure(val reason: String) : WarrenForumAttachOutcome
}

/**
 * Attaches the redacted problem report to a forum bug report (doc 55), on
 * the user's approval of the consent prompt and never before: collects the
 * report for the send (the live probes run, like the in-app report's own
 * send), gzips it, sizes it against the broker's cap, reads the mnemonic
 * silently, and hands everything to [WarrenJniBridge.forumAttachLogs], which
 * preflights the session, signs AND POSTs `/v1/forum/attach-logs` inside
 * Rust. Only the link, the topic and the gzip cross the boundary. The
 * collected file is deleted whatever the outcome.
 */
class WarrenForumAttachUseCase(
    private val walletRepository: WalletRepository,
    private val reporter: WarrenSupportReporter,
    private val journal: ForumJournal,
    private val jni: WarrenJniBridge,
    private val tunnelState: WarrenTunnelStateProvider,
    private val compressor: LogCompressor = GzipLogCompressor,
    // Fire-and-forget scope for the cancel notify so it survives the consent
    // prompt being dismissed (a composition scope would cancel it mid-flight).
    private val cancelScope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
) {

    /**
     * Attach the logs to [topicId] ([ForumAttachLink.PRE_TOPIC] for a report
     * still being composed) for the session [link] names. [topicId] is passed
     * apart from the link because a typed code carries none and the prompt
     * asks for it.
     */
    suspend fun attach(link: ForumAttachLink, topicId: Long): WarrenForumAttachOutcome {
        refusedBeforeCollecting()?.let {
            return it
        }
        val report =
            reporter.collect(forSend = true).getOrElse { e ->
                Logger.w(throwable = e) { "WarrenForumAttachUseCase: collection failed" }
                journal.record(ForumEvent.ATTACH_RESULT, JournalField.Class("collect-failed"))
                return WarrenForumAttachOutcome.Failure("collect-failed")
            }
        try {
            return when (val logs = compressForWire(report)) {
                is WireLogs.Refused -> logs.outcome
                is WireLogs.Ready -> signAndSend(link, topicId, logs.gz)
            }
        } finally {
            reporter.discard(report)
        }
    }

    /** The redacted report for "View the logs": nothing leaves the device for it. */
    suspend fun collectForPreview(): Result<CollectedReport> = reporter.collect(forSend = false)

    fun discard(report: CollectedReport) = reporter.discard(report)

    /** The outcome that ends an attach before anything is collected, or null to go on. */
    private fun refusedBeforeCollecting(): WarrenForumAttachOutcome? {
        if (walletRepository.state.value is WalletState.Absent) {
            Logger.w("WarrenForumAttachUseCase: no wallet on device")
            journal.record(ForumEvent.ATTACH_RESULT, JournalField.Class("wallet-absent"))
            return WarrenForumAttachOutcome.WalletNotReady
        }
        // A deferred attempt must touch nothing.
        val preflight = ForumPreflight.of(tunnelState.connectedInfo.value)
        return if (preflight is ForumPreflight.Defer) {
            Logger.w("WarrenForumAttachUseCase: deferred, tunnel ${preflight.tunnelClass}")
            journal.record(ForumEvent.ATTACH_DEFERRED, JournalField.Class(preflight.tunnelClass))
            WarrenForumAttachOutcome.Deferred(preflight.tunnelClass)
        } else {
            null
        }
    }

    /** The report's logs gzipped for the wire, or the outcome that refuses them. */
    private sealed interface WireLogs {
        class Ready(val gz: ByteArray) : WireLogs

        class Refused(val outcome: WarrenForumAttachOutcome) : WireLogs
    }

    private suspend fun compressForWire(report: CollectedReport): WireLogs {
        val gz =
            try {
                withContext(Dispatchers.IO) { compressor.compress(report.file) }
            } catch (e: IOException) {
                Logger.w(throwable = e) { "WarrenForumAttachUseCase: gzip failed" }
                journal.record(ForumEvent.ATTACH_RESULT, JournalField.Class("gzip"))
                return WireLogs.Refused(WarrenForumAttachOutcome.Failure("gzip"))
            }
        // The first leg of the report-size chain: a gzip the broker would
        // refuse reaches neither the wallet nor the network.
        return if (gz.size > WarrenSupportReporterImpl.MAX_LOG_GZ_BYTES) {
            journal.record(
                ForumEvent.ATTACH_RESULT,
                JournalField.Class("too-large"),
                JournalField.GzBytes(gz.size.toLong()),
            )
            WireLogs.Refused(WarrenForumAttachOutcome.TooLarge)
        } else {
            WireLogs.Ready(gz)
        }
    }

    // The keystore read and the JNI bridge each fail through an open set of
    // runtime exceptions (keystore, user-auth, bridge and native-panic classes),
    // and every one of them means the same thing here: this attempt failed and
    // its class goes to the journal. None may escape into the consent prompt.
    @Suppress("TooGenericExceptionCaught")
    private suspend fun signAndSend(
        link: ForumAttachLink,
        topicId: Long,
        gz: ByteArray,
    ): WarrenForumAttachOutcome {
        val mnemonic =
            try {
                walletRepository.readMnemonic()
            } catch (e: Exception) {
                Logger.e(throwable = e) { "WarrenForumAttachUseCase: mnemonic read failed" }
                journal.record(ForumEvent.ATTACH_RESULT, JournalField.Class("wallet-read"))
                return WarrenForumAttachOutcome.Failure("wallet-read")
            }
        val started = System.currentTimeMillis()
        journal.record(ForumEvent.ATTACH_SIGNING, JournalField.PreTopic(topicId == ForumAttachLink.PRE_TOPIC))
        return withContext(Dispatchers.IO) {
            val outcome =
                mnemonic.use { m ->
                    val raw =
                        try {
                            jni.forumAttachLogs(m.phrase, link.sid, topicId, link.host, gz)
                        } catch (e: Exception) {
                            Logger.e(throwable = e) { "WarrenJniBridge.forumAttachLogs threw" }
                            return@use WarrenForumAttachOutcome.Failure("jni")
                        }
                    parseForumAttachOutcome(raw)
                }
            journal.record(
                ForumEvent.ATTACH_RESULT,
                JournalField.Class(attachOutcomeClass(outcome)),
                JournalField.ElapsedMs(System.currentTimeMillis() - started),
                JournalField.GzBytes(gz.size.toLong()),
            )
            if (outcome !is WarrenForumAttachOutcome.Attached) {
                Logger.w("WarrenForumAttachUseCase: not attached: ${attachOutcomeClass(outcome)}")
            }
            outcome
        }
    }

    /**
     * Best-effort: notify the provider the user declined so the waiting forum
     * page shows "cancelled". Fire-and-forget on [cancelScope] so it is not
     * cancelled when the consent prompt leaves composition.
     */
    // The bridge's failure classes are the same open set as in signAndSend, and
    // a decline that cannot be relayed is not the user's problem.
    @Suppress("TooGenericExceptionCaught")
    fun cancel(link: ForumAttachLink) {
        journal.record(ForumEvent.ATTACH_DECLINED)
        cancelScope.launch {
            try {
                jni.forumAttachCancel(link.sid, link.host)
            } catch (e: Exception) {
                Logger.w(throwable = e) { "WarrenJniBridge.forumAttachCancel threw" }
            }
        }
    }
}

/** The coarse class of an outcome, for the log and the journal. */
internal fun attachOutcomeClass(outcome: WarrenForumAttachOutcome): String =
    when (outcome) {
        WarrenForumAttachOutcome.Attached -> "attached"
        WarrenForumAttachOutcome.NotAuthor -> "not-author"
        WarrenForumAttachOutcome.Expired -> "expired"
        WarrenForumAttachOutcome.TooLarge -> "too-large"
        WarrenForumAttachOutcome.ClockSkew -> "clock-skew"
        WarrenForumAttachOutcome.ServerError -> "server-error"
        WarrenForumAttachOutcome.WalletNotReady -> "wallet-absent"
        is WarrenForumAttachOutcome.Deferred -> "deferred-${outcome.tunnelClass}"
        is WarrenForumAttachOutcome.Failure -> outcome.reason
    }

/**
 * Map the `{"ok":..}` JNI envelope (`warren_jni::forum::attach_envelope`) to
 * an outcome. Pure, so it is unit-testable off-device; never surfaces the raw
 * error string to the user.
 */
internal fun parseForumAttachOutcome(rawJson: String): WarrenForumAttachOutcome =
    try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        if (root["ok"]?.jsonPrimitive?.boolean == true) {
            WarrenForumAttachOutcome.Attached
        } else {
            when (root["error"]?.jsonPrimitive?.content) {
                "not-author" -> WarrenForumAttachOutcome.NotAuthor
                "expired" -> WarrenForumAttachOutcome.Expired
                "too-large" -> WarrenForumAttachOutcome.TooLarge
                "clock-skew" -> WarrenForumAttachOutcome.ClockSkew
                "server-error" -> WarrenForumAttachOutcome.ServerError
                else ->
                    WarrenForumAttachOutcome.Failure(
                        root["reason"]?.jsonPrimitive?.content?.takeIf { it.isNotBlank() } ?: "unknown"
                    )
            }
        }
    } catch (e: SerializationException) {
        WarrenForumAttachOutcome.Failure("invalid-envelope")
    } catch (e: IllegalArgumentException) {
        // A field of the wrong JSON shape is as much a broken envelope as bad JSON.
        WarrenForumAttachOutcome.Failure("invalid-envelope")
    } catch (e: IllegalStateException) {
        WarrenForumAttachOutcome.Failure("invalid-envelope")
    }
