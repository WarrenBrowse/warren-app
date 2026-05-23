package com.warrenbrowse.vpn.feature.problemreport.impl

import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.async
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.WhileSubscribed
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.common.compose.MINIMUM_LOADING_TIME_MILLIS
import com.warrenbrowse.vpn.lib.common.constant.VIEW_MODEL_STOP_TIMEOUT
import com.warrenbrowse.vpn.lib.common.util.combine
import com.warrenbrowse.vpn.lib.repository.ProblemReportRepository
import com.warrenbrowse.vpn.lib.repository.SendProblemReportResult
import com.warrenbrowse.vpn.lib.repository.WarrenSupportReportInvoker
import com.warrenbrowse.vpn.lib.repository.WarrenSupportReportOutcome

// D.4 step 57: Mullvad "include my account ID" checkbox surface removed.
// D.6: send path rewired to WarrenSupportReportInvoker (biometric unlock +
// signed POST /v1/support via warren-api-client). ProblemReportRepository
// keeps the local log-collection surface only.
data class ReportProblemUiState(
    val sendingState: SendingReportUiState? = null,
    val email: String = "",
    val description: String = "",
    val descriptionError: DescriptionError? = null,
    val logCollectingState: LogCollectingState = LogCollectingState.Loading,
    val isPlayBuild: Boolean = false,
)

sealed interface SendingReportUiState {
    data object Sending : SendingReportUiState

    data class Success(val email: String?) : SendingReportUiState

    data class Error(val error: SendProblemReportResult.Error) : SendingReportUiState
}

sealed interface LogCollectingState {
    data object Loading : LogCollectingState

    data object Success : LogCollectingState

    data object Failed : LogCollectingState
}

sealed interface ReportProblemSideEffect {
    data object ShowConfirmNoEmail : ReportProblemSideEffect
}

sealed interface DescriptionError {
    data object Empty : DescriptionError
}

// Audit follow-up: the previous constructor took two ProblemReportRepository
// parameters resolved from the same Koin singleton — a latent footgun for
// any future split. Collapsed to a single parameter; all log-lifecycle calls
// now go through `problemReportRepository`.
class ReportProblemViewModel(
    private val problemReportRepository: ProblemReportRepository,
    private val isPlayBuild: Boolean,
    private val supportReportInvoker: WarrenSupportReportInvoker,
) : ViewModel() {

    private val sendingState: MutableStateFlow<SendingReportUiState?> = MutableStateFlow(null)
    private val areLogsCollected: MutableStateFlow<LogCollectingState> =
        MutableStateFlow(LogCollectingState.Loading)
    private val descriptionError: MutableStateFlow<DescriptionError?> = MutableStateFlow(null)

    val uiState =
        combine(
                sendingState,
                problemReportRepository.problemReport,
                areLogsCollected,
                descriptionError,
            ) { sendingState, userReport, areLogsCollected, descriptionError ->
                ReportProblemUiState(
                    sendingState = sendingState,
                    email = userReport.email ?: "",
                    description = userReport.description,
                    logCollectingState = areLogsCollected,
                    isPlayBuild = isPlayBuild,
                    descriptionError = descriptionError,
                )
            }
            .stateIn(
                viewModelScope,
                SharingStarted.WhileSubscribed(VIEW_MODEL_STOP_TIMEOUT),
                ReportProblemUiState(isPlayBuild = isPlayBuild),
            )

    private val _uiSideEffect = Channel<ReportProblemSideEffect>()
    val uiSideEffect = _uiSideEffect.receiveAsFlow()

    fun sendReport(
        activity: FragmentActivity,
        email: String,
        description: String,
        skipEmptyEmailCheck: Boolean = false,
    ) {
        viewModelScope.launch {
            if (description.isBlank()) {
                descriptionError.emit(DescriptionError.Empty)
                return@launch
            }

            val userEmail = email.trim()
            val nullableEmail = if (email.isEmpty()) null else userEmail
            if (!skipEmptyEmailCheck && shouldShowConfirmNoEmail(nullableEmail)) {
                _uiSideEffect.send(ReportProblemSideEffect.ShowConfirmNoEmail)
            } else {
                sendingState.emit(SendingReportUiState.Sending)

                val redactedLogs = problemReportRepository.readLogs().joinToString("\n")
                val userMessage = composeUserMessage(nullableEmail, description)

                val deferredResult = async {
                    supportReportInvoker.submit(activity, userMessage, redactedLogs)
                }
                delay(MINIMUM_LOADING_TIME_MILLIS)
                val outcome = deferredResult.await()

                if (outcome is WarrenSupportReportOutcome.Success) {
                    problemReportRepository.setEmail("")
                    problemReportRepository.setDescription("")
                    problemReportRepository.deleteLogs()
                }
                sendingState.tryEmit(outcome.toUiResult(nullableEmail))
            }
        }
    }

    fun clearSendResult() {
        sendingState.tryEmit(null)
    }

    fun updateEmail(email: String) {
        problemReportRepository.setEmail(email)
    }

    fun updateDescription(description: String) {
        problemReportRepository.setDescription(description)
        descriptionError.tryEmit(null)
    }

    private fun shouldShowConfirmNoEmail(userEmail: String?): Boolean =
        userEmail.isNullOrEmpty() && uiState.value.sendingState !is SendingReportUiState

    // D.6: the warren-api /v1/support endpoint has a single free-form
    // user_message field. We prepend the email when supplied so the
    // operator can correlate; the wallet pubkey (= signer identity) is
    // already carried in the auth header.
    private fun composeUserMessage(email: String?, description: String): String =
        if (email.isNullOrEmpty()) {
            description
        } else {
            "Reply-to: $email\n\n$description"
        }

    private fun WarrenSupportReportOutcome.toUiResult(email: String?): SendingReportUiState =
        when (this) {
            is WarrenSupportReportOutcome.Success -> SendingReportUiState.Success(email)
            // D.6: collapse all server-side / network / auth failures to
            // the legacy SendReport error so the UI continues to render
            // the existing error sheet without bespoke per-cause copy.
            else -> SendingReportUiState.Error(SendProblemReportResult.Error.SendReport)
        }

    init {
        viewModelScope.launch {
            if (problemReportRepository.collectLogs()) {
                areLogsCollected.emit(LogCollectingState.Success)
            } else {
                areLogsCollected.emit(LogCollectingState.Failed)
            }
        }
    }

    override fun onCleared() {
        super.onCleared()
        // Delete any logs if user leaves the screen
        problemReportRepository.deleteLogs()
    }
}
