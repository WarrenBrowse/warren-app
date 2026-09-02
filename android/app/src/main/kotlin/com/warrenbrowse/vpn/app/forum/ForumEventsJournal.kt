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

/**
 * The forum flows' event journal: one JSON line per step of every sign-in
 * attempt (link received and its verdict, prompt shown, approved, declined,
 * signed, the outcome class and how long it took), in the Kotlin log
 * directory so the problem-report collector carries it like any other log.
 *
 * It exists because the app's rolling logs are filled by the tunnel first,
 * and the one thing a report about "the forum sign-in does not work" needs
 * is the history of the attempts. Nothing identifying is ever written: no
 * sid, no address, no handle; only event names, classes and durations.
 */
class ForumEventsJournal(private val logDir: File, private val scope: CoroutineScope) {
    private val dispatcher = Executors.newSingleThreadExecutor().asCoroutineDispatcher()
    private var sequence = 0L

    fun record(event: String, vararg fields: Pair<String, String>) {
        val line = format(Instant.now(), sequence++, event, fields.toList())
        Logger.i("forum: $event ${fields.joinToString(" ") { "${it.first}=${it.second}" }}")
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
     * The `class` of the last journaled [event], or null when none was
     * journaled. Sequenced behind the pending writes on the journal's own
     * thread, so a record followed by a readback sees the record. The report
     * header reads the last `login.result` this way: the staff read the header
     * before the logs, and what the last sign-in attempt said is its most
     * useful line.
     */
    suspend fun lastClassOf(event: String): String? =
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
            event: String,
            fields: List<Pair<String, String>>,
        ): String {
            val entries =
                linkedMapOf<String, JsonPrimitive>(
                    "seq" to JsonPrimitive(sequence),
                    "at" to JsonPrimitive(at.toString()),
                    "event" to JsonPrimitive(event),
                )
            for ((key, value) in fields) entries[key] = JsonPrimitive(value)
            return JsonObject(entries).toString()
        }

        /**
         * The `class` of one journal line when it is an [event] line, else
         * null. A line that does not parse (a process killed mid-write) is
         * skipped rather than failing the readback.
         */
        fun classOf(line: String, event: String): String? =
            try {
                val entry = Json.parseToJsonElement(line).jsonObject
                if (entry["event"]?.jsonPrimitive?.content == event) {
                    entry["class"]?.jsonPrimitive?.content
                } else {
                    null
                }
            } catch (e: Exception) {
                null
            }
    }
}
