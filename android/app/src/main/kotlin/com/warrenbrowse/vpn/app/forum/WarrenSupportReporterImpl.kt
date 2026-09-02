package com.warrenbrowse.vpn.app.forum

import android.content.Context
import android.os.Build
import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.BuildConfig
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
import com.warrenbrowse.vpn.lib.model.wallet.WalletState
import com.warrenbrowse.vpn.lib.repository.CollectedReport
import com.warrenbrowse.vpn.lib.repository.ForumIdentityRepository
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.ReportForm
import com.warrenbrowse.vpn.lib.repository.ReportSubmitOutcome
import com.warrenbrowse.vpn.lib.repository.WalletRepository
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import com.warrenbrowse.vpn.lib.repository.WarrenSupportReporter
import com.warrenbrowse.vpn.lib.repository.WarrenTunnelStateProvider
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.Locale
import java.util.UUID
import java.util.zip.GZIPOutputStream
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long

/**
 * The platform facts of the report header, keyed as the header renders them.
 * [ForumDiagnostics] is the production reader; a test supplies the map, since
 * the readers need a device.
 */
fun interface ForumFacts {
    fun collect(tunnelState: String, walletState: String, lastLoginClass: String?): Map<String, String>
}

/**
 * Compresses the collected report for the wire. Gzip in production; a test
 * supplies the size it wants to see refused or accepted at the cap.
 */
fun interface LogCompressor {
    fun compress(file: File): ByteArray
}

/** Gzip, streamed from the report file. */
object GzipLogCompressor : LogCompressor {
    override fun compress(file: File): ByteArray {
        val out = ByteArrayOutputStream()
        GZIPOutputStream(out).use { gz -> file.inputStream().use { it.copyTo(gz) } }
        return out.toByteArray()
    }
}

/**
 * The production [WarrenSupportReporter]: collects the redacted report through
 * the shared Rust collector (platform facts read here, everything else read
 * and redacted in Rust), then signs and sends it through `WarrenJni.forumReport`
 * with the wallet mnemonic read silently, the way the forum login signs.
 */
