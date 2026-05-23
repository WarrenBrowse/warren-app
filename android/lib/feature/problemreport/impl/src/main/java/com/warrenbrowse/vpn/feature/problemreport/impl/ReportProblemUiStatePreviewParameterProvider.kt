package com.warrenbrowse.vpn.feature.problemreport.impl

import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import com.warrenbrowse.vpn.lib.repository.SendProblemReportResult

class ReportProblemUiStatePreviewParameterProvider :
    PreviewParameterProvider<ReportProblemUiState> {
    override val values: Sequence<ReportProblemUiState>
        get() =
            sequenceOf(
                ReportProblemUiState(),
                ReportProblemUiState(sendingState = SendingReportUiState.Sending),
                ReportProblemUiState(sendingState = SendingReportUiState.Success("email@mail.com")),
                ReportProblemUiState(
                    sendingState =
                        SendingReportUiState.Error(SendProblemReportResult.Error.SendReport)
                ),
            )
}
