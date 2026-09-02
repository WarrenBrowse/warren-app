package com.warrenbrowse.vpn.app.forum

import co.touchlab.kermit.Logger
import java.io.File
import java.time.Instant
import java.util.concurrent.Executors
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/** The steps of the forum flows a journal line can name. Closed: never built from data. */
enum class ForumEvent(val token: String) {
    LINK_RECEIVED("link.received"),
    LOGIN_DEFERRED("login.deferred"),
    LOGIN_SIGNING("login.signing"),
    LOGIN_RESULT("login.result"),
    LOGIN_DECLINED("login.declined"),
    REPORT_DEFERRED("report.deferred"),
    REPORT_COLLECT("report.collect"),
    REPORT_SUBMIT("report.submit"),
}

/** Where an accepted sign-in link came from. */
enum class LinkSource(val token: String) {
    DEEP_LINK("deep-link"),
    TYPED_CODE("typed-code"),
}

/**
 * One field of a journal line. The key set is closed and every value is a
 * number, a flag, or a token checked against a grammar shorter than a session
 * id, so no call site has a field to put a sid, an address or a handle in.
 */
sealed interface JournalField {
    val key: String
    val value: String

    /** The class of an outcome or a deferral. */
    data class Class(val token: String) : JournalField {
        override val key = "class"
        override val value = classToken(token)
    }

    /** Why a step failed, as a class token. */
    data class Reason(val token: String) : JournalField {
        override val key = "reason"
        override val value = classToken(token)
    }

    /** The verdict on a sign-in link. */
    data class Verdict(val token: String) : JournalField {
        override val key = "verdict"
        override val value = classToken(token)
    }

    data class Source(val source: LinkSource) : JournalField {
        override val key = "source"
        override val value = source.token
    }

    /** The host of the intent referrer: a package or a web host, never a session id. */
    data class Referrer(val host: String?) : JournalField {
        override val key = "referrer"
        override val value = host?.let(::hostToken) ?: NONE
    }

    data class CrossDevice(val flag: Boolean) : JournalField {
        override val key = "cross_device"
        override val value = flag.toString()
    }

    data class ColdStart(val flag: Boolean) : JournalField {
        override val key = "cold_start"
        override val value = flag.toString()
    }

    data class WithLogs(val flag: Boolean) : JournalField {
        override val key = "with_logs"
        override val value = flag.toString()
    }

    data class ElapsedMs(val count: Long) : JournalField {
        override val key = "elapsed_ms"
        override val value = count.toString()
    }

    data class Bytes(val count: Long) : JournalField {
        override val key = "bytes"
        override val value = count.toString()
    }

    data class GzBytes(val count: Long) : JournalField {
        override val key = "gz_bytes"
        override val value = count.toString()
    }

    companion object {
        const val NONE = "none"
        const val MALFORMED = "malformed"

        /**
         * A class token is lowercase words joined by hyphens or a colon, at most
         * [MAX_TOKEN_CHARS] long: shorter than a session id (32 hex characters)
         * and an SS58 address (49), so neither fits even through a wrong call
         * site. Anything else is journaled as [MALFORMED].
         */
        const val MAX_TOKEN_CHARS = 24
        private val CLASS_TOKEN = Regex("[a-z][a-z0-9]*(?:[-:][a-z0-9]+)*")
        private val HOST_TOKEN = Regex("[a-z0-9_-]+(?:\\.[a-z0-9_-]+)+")
        private const val MAX_HOST_CHARS = 64

        fun classToken(token: String): String =
            if (token.length <= MAX_TOKEN_CHARS && CLASS_TOKEN.matches(token)) token else MALFORMED

        /** A dotted host or package name; a bare token (a session id has no dot) is malformed. */
        fun hostToken(host: String): String =
            if (host.length <= MAX_HOST_CHARS && HOST_TOKEN.matches(host)) host else MALFORMED
    }
}

/** The journal the forum flows write to; [ForumEventsJournal] is the file, a test fakes it. */
interface ForumJournal {
    fun record(event: ForumEvent, vararg fields: JournalField)

    /** The `class` of the last journaled [event], or null when none was journaled. */
    suspend fun lastClassOf(event: ForumEvent): String?
}

/**
 * The forum flows' event journal: one JSON line per step of every sign-in
 * attempt (link received and its verdict, prompt shown, approved, declined,
 * signed, the outcome class and how long it took), in the Kotlin log
 * directory so the problem-report collector carries it like any other log.
 *
 * It exists because the app's rolling logs are filled by the tunnel first,
 * and the one thing a report about "the forum sign-in does not work" needs
 * is the history of the attempts. Nothing identifying is ever written: the
 * fields are [JournalField]s, whose types leave no room for a sid, an
 * address or a handle.
 */
class ForumEventsJournal(private val logDir: File, private val scope: CoroutineScope) :
    ForumJournal {
    private val dispatcher = Executors.newSingleThreadExecutor().asCoroutineDispatcher()
    private var sequence = 0L

    override fun record(event: ForumEvent, vararg fields: JournalField) {
        val line = format(Instant.now(), sequence++, event, fields.toList())
        Logger.i("forum: ${event.token} ${fields.joinToString(" ") { "${it.key}=${it.value}" }}")
        scope.launch(dispatcher) {
            try {
                logDir.mkdirs()
                val file = File(logDir, FILE_NAME)
                if (file.length() > MAX_BYTES) truncateHead(file)
                file.appendText(line + "\n")
            } catch (e: Exception) {
                Logger.w(throwable = e) { "forum events journal write failed" }
            }
        }
    }

    /**
     * Sequenced behind the pending writes on the journal's own thread, so a
     * record followed by a readback sees the record. The report header reads
     * the last `login.result` this way: the staff read the header before the
     * logs, and what the last sign-in attempt said is its most useful line.
     */
    override suspend fun lastClassOf(event: ForumEvent): String? =
        withContext(dispatcher) {
            val file = File(logDir, FILE_NAME)
            if (!file.isFile) return@withContext null
            try {
                file.useLines { lines -> lines.mapNotNull { classOf(it, event) }.lastOrNull() }
            } catch (e: Exception) {
                Logger.w(throwable = e) { "forum events journal read failed" }
                null
            }
        }

    /** Keeps the newest half of the file: the history that matters is recent. */
    private fun truncateHead(file: File) {
        val lines = file.readLines()
        file.writeText(lines.drop(lines.size / 2).joinToString("\n", postfix = "\n"))
    }

    companion object {
        const val FILE_NAME = "warren-events.log"
        const val MAX_BYTES = 256L * 1024L

        /** One journal line: pure, so the shape is unit-testable. */
        fun format(
            at: Instant,
            sequence: Long,
            event: ForumEvent,
            fields: List<JournalField>,
        ): String {
            val entries =
                linkedMapOf<String, JsonPrimitive>(
                    "seq" to JsonPrimitive(sequence),
                    "at" to JsonPrimitive(at.toString()),
                    "event" to JsonPrimitive(event.token),
                )
            for (field in fields) entries[field.key] = JsonPrimitive(field.value)
            return JsonObject(entries).toString()
        }

        /**
         * The `class` of one journal line when it is an [event] line, else
         * null. A line that does not parse (a process killed mid-write) is
         * skipped rather than failing the readback.
         */
        fun classOf(line: String, event: ForumEvent): String? =
            try {
                val entry = Json.parseToJsonElement(line).jsonObject
                if (entry["event"]?.jsonPrimitive?.content == event.token) {
                    entry["class"]?.jsonPrimitive?.content
                } else {
                    null
                }
            } catch (e: Exception) {
                null
            }
    }
}
