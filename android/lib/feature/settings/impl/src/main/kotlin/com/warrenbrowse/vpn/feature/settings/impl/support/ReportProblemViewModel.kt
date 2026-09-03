package com.warrenbrowse.vpn.feature.settings.impl.support

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.warrenbrowse.vpn.lib.repository.CollectedReport
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.ReportArea
import com.warrenbrowse.vpn.lib.repository.ReportForm
import com.warrenbrowse.vpn.lib.repository.ReportFrequency
import com.warrenbrowse.vpn.lib.repository.ReportSubmitOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenSupportReporter
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Shortest description the forum accepts; the broker refuses less. */
const val REPORT_MIN_DESCRIPTION_CHARS = 20
/** Longest description, and longest steps, the broker accepts. */
const val REPORT_MAX_DESCRIPTION_CHARS = 4_000

/**
 * How far past the cap the fields still take keys: the counter then shows the
 * overrun to trim instead of the field silently refusing to type.
 */
private const val REPORT_OVERRUN_CHARS = 200

/** What the report screen shows. */
data class ReportProblemUiState(
    val area: ReportArea? = null,
    val frequency: ReportFrequency? = null,
    val whatHappened: String = "",
    val steps: String = "",
    val includeLogs: Boolean = true,
    /** The redacted report collected for the preview or the send, if any. */
    val collected: CollectedReport? = null,
    val collecting: Boolean = false,
    val collectFailed: Boolean = false,
    val sending: Boolean = false,
    val outcome: ReportSubmitOutcome? = null,
) {
    val descriptionChars: Int
        get() = whatHappened.trim().length

    val stepsChars: Int
        get() = steps.trim().length

    /** The form as the broker takes it, or null while [canSend] is false. */
    fun formIfSendable(): ReportForm? =
        if (canSend && area != null && frequency != null) {
            ReportForm(
                area = area,
                frequency = frequency,
                whatHappened = whatHappened.trim(),
                steps = steps.trim().ifEmpty { null },
            )
        } else {
            null
        }

    val canSend: Boolean
        get() =
            area != null &&
                frequency != null &&
                descriptionChars in REPORT_MIN_DESCRIPTION_CHARS..REPORT_MAX_DESCRIPTION_CHARS &&
                // The broker measures `steps` against the same cap. Left
                // unchecked, an over-cap steps field spent the whole upload and
                // came back as a 422 whose notice names the description, the
                // field that was within bounds.
                stepsChars <= REPORT_MAX_DESCRIPTION_CHARS &&
                !sending &&
                !collecting &&
                outcome !is ReportSubmitOutcome.Created
}

class ReportProblemViewModel(private val reporter: WarrenSupportReporter) : ViewModel() {
    private val _state = MutableStateFlow(ReportProblemUiState())
    val state: StateFlow<ReportProblemUiState> = _state.asStateFlow()

    fun setArea(area: ReportArea) = _state.update { it.copy(area = area, outcome = null) }

    fun setFrequency(frequency: ReportFrequency) =
        _state.update { it.copy(frequency = frequency, outcome = null) }

    fun setWhatHappened(text: String) =
        _state.update {
            it.copy(whatHappened = text.take(REPORT_MAX_DESCRIPTION_CHARS + REPORT_OVERRUN_CHARS), outcome = null)
        }

    fun setSteps(text: String) =
        _state.update {
            it.copy(steps = text.take(REPORT_MAX_DESCRIPTION_CHARS + REPORT_OVERRUN_CHARS), outcome = null)
        }

    fun setIncludeLogs(include: Boolean) {
        val collected = _state.value.collected
        if (!include && collected != null) {
            reporter.discard(collected)
        }
        _state.update {
            it.copy(includeLogs = include, collected = if (include) it.collected else null, outcome = null)
        }
    }

    /**
     * Collects the report so the user can read it (the desktop "View the
     * logs"). Returns through [onReady] with the file to show. Nothing leaves
     * the device for it: the live probes are taken by the send's own
     * collection, not here.
     */
    fun collect(onReady: (CollectedReport) -> Unit = {}) {
        if (_state.value.collecting) return
        _state.update { it.copy(collecting = true, collectFailed = false) }
        viewModelScope.launch {
            val previous = _state.value.collected
            val result = reporter.collect(forSend = false)
            result.fold(
                onSuccess = { report ->
                    previous?.let(reporter::discard)
                    _state.update { it.copy(collecting = false, collected = report) }
                    onReady(report)
                },
                onFailure = { _state.update { it.copy(collecting = false, collectFailed = true) } },
            )
        }
    }

    fun send() {
        val s = _state.value
        val form = s.formIfSendable() ?: return
        // Before anything is collected: a tunnel between states cannot reach
        // the broker, and the user is told so with the form left intact.
        val preflight = reporter.preflight()
        if (preflight is ForumPreflight.Defer) {
            _state.update { it.copy(outcome = ReportSubmitOutcome.Deferred(preflight.tunnelClass)) }
        } else {
            _state.update { it.copy(sending = true, outcome = null) }
            sendCollected(s, form)
        }
    }

    private fun sendCollected(s: ReportProblemUiState, form: ReportForm) {
        viewModelScope.launch {
            // A send with logs always collects afresh: the report should describe
            // the moment of the send, not the moment the screen was opened, and
            // this is the one collection that probes the network.
            val report =
                if (s.includeLogs) {
                    reporter.collect(forSend = true).getOrNull().also { fresh ->
                        if (fresh != null) s.collected?.let(reporter::discard)
                    }
                } else {
                    null
                }
            val outcome = reporter.submit(form, report)
            report?.let(reporter::discard)
            _state.update {
                it.copy(
                    sending = false,
                    collected = if (report != null) null else it.collected,
                    outcome = outcome,
                )
            }
        }
    }

    /** Sends again without the logs after a size refusal. */
    fun sendWithoutLogs() {
        setIncludeLogs(false)
        send()
    }

    override fun onCleared() {
        _state.value.collected?.let(reporter::discard)
        super.onCleared()
    }
}
