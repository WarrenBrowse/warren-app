@file:JvmName("ProblemReportRepositoryKt")

package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import java.io.File
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import com.warrenbrowse.vpn.lib.model.UserReport

const val PROBLEM_REPORT_LOGS_FILE = "problem_report.txt"

// Modelled as a `sealed interface` (rather than a single `data object`) so
// distinct send-failure causes (network / signature / payload-too-large /
// ...) can extend it later without reshaping the UI state.
sealed interface SendProblemReportResult {
    sealed interface Error : SendProblemReportResult {
        data object SendReport : Error
    }
}

// Owns the local log surface (collect / read / delete) and the
// StateFlow<UserReport> form state; the UI calls setEmail / setDescription /
// collectLogs / readLogs / deleteLogs. The send path lives in
// WarrenSupportReportInvoker, which routes through WarrenJni's signed POST to
// /v1/support. The JNI surface is consumed through the injected
// [WarrenJniBridge] rather than importing `com.warrenbrowse.vpn.jni.WarrenJni`
// directly from the `:app` module.
class ProblemReportRepository(
    context: Context,
    private val jni: WarrenJniBridge,
    val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) {
    private val _problemReport = MutableStateFlow(UserReport("", ""))
    val problemReport: StateFlow<UserReport> = _problemReport.asStateFlow()
    private val logDirectory = File(context.filesDir.toURI())
    private val problemReportOutputPath = File(logDirectory, PROBLEM_REPORT_LOGS_FILE)
    private val collectReportMutex = Mutex()

    fun setEmail(email: String) = _problemReport.update { it.copy(email = email) }

    fun setDescription(description: String) = _problemReport.update {
        it.copy(description = description)
    }

    suspend fun collectLogs(): Boolean =
        withContext(dispatcher) {
            collectReportMutex.withLock {
                deleteLogs()
                // WarrenJniBridge.collectReport returns the redacted
                // bundle as a single string (empty until the file-appender
                // lands). We mirror it into the legacy on-disk path so the
                // existing ViewLogs screen + readLogs API continue to
                // function.
                val bundle = runCatching { jni.collectReport() }.getOrElse { "" }
                try {
                    problemReportOutputPath.writeText(bundle)
                    true
                } catch (e: Exception) {
                    false
                }
            }
        }

    suspend fun readLogs(): List<String> {
        if (!logsExists()) {
            collectLogs()
        }

        return if (logsExists()) {
            problemReportOutputPath.readLines()
        } else {
            listOf("Failed to collect logs for problem report")
        }
    }

    private fun logsExists() = problemReportOutputPath.exists()

    fun deleteLogs() {
        problemReportOutputPath.delete()
    }
}
