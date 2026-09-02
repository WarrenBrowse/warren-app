package com.warrenbrowse.vpn.feature.settings.impl.support

import com.warrenbrowse.vpn.lib.common.test.TestCoroutineRule
import com.warrenbrowse.vpn.lib.model.forum.ForumIdentity
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
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

@ExtendWith(TestCoroutineRule::class)
class ReportProblemViewModelTest {

    private class FakeReporter(
        private val verdict: ForumPreflight = ForumPreflight.Proceed,
        private val outcome: ReportSubmitOutcome = created,
    ) : WarrenSupportReporter {
        var collects = 0
        var submits = 0
        val submitted = mutableListOf<CollectedReport?>()
        val discarded = mutableListOf<CollectedReport>()

        /** When set, a submit waits on it: the send is in flight until the test releases it. */
        var gate: CompletableDeferred<Unit>? = null

        override fun preflight(): ForumPreflight = verdict

        override suspend fun collect(): Result<CollectedReport> {
            collects++
            return Result.success(CollectedReport(File("report-$collects"), collects.toLong()))
        }

        override suspend fun submit(form: ReportForm, report: CollectedReport?): ReportSubmitOutcome {
            submits++
            submitted += report
            gate?.await()
            return outcome
        }

        override fun discard(report: CollectedReport) {
            discarded += report
        }
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

    @Test
    fun every_outcome_of_a_send_lands_in_the_state_with_the_send_over_and_the_logs_dropped() =
        runTest {
            val outcomes =
                listOf(
                    created,
                    ReportSubmitOutcome.Created(7, "https://forum.example/t/7", null, "partial"),
                    ReportSubmitOutcome.SubscriptionRequired,
                    ReportSubmitOutcome.ClockSkew,
                    ReportSubmitOutcome.RateLimited,
                    ReportSubmitOutcome.TooLarge,
                    ReportSubmitOutcome.Invalid,
                    ReportSubmitOutcome.ServerError,
                    ReportSubmitOutcome.WalletNotReady,
                    ReportSubmitOutcome.Deferred("reconnecting"),
                    ReportSubmitOutcome.Failure("transport"),
                )

            for (outcome in outcomes) {
                val reporter = FakeReporter(outcome = outcome)
                val viewModel = ReportProblemViewModel(reporter)
                filledIn(viewModel)

                viewModel.send()

                val state = viewModel.state.value
                assertEquals(outcome, state.outcome, "outcome $outcome")
                assertFalse(state.sending, "sending after $outcome")
                // The collected report was sent and discarded; nothing lingers in the cache.
                assertNull(state.collected, "collected after $outcome")
                assertEquals(listOf(reporter.submitted.single()), reporter.discarded, "discard after $outcome")
                // Only a filed report disarms Send: every refusal is retried from the same form.
                assertEquals(outcome !is ReportSubmitOutcome.Created, state.canSend, "canSend after $outcome")
                assertEquals(ReportArea.CONNECTION, state.area)
                assertEquals("The tunnel drops every few minutes on wifi.", state.whatHappened)
            }
        }

    @Test
    fun a_send_in_flight_outlives_the_screen_that_started_it() = runTest {
        val reporter = FakeReporter().apply { gate = CompletableDeferred() }
        val viewModel = ReportProblemViewModel(reporter)
        filledIn(viewModel)
        viewModel.setSteps("Turn wifi off and on.")

        viewModel.send()
        assertTrue(viewModel.state.value.sending)
        assertNull(viewModel.state.value.outcome)

        // The screen is recreated (rotation): a new observer of the same view
        // model sees the form and the send exactly as the old one left them.
        val recreated = viewModel.state.value
        assertEquals(ReportArea.CONNECTION, recreated.area)
        assertEquals(ReportFrequency.ALWAYS, recreated.frequency)
        assertEquals("The tunnel drops every few minutes on wifi.", recreated.whatHappened)
        assertEquals("Turn wifi off and on.", recreated.steps)
        assertTrue(recreated.sending)
        assertFalse(recreated.canSend)

        reporter.gate!!.complete(Unit)

        assertEquals(created, viewModel.state.value.outcome)
        assertFalse(viewModel.state.value.sending)
        assertEquals(1, reporter.submits)
    }

    @Test
    fun send_without_logs_skips_the_collection_and_drops_the_report_collected_for_the_preview() =
        runTest {
            val reporter = FakeReporter()
            val viewModel = ReportProblemViewModel(reporter)
            filledIn(viewModel)
            viewModel.collect()
            val previewed = viewModel.state.value.collected
            assertEquals(1, reporter.collects)
            assertEquals(1, previewed?.bytes)

            viewModel.sendWithoutLogs()

            assertEquals(1, reporter.collects)
            assertEquals(1, reporter.submits)
            assertNull(reporter.submitted.single())
            assertEquals(listOf(previewed), reporter.discarded)
            val state = viewModel.state.value
            assertFalse(state.includeLogs)
            assertNull(state.collected)
            assertEquals(created, state.outcome)
        }

    @Test
    fun turning_the_logs_back_on_after_a_size_refusal_sends_them_again() = runTest {
        val reporter = FakeReporter(outcome = ReportSubmitOutcome.TooLarge)
        val viewModel = ReportProblemViewModel(reporter)
        filledIn(viewModel)

        viewModel.send()
        assertEquals(ReportSubmitOutcome.TooLarge, viewModel.state.value.outcome)
        viewModel.setIncludeLogs(true)
        assertNull(viewModel.state.value.outcome)
        viewModel.send()

        assertEquals(2, reporter.collects)
        assertEquals(listOf(1L, 2L), reporter.submitted.map { it?.bytes })
    }

    private companion object {
        val created =
            ReportSubmitOutcome.Created(
                topicId = 171,
                topicUrl = "https://forum.example/t/171",
                identity = ForumIdentity("farul-togis-hubuf", 3),
                logs = "attached",
            )
    }
}
