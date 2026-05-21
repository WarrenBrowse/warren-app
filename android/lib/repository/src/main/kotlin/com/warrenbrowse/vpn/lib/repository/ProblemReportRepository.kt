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
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointFromIntentHolder
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointOverride
import com.warrenbrowse.vpn.lib.model.UserReport
import com.warrenbrowse.vpn.lib.payment.model.PaymentStatus

const val PROBLEM_REPORT_LOGS_FILE = "problem_report.txt"

sealed interface SendProblemReportResult {
    data object Success : SendProblemReportResult

    sealed interface Error : SendProblemReportResult {
        data object CollectLog : Error

        // This is usually due to network error or bad email address
        data object SendReport : Error
    }
}

class ProblemReportRepository(
    context: Context,
    private val apiEndpointOverride: ApiEndpointOverride?,
    private val apiEndpointFromIntentHolder: ApiEndpointFromIntentHolder,
    private val accountRepository: AccountRepository,
    kermitFileLogDirName: String,
    private val paymentLogic: PaymentLogic,
    val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) {
    init {
        System.loadLibrary("warren_jni")
    }

    private val _problemReport = MutableStateFlow(UserReport("", ""))
    val problemReport: StateFlow<UserReport> = _problemReport.asStateFlow()
    private val cacheDirectory = File(context.cacheDir.toURI())
    private val logDirectory = File(context.filesDir.toURI())
    private val problemReportOutputPath = File(logDirectory, PROBLEM_REPORT_LOGS_FILE)
    private val kermitFileLogDirPath = File(logDirectory, kermitFileLogDirName)
    private val collectReportMutex = Mutex()

    fun setEmail(email: String) = _problemReport.update { it.copy(email = email) }

    fun setDescription(description: String) = _problemReport.update {
        it.copy(description = description)
    }

    suspend fun collectLogs(): Boolean =
        withContext(dispatcher) {

            // Lock to avoid potential truncation of the log file that the daemon creates
            collectReportMutex.withLock {
                // Delete any old report
                deleteLogs()

                val availableProducts = paymentLogic.allAvailableProducts()

                collectReport(
                    logDirectory = logDirectory.absolutePath,
                    kermitFileLogDir = kermitFileLogDirPath.absolutePath,
                    problemReportOutputPath = problemReportOutputPath.absolutePath,
                    unverifiedPurchases =
                        availableProducts?.count {
                            it.status == PaymentStatus.VERIFICATION_IN_PROGRESS
                        } ?: 0,
                    pendingPurchases =
                        availableProducts?.count { it.status == PaymentStatus.PENDING } ?: 0,
                )
            }
        }

    suspend fun sendReport(
        userReport: UserReport,
        includeAccountId: Boolean,
    ): SendProblemReportResult {
        // If report is not collected then, collect it, if it fails then return error
        if (!logsExists() && !collectLogs()) {
            return SendProblemReportResult.Error.CollectLog
        }

        val sentSuccessfully =
            withContext(dispatcher) {
                val intentApiOverride = apiEndpointFromIntentHolder.apiEndpointOverride
                val apiOverride =
                    if (BuildConfig.DEBUG && intentApiOverride != null) {
                        intentApiOverride
                    } else {
                        apiEndpointOverride
                    }

                sendProblemReport(
                    userEmail = userReport.email ?: "",
                    userMessage = userReport.description,
                    accountId =
                        if (includeAccountId) {
                            accountRepository.accountData.value?.id?.value?.toString()
                        } else {
                            null
                        },
                    reportPath = problemReportOutputPath.absolutePath,
                    cacheDirectory = cacheDirectory.absolutePath,
                    apiEndpointOverride = apiOverride,
                )
            }

        return if (sentSuccessfully) {
            deleteLogs()
            SendProblemReportResult.Success
        } else {
            SendProblemReportResult.Error.SendReport
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

    // Warren-specific problem report path is unimplemented (D.6 follow-up).
    // The upstream `mullvad-problem-report` Rust crate was dropped from
    // warren-jni during D.3 and the Warren-native equivalent (collect a
    // tarball of Kotlin + Rust logs, POST to warren-api `/v1/support`) has
    // not landed yet. These stubs let the repository link without an
    // `UnsatisfiedLinkError` at runtime and signal failure cleanly to
    // callers, who already handle `SendProblemReportResult.Error.*`.
    @Suppress("UnusedPrivateMember")
    private fun collectReport(
        logDirectory: String,
        kermitFileLogDir: String,
        problemReportOutputPath: String,
        unverifiedPurchases: Int,
        pendingPurchases: Int,
    ): Boolean {
        // TODO (D.6): wire warren-side problem-report collector.
        return false
    }

    @Suppress("UnusedPrivateMember")
    private fun sendProblemReport(
        userEmail: String,
        userMessage: String,
        accountId: String?,
        reportPath: String,
        cacheDirectory: String,
        apiEndpointOverride: ApiEndpointOverride?,
    ): Boolean {
        // TODO (D.6): POST report to warren-api `/v1/support` with the
        // signed canonical request flow (see warren-jni signCanonicalRequest).
        return false
    }
}
