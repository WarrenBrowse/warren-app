package com.warrenbrowse.vpn.app.forum

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * A session id typed under "Sign in to the forum with a code" is placed by
 * two unsigned reads before any consent is raised: the forum's attach page
 * prints its session id in the same shape as the sign-in code, and the
 * reporter of 2026-09-07 typed one into this screen four times and was told
 * four times that a sign-in had expired.
 */
class WarrenForumCodeUseCaseTest {

    private val sid = "0123456789abcdef0123456789abcdef"

    private class Harness(
        val jni: FakeJniBridge,
        ioDispatcher: kotlinx.coroutines.CoroutineDispatcher = Dispatchers.Unconfined,
        probeTimeoutMillis: Long = WarrenForumCodeUseCase.PROBE_TIMEOUT_MILLIS,
    ) {
        // The JVM has no main looper: the scope a prompt would launch on is injected.
        private val scope = CoroutineScope(Dispatchers.Unconfined)
        val loginController = ForumLoginController(scope)
        val attachController = ForumAttachController(scope)
        val journal = RecordingJournal()
        val useCase =
            WarrenForumCodeUseCase(
                jni = jni,
                loginController = loginController,
                attachController = attachController,
                journal = journal,
                // Unconfined: the probe runs to completion inside the test.
                ioDispatcher = ioDispatcher,
                probeTimeoutMillis = probeTimeoutMillis,
            )

        fun linkReceived(): List<JournalField> = journal.fieldsOf(ForumEvent.LINK_RECEIVED).single()
    }

    @Test
    fun a_code_the_broker_holds_as_an_attach_session_opens_the_attach_consent_with_its_topic() = runTest {
        val h = Harness(FakeJniBridge(codeProbeAnswer = { """{"kind":"attach","topic_id":199}""" }))

        h.useCase.requestSignIn(sid)

        assertEquals(forumAttachLinkFromCode(sid, topicId = 199L), h.attachController.pending.value)
        assertNull(h.loginController.pending.value)
        val fields = h.linkReceived()
        assertTrue(fields.contains(JournalField.Source(LinkSource.TYPED_CODE)))
        assertTrue(fields.contains(JournalField.Kind(ForumLinkKind.ATTACH)))
        assertTrue(fields.contains(JournalField.Verdict("accepted")))
        assertTrue(fields.contains(JournalField.PreTopic(false)))
    }

    @Test
    fun a_pre_topic_attach_session_opens_the_attach_consent_for_the_report_being_composed() = runTest {
        val h = Harness(FakeJniBridge(codeProbeAnswer = { """{"kind":"attach","topic_id":0}""" }))

        h.useCase.requestSignIn(sid)

        assertEquals(
            forumAttachLinkFromCode(sid, topicId = ForumAttachLink.PRE_TOPIC),
            h.attachController.pending.value,
        )
        assertTrue(h.linkReceived().contains(JournalField.PreTopic(true)))
    }

    @Test
    fun an_attach_placement_without_a_topic_falls_back_to_the_login_consent() = runTest {
        // A provider older than the meta's topic field: no topic means no
        // attach consent, which could only ever be refused as a dead session.
        val h = Harness(FakeJniBridge(codeProbeAnswer = { """{"kind":"attach"}""" }))

        h.useCase.requestSignIn(sid)

        assertNull(h.attachController.pending.value)
        assertEquals(forumLoginLinkFromCode(sid), h.loginController.pending.value)
        assertTrue(h.linkReceived().contains(JournalField.Class("attach-no-topic")))
    }

    @Test
    fun a_code_the_broker_holds_as_a_login_session_opens_the_login_consent() = runTest {
        val h = Harness(FakeJniBridge(codeProbeAnswer = { """{"kind":"login"}""" }))

        h.useCase.requestSignIn(sid)

        assertEquals(forumLoginLinkFromCode(sid), h.loginController.pending.value)
        assertNull(h.attachController.pending.value)
        assertTrue(h.linkReceived().contains(JournalField.Kind(ForumLinkKind.LOGIN)))
    }

    @Test
    fun a_code_the_probe_cannot_place_falls_back_to_the_login_consent_which_preflights_again() = runTest {
        // The login flow re-reads the session status before signing, so an
        // unplaced code costs nothing it did not cost before the probe existed.
        val h = Harness(FakeJniBridge(codeProbeAnswer = { """{"kind":"unknown"}""" }))

        h.useCase.requestSignIn(sid)

        assertEquals(forumLoginLinkFromCode(sid), h.loginController.pending.value)
        assertNull(h.attachController.pending.value)
        assertTrue(h.linkReceived().contains(JournalField.Class("unknown")))
    }

    @Test
    fun a_probe_that_throws_is_journaled_as_such_and_still_raises_the_login_consent() = runTest {
        val h = Harness(FakeJniBridge(codeProbeAnswer = { error("bridge not ready") }))

        h.useCase.requestSignIn(sid)

        assertEquals(forumLoginLinkFromCode(sid), h.loginController.pending.value)
        assertTrue(h.linkReceived().contains(JournalField.Class("jni")))
    }

    @Test
    fun a_spent_code_reaches_the_login_consent_which_answers_expired_on_approve() = runTest {
        val h = Harness(FakeJniBridge(codeProbeAnswer = { """{"kind":"gone"}""" }))

        h.useCase.requestSignIn(sid)

        assertEquals(forumLoginLinkFromCode(sid), h.loginController.pending.value)
        assertTrue(h.linkReceived().contains(JournalField.Class("gone")))
    }

    @Test
    fun the_probe_budget_is_handed_to_the_native_reads() = runTest {
        // `withTimeoutOrNull` cannot interrupt a blocking native call, so the
        // reads themselves are bounded in Rust with the budget the use case
        // holds, and the Kotlin bound is only the belt over it.
        val h = Harness(FakeJniBridge(codeProbeAnswer = { """{"kind":"login"}""" }), probeTimeoutMillis = 7_000L)

        h.useCase.requestSignIn(sid)

        assertEquals(listOf(7_000L), h.jni.codeProbeBudgets)
    }

    @Test
    fun a_probe_past_its_budget_falls_back_to_the_login_consent() = runBlocking {
        // A native read that outlives the budget (a broker that accepts the
        // connection and never answers) must not hold the screen: the login
        // consent is raised, which preflights again before signing.
        val jni =
            FakeJniBridge(
                codeProbeAnswer = {
                    Thread.sleep(2_000)
                    """{"kind":"attach","topic_id":199}"""
                }
            )
        val h = Harness(jni, ioDispatcher = Dispatchers.IO, probeTimeoutMillis = 100L)

        h.useCase.requestSignIn(sid)

        assertEquals(forumLoginLinkFromCode(sid), h.loginController.pending.value)
        assertNull(h.attachController.pending.value)
        assertTrue(h.linkReceived().contains(JournalField.Class("timeout")))
    }
}
