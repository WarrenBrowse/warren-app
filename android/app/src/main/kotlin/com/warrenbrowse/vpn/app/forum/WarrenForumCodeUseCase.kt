package com.warrenbrowse.vpn.app.forum

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.repository.ForumSignInRequests
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull

/**
 * Places a session id typed under "Sign in to the forum with a code" and
 * raises the consent it calls for. The forum's attach page prints its
 * session id in the same shape as the sign-in page's code, and a reader
 * whose "Open the app" button did nothing types whichever they see; before
 * the probe existed such a code was preflighted as a login and answered
 * "expired" (topic 199, 2026-09-07). The probe reads the unsigned status
 * endpoints in Rust ([WarrenJniBridge.forumCodeProbe]), the attach meta
 * included, which names the topic the code cannot carry: an attach session
 * opens the attach consent with its topic, anything else the login consent,
 * which preflights again before signing, so an unplaced code costs nothing
 * it did not cost before. The reads are bounded: the screen shows progress
 * meanwhile, and a broker that does not answer falls back to the login flow.
 */
class WarrenForumCodeUseCase(
    private val jni: WarrenJniBridge,
    private val loginController: ForumLoginController,
    private val attachController: ForumAttachController,
    private val journal: ForumJournal,
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
    private val probeTimeoutMillis: Long = PROBE_TIMEOUT_MILLIS,
) : ForumSignInRequests {

    override suspend fun requestSignIn(sid: String) {
        val placement =
            withTimeoutOrNull(probeTimeoutMillis) { withContext(ioDispatcher) { probe(sid) } }
                ?: Placement.Login("timeout")
        when (placement) {
            is Placement.Attach -> {
                journal.record(
                    ForumEvent.LINK_RECEIVED,
                    JournalField.Verdict("accepted"),
                    JournalField.Source(LinkSource.TYPED_CODE),
                    JournalField.Kind(ForumLinkKind.ATTACH),
                    JournalField.Class(placement.probeClass),
                    JournalField.PreTopic(placement.topicId == ForumAttachLink.PRE_TOPIC),
                )
                attachController.request(forumAttachLinkFromCode(sid, placement.topicId))
            }
            is Placement.Login -> {
                journal.record(
                    ForumEvent.LINK_RECEIVED,
                    JournalField.Verdict("accepted"),
                    JournalField.Source(LinkSource.TYPED_CODE),
                    JournalField.Kind(ForumLinkKind.LOGIN),
                    JournalField.Class(placement.probeClass),
                )
                loginController.request(forumLoginLinkFromCode(sid))
            }
        }
    }

    /** Which consent the probe calls for, with the class the journal records. */
    private sealed interface Placement {
        val probeClass: String

        data class Attach(val topicId: Long) : Placement {
            override val probeClass = "attach"
        }

        /** The login consent, for a login session and for every code the probe could not place. */
        data class Login(override val probeClass: String) : Placement
    }

    // The bridge fails through an open set of runtime exceptions (bridge and
    // native-panic classes); a probe that throws is journaled as such and the
    // code falls back to the flow it always had.
    @Suppress("TooGenericExceptionCaught")
    private fun probe(sid: String): Placement =
        try {
            parsePlacement(jni.forumCodeProbe(sid, ALLOWED_CONNECT_HOST))
        } catch (e: Exception) {
            Logger.w(throwable = e) { "WarrenJniBridge.forumCodeProbe threw" }
            Placement.Login("jni")
        }

    /**
     * The probe envelope: `kind` and, for an attach session, `topic_id` (0 for
     * a pre-topic session). An attach kind without a usable topic is not
     * offered: the upload could only ever be refused as a dead session.
     */
    private fun parsePlacement(rawJson: String): Placement =
        try {
            val root = Json.parseToJsonElement(rawJson).jsonObject
            val kind = root["kind"]?.jsonPrimitive?.content
            if (kind == "attach") {
                val topicId = root["topic_id"]?.jsonPrimitive?.longOrNull
                if (topicId != null && topicId in ForumAttachLink.PRE_TOPIC..MAX_FORUM_TOPIC_ID) {
                    Placement.Attach(topicId)
                } else {
                    Placement.Login("attach-no-topic")
                }
            } else {
                Placement.Login(kind?.takeIf { it in CODE_KINDS } ?: "unknown")
            }
        } catch (e: SerializationException) {
            Placement.Login("unknown")
        } catch (e: IllegalArgumentException) {
            Placement.Login("unknown")
        }

    companion object {
        /**
         * The bound on the placement: up to three broker reads, each on the
         * forum transport's own 15 s. Past it the login consent is raised,
         * which preflights again before signing.
         */
        const val PROBE_TIMEOUT_MILLIS: Long = 20_000L
        private val CODE_KINDS = setOf("login", "gone", "unknown")
    }
}