class WarrenSupportReporterImpl(
    private val context: Context,
    private val jni: WarrenJniBridge,
    private val walletRepository: WalletRepository,
    private val forumIdentityRepository: ForumIdentityRepository,
    private val tunnelState: WarrenTunnelStateProvider,
    private val journal: ForumJournal,
    private val appLogDir: File,
    private val facts: ForumFacts = ForumDiagnostics(AndroidForumPlatformReads(context)),
    private val compressor: LogCompressor = GzipLogCompressor,
) : WarrenSupportReporter {

    override fun preflight(): ForumPreflight {
        val verdict = ForumPreflight.of(tunnelState.connectedInfo.value)
        if (verdict is ForumPreflight.Defer) {
            journal.record(ForumEvent.REPORT_DEFERRED, JournalField.Class(verdict.tunnelClass))
        }
        return verdict
    }

    override suspend fun collect(): Result<CollectedReport> =
        withContext(Dispatchers.IO) {
            runCatching {
                val wallet = walletRepository.state.value
                val pubkey =
                    when (wallet) {
                        is WalletState.Ready -> wallet.pubkey.value
                        is WalletState.Locked -> wallet.pubkey.value
                        else -> null
                    }
                val walletWord =
                    when (wallet) {
                        is WalletState.Ready -> "ready"
                        is WalletState.Locked -> "locked"
                        else -> "absent"
                    }
                val metadata =
                    facts.collect(
                        tunnelState = tunnelState.state.value.substringBefore(':').ifBlank { "unknown" },
                        walletState = walletWord,
                        lastLoginClass = journal.lastClassOf(ForumEvent.LOGIN_RESULT),
                    )
                val metadataJson =
                    JsonObject(metadata.mapValues { JsonPrimitive(it.value) }).toString()
                val redactJson = buildJsonArray(listOfNotNull(pubkey))
                val output = File(context.cacheDir, "warren-report-${UUID.randomUUID()}.log")
                val raw = jni.collectProblemReport(metadataJson, redactJson, appLogDir.absolutePath, output.absolutePath)
                val root = Json.parseToJsonElement(raw).jsonObject
                if (root["ok"]?.jsonPrimitive?.boolean != true) {
                    val reason = root["error"]?.jsonPrimitive?.content ?: "unknown"
                    journal.record(
                        ForumEvent.REPORT_COLLECT,
                        JournalField.Class("failed"),
                        JournalField.Reason(collectReasonToken(reason)),
                    )
                    error("collect failed: $reason")
                }
                val bytes = root["bytes"]?.jsonPrimitive?.long ?: output.length()
                journal.record(ForumEvent.REPORT_COLLECT, JournalField.Class("ok"), JournalField.Bytes(bytes))
                CollectedReport(file = output, bytes = bytes)
            }
        }

    override suspend fun submit(form: ReportForm, report: CollectedReport?): ReportSubmitOutcome {
        if (walletRepository.state.value is WalletState.Absent) {
            journal.record(ForumEvent.REPORT_SUBMIT, JournalField.Class("wallet-absent"))
            return ReportSubmitOutcome.WalletNotReady
        }
        // The logs are sized before the wallet is touched: a refusal at the cap
        // reads nothing and reaches nothing.
        val logGz =
            try {
                withContext(Dispatchers.IO) { report?.let { compressor.compress(it.file) } }
            } catch (e: Exception) {
                Logger.w(throwable = e) { "WarrenSupportReporter: gzip failed" }
                journal.record(ForumEvent.REPORT_SUBMIT, JournalField.Class("gzip"))
                return ReportSubmitOutcome.Failure("gzip")
            }
        if (logGz != null && logGz.size > MAX_LOG_GZ_BYTES) {
            journal.record(
                ForumEvent.REPORT_SUBMIT,
                JournalField.Class("too-large"),
                JournalField.GzBytes(logGz.size.toLong()),
            )
            return ReportSubmitOutcome.TooLarge
        }
        val mnemonic =
            try {
                walletRepository.readMnemonic()
            } catch (e: Exception) {
                Logger.e(throwable = e) { "WarrenSupportReporter: mnemonic read failed" }
                journal.record(ForumEvent.REPORT_SUBMIT, JournalField.Class("wallet-read"))
                return ReportSubmitOutcome.Failure("wallet-read")
            }
        return withContext(Dispatchers.IO) {
            val started = System.currentTimeMillis()
            val outcome =
                mnemonic.use { m ->
                    val raw =
                        try {
                            jni.forumReport(m.phrase, reportJson(form), logGz)
                        } catch (e: Exception) {
                            Logger.e(throwable = e) { "WarrenJni.forumReport threw" }
                            journal.record(ForumEvent.REPORT_SUBMIT, JournalField.Class("jni"))
                            return@use ReportSubmitOutcome.Failure("jni")
                        }
                    parseReportOutcome(raw)
                }
            journal.record(
                ForumEvent.REPORT_SUBMIT,
                JournalField.Class(reportOutcomeClass(outcome)),
                JournalField.ElapsedMs(System.currentTimeMillis() - started),
                JournalField.WithLogs(logGz != null),
            )
            if (outcome is ReportSubmitOutcome.Created) {
                outcome.identity?.let(forumIdentityRepository::save)
            }
            outcome
        }
    }

    override fun discard(report: CollectedReport) {
        if (!report.file.delete()) {
            Logger.w("WarrenSupportReporter: could not delete a discarded report")
        }
    }

    /** The connect contract's body, without the log field Rust adds. */
    private fun reportJson(form: ReportForm): String {
        val fields =
            linkedMapOf<String, JsonPrimitive>(
                "platform" to JsonPrimitive("android"),
                "area" to JsonPrimitive(form.area.token),
                "frequency" to JsonPrimitive(form.frequency.token),
                "what_happened" to JsonPrimitive(form.whatHappened.trim()),
                "app_version" to JsonPrimitive(BuildConfig.VERSION_NAME),
                "os_version" to
                    JsonPrimitive(
                        "Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT}) ${Build.MANUFACTURER} ${Build.MODEL}"
                            .take(80)
                    ),
                "locale" to JsonPrimitive(Locale.getDefault().language),
            )
        form.steps?.trim()?.takeIf { it.isNotEmpty() }?.let { fields["steps"] = JsonPrimitive(it) }
        return JsonObject(fields).toString()
    }

    private fun buildJsonArray(items: List<String>): String =
        kotlinx.serialization.json.JsonArray(items.map { JsonPrimitive(it) }).toString()

    companion object {
        /** Largest gzip the app agrees to send; tracks the desktop's `MAX_LOG_GZ_BYTES`. */
        const val MAX_LOG_GZ_BYTES = 12 * 1024 * 1024
    }
}

