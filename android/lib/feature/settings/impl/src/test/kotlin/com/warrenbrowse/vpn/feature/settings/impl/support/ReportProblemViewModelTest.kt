package com.warrenbrowse.vpn.feature.settings.impl.support

import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.repository.CollectedReport
import com.warrenbrowse.vpn.lib.repository.ForumPreflight
import com.warrenbrowse.vpn.lib.repository.ReportArea
import com.warrenbrowse.vpn.lib.repository.ReportForm
import com.warrenbrowse.vpn.lib.repository.ReportFrequency
import com.warrenbrowse.vpn.lib.repository.ReportSubmitOutcome
import com.warrenbrowse.vpn.lib.repository.WarrenSupportReporter
import java.io.File
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertTrue
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class ReportProblemViewModelTest {

    private class FakeReporter(private val verdict: ForumPreflight) : WarrenSupportReporter {
        var collects = 0
        var submits = 0

        override fun preflight(): ForumPreflight = verdict

        override suspend fun collect(): Result<CollectedReport> {
            collects++
            return Result.success(CollectedReport(File("unused"), 1))
        }

        override suspend fun submit(form: ReportForm, report: CollectedReport?): ReportSubmitOutcome {
            submits++
            return ReportSubmitOutcome.Created(topicId = 1, topicUrl = "", identity = null, logs = "none")
        }

        override fun discard(report: CollectedReport) = Unit
    }

    private fun filledIn(viewModel: ReportProblemViewModel) {
        viewModel.setArea(ReportArea.CONNECTION)
        viewModel.setFrequency(ReportFrequency.ALWAYS)
        viewModel.setWhatHappened("The tunnel drops every few minutes on wifi.")
    }

    @Test
    fun a_send_while_the_tunnel_is_between_states_is_deferred_before_anything_is_collected() =
        runTest {
            val reporter = FakeReporter(ForumPreflight.Defer("blocking"))
            val viewModel = ReportProblemViewModel(reporter)
            filledIn(viewModel)

            viewModel.send()

            assertEquals(ReportSubmitOutcome.Deferred("blocking"), viewModel.state.value.outcome)
            assertEquals(0, reporter.collects)
            assertEquals(0, reporter.submits)
            assertFalse(viewModel.state.value.sending)
            // The form is intact and Send stays armed for the retry.
            assertTrue(viewModel.state.value.canSend)
        }

    @Test
    fun a_send_with_the_tunnel_settled_collects_then_submits() = runTest {
        val reporter = FakeReporter(ForumPreflight.Proceed)
        val viewModel = ReportProblemViewModel(reporter)
        filledIn(viewModel)

        viewModel.send()

        assertIs<ReportSubmitOutcome.Created>(viewModel.state.value.outcome)
        assertEquals(1, reporter.collects)
        assertEquals(1, reporter.submits)
    }
}
