package com.warrenbrowse.vpn.app.forum

import co.touchlab.kermit.Logger
import com.warrenbrowse.vpn.lib.repository.ForumSignInRequests
import com.warrenbrowse.vpn.lib.repository.WarrenJniBridge
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * Places a session id typed under "Sign in to the forum with a code" and
 * raises the consent it calls for. The forum's attach page prints its
 * session id in the same shape as the sign-in page's code, and a reader
 * whose "Open the app" button did nothing types whichever they see; before
 * the probe existed such a code was preflighted as a login and answered
 * "expired" (topic 199, 2026-09-07). The probe reads the two unsigned status
 * endpoints in Rust ([WarrenJniBridge.forumCodeProbe]); an attach session
 * opens the attach consent, anything else the login consent, which
 * preflights again before signing, so an unplaced code costs nothing it did
 * not cost before.
 */
class WarrenForumCodeUseCase(
    private val jni: WarrenJniBridge,
    private val loginController: ForumLoginController,
    private val attachController: ForumAttachController,
    private val journal: ForumJournal,
    private val scope: CoroutineScope,
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
) : ForumSignInRequests {

    override fun requestSignIn(sid: String) {
        scope.launch {
            val probe = withContext(ioDispatcher) { probe(sid) }
            val kind = if (probe == CODE_KIND_ATTACH) ForumLinkKind.ATTACH else ForumLinkKind.LOGIN
            journal.record(
                ForumEvent.LINK_RECEIVED,
                JournalField.Verdict("accepted"),
                JournalField.Source(LinkSource.TYPED_CODE),
                JournalField.Kind(kind),
                JournalField.Class(probe),
            )
            when (kind) {
                ForumLinkKind.ATTACH -> attachController.request(forumAttachLinkFromCode(sid))
                ForumLinkKind.LOGIN -> loginController.request(forumLoginLinkFromCode(sid))
            }
        }
    }

    // The bridge fails through an open set of runtime exceptions (bridge and
    // native-panic classes); a probe that throws is journaled as such and the
    // code falls back to the flow it always had.
    @Suppress("TooGenericExceptionCaught")
    private fun probe(sid: String): String =
        try {
            parseCodeProbe(jni.forumCodeProbe(sid, ALLOWED_CONNECT_HOST))
        } catch (e: Exception) {
            Logger.w(throwable = e) { "WarrenJniBridge.forumCodeProbe threw" }
            "jni"
        }

    private companion object {
        const val CODE_KIND_ATTACH = "attach"
        val CODE_KINDS = setOf("login", CODE_KIND_ATTACH, "gone", "unknown")
    }

    /** The `kind` of the probe envelope, or `unknown` for anything off the table. */
    private fun parseCodeProbe(rawJson: String): String =
        try {
            Json.parseToJsonElement(rawJson).jsonObject["kind"]?.jsonPrimitive?.content?.takeIf { it in CODE_KINDS }
                ?: "unknown"
        } catch (e: SerializationException) {
            "unknown"
        } catch (e: IllegalArgumentException) {
            "unknown"
        }
}