/**
 * The io error kind of a collector failure as a journal token: the Rust
 * envelope says `<step>: <Kind>`, and the step is already the class.
 */
internal fun collectReasonToken(reason: String): String =
    reason.substringAfterLast(':').trim().lowercase().ifEmpty { "unknown" }

/** The coarse class of an outcome, for the log and the journal. */
internal fun reportOutcomeClass(outcome: ReportSubmitOutcome): String =
    when (outcome) {
        is ReportSubmitOutcome.Created -> "created-${outcome.logs}"
        ReportSubmitOutcome.SubscriptionRequired -> "subscription-required"
        ReportSubmitOutcome.ClockSkew -> "clock-skew"
        ReportSubmitOutcome.RateLimited -> "rate-limited"
        ReportSubmitOutcome.TooLarge -> "too-large"
        ReportSubmitOutcome.Invalid -> "invalid"
        ReportSubmitOutcome.ServerError -> "server-error"
        ReportSubmitOutcome.WalletNotReady -> "wallet-absent"
        is ReportSubmitOutcome.Deferred -> "deferred-${outcome.tunnelClass}"
        is ReportSubmitOutcome.Failure -> outcome.reason
    }

/**
 * Map the `warren_forum::report_envelope` JSON to an outcome. Pure, so it is
 * unit-testable off-device; the tokens are the frozen contract with Rust.
 */
internal fun parseReportOutcome(rawJson: String): ReportSubmitOutcome =
    try {
        val root = Json.parseToJsonElement(rawJson).jsonObject
        if (root["ok"]?.jsonPrimitive?.boolean == true) {
            val handle = root["handle"]?.jsonPrimitive?.content
            ReportSubmitOutcome.Created(
                topicId = root["topic_id"]?.jsonPrimitive?.long ?: 0L,
                topicUrl = root["topic_url"]?.jsonPrimitive?.content ?: "",
                identity =
                    handle?.let {
                        ForumIdentity(handle = it, notifySlot = root["notify_slot"]?.jsonPrimitive?.int)
                    },
                logs = root["logs"]?.jsonPrimitive?.content ?: "none",
            )
        } else {
            when (root["error"]?.jsonPrimitive?.content) {
                "subscription-required" -> ReportSubmitOutcome.SubscriptionRequired
                "clock-skew" -> ReportSubmitOutcome.ClockSkew
                "rate-limited" -> ReportSubmitOutcome.RateLimited
                "too-large" -> ReportSubmitOutcome.TooLarge
                "invalid" -> ReportSubmitOutcome.Invalid
                "server-error" -> ReportSubmitOutcome.ServerError
                else ->
                    ReportSubmitOutcome.Failure(
                        root["reason"]?.jsonPrimitive?.content?.takeIf { it.isNotBlank() }
                            ?: "unknown"
                    )
            }
        }
    } catch (e: Exception) {
        ReportSubmitOutcome.Failure("invalid-envelope")
    }
