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

// D.6 step 65 + audit follow-up: single-variant tag retained as a
// `sealed interface` (instead of `data object`) so a future revival
// of distinct send-failure causes (network / signature / payload-too-
// large / ...) can re-extend it without rewriting the UI state shape.
// The previous `Success` + `Error.CollectLog` variants were dead:
// `Success` was never emitted (the ViewModel projects
// `WarrenSupportReportOutcome.Success` directly to its UI state),
// `CollectLog` was unreachable from the live flow (the legacy
// repository.sendReport that emitted it was removed in step 65).
sealed interface SendProblemReportResult {
    sealed interface Error : SendProblemReportResult {
        data object SendReport : Error
    }
}

// D.6 step 65: ProblemReportRepository slimmed to the local log surface
// (collect / read / delete). The send path moved to
// WarrenSupportReportInvoker which routes through WarrenJni's signed POST
// to /v1/support. The repository keeps the StateFlow<UserReport> form
// state + the log file lifecycle (UI continues to call setEmail /
// setDescription / collectLogs / readLogs / deleteLogs).
//
// Audit follow-up: previous version carried three constructor params
// (apiEndpointOverride / apiEndpointFromIntentHolder / kermitFileLogDirName)
// that became dead after the send path moved. Dropped along with the
// redundant `System.loadLibrary("warren_jni")` (WarrenJni's own static
// init already loads the library; calling it again here only worked by
// load-order coincidence). The JNI surface is now consumed through the
// injected [WarrenJniBridge] instead of importing
// `com.warrenbrowse.vpn.jni.WarrenJni` directly from the `:app` module.
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
                // D.6: WarrenJniBridge.collectReport returns the redacted
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
