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

// D.6 step 65 + audit follow-up: only `Error.SendReport` is still
// produced by the live path. `Success` is unused (the ViewModel
// projects WarrenSupportReportOutcome.Success directly to its
// SendingReportUiState.Success). `Error.CollectLog` is unreachable
// from any live code path (the previous repository.sendReport
// flow that emitted it was removed). Kept only for the
// PreviewParameterProvider's dummy state until the screen previews
// are reworked.
sealed interface SendProblemReportResult {
    data object Success : SendProblemReportResult

    sealed interface Error : SendProblemReportResult {
        data object CollectLog : Error
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
// load-order coincidence).
class ProblemReportRepository(
    context: Context,
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
                // D.6: WarrenJni.collectReport returns the redacted bundle
                // as a single string (empty until the file-appender lands).
                // We mirror it into the legacy on-disk path so the existing
                // ViewLogs screen + readLogs API continue to function.
                val bundle = runCatching { com.warrenbrowse.vpn.jni.WarrenJni.collectReport() }
                    .getOrElse { "" }
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
