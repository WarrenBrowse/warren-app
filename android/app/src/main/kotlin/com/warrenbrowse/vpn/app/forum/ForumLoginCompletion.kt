package com.warrenbrowse.vpn.app.forum

/**
 * What a bound approval hands back (warren-connect `docs/FORUM-LOGIN-V2.md`):
 * the one-time code the browser that opened the sign-in must present, and on a
 * same-device approval the handoff URL that carries it to the default browser.
 * Both are live credentials until the session ends, so [toString] prints
 * neither and nothing here is ever journaled, logged or persisted.
 */
data class ForumLoginCompletion(val code: String, val handoffUrl: String?) {
    override fun toString(): String =
        "ForumLoginCompletion(code=<redacted>, handoffUrl=${if (handoffUrl == null) "none" else "<redacted>"})"
}

/** How long after the provider's answer the code may still be shown: the session's own lifetime. */
const val FORUM_LOGIN_CODE_LIFETIME_MILLIS: Long = 300_000L

enum class ForumCompletionScreen(val token: String) {
    /** No completion: the browser completes on its own, as before the code existed. */
    RETURNED_TO_BROWSER("returned-to-browser"),
    /** The handoff is open in the default browser; the code waits behind "Show the code". */
    FINISHING_IN_BROWSER("finishing-in-browser"),
    /** The code, with the warning never to share it. */
    SHOW_CODE("show-code"),
    /**
     * The code under the warning that the link was for a sign-in on another
     * device: a same-device link answered without a handoff got the answer to
     * a QR's id, so it lost its `xd=1` on the way, which is how a relayed
     * approval reaches someone's own phone.
     */
    SHOW_CODE_RELAYED("show-code-relayed"),
}

enum class ForumHandoff(val token: String) {
    OPEN_AT_ONCE("open-at-once"),
    ON_BUTTON("on-button"),
    NEVER("never"),
}

data class ForumCompletionPlan(val screen: ForumCompletionScreen, val handoff: ForumHandoff)

/**
 * The screen and the handoff of an approved login, from how its sid arrived
 * and the completion the answer carried. Pinned by the `login.completion`
 * table of `fixtures/client-rules/forum_outcomes.json`.
 */
fun forumCompletionPlan(
    approach: ForumLoginApproach,
    completion: ForumLoginCompletion?,
): ForumCompletionPlan {
    if (completion == null) {
        return ForumCompletionPlan(ForumCompletionScreen.RETURNED_TO_BROWSER, ForumHandoff.NEVER)
    }
    val hasHandoff = completion.handoffUrl != null
    return when (approach) {
        ForumLoginApproach.SAME_DEVICE_LINK ->
            if (hasHandoff) {
                ForumCompletionPlan(ForumCompletionScreen.FINISHING_IN_BROWSER, ForumHandoff.OPEN_AT_ONCE)
            } else {
                ForumCompletionPlan(ForumCompletionScreen.SHOW_CODE_RELAYED, ForumHandoff.NEVER)
            }
        ForumLoginApproach.TYPED_CODE ->
            ForumCompletionPlan(
                ForumCompletionScreen.SHOW_CODE,
                if (hasHandoff) ForumHandoff.ON_BUTTON else ForumHandoff.NEVER,
            )
        // The browser signing in is on another device: a handoff a provider
        // sent anyway would carry the code to this device's browser.
        ForumLoginApproach.CROSS_DEVICE_LINK ->
            ForumCompletionPlan(ForumCompletionScreen.SHOW_CODE, ForumHandoff.NEVER)
    }
}

// The crate validated both before they crossed the FFI; the handoff is opened
// in the default browser, so the decoder takes nothing else either.
private val COMPLETION_CODE = Regex("^[0-9]{6}$")

/**
 * The completion of the envelope's `completion` object for the login of [sid],
 * or null when it is absent or malformed. A handoff that is not exactly this
 * session's is dropped and the code kept: another session's would finish
 * someone else's sign-in in this device's browser.
 */
internal fun forumLoginCompletionOf(
    code: String?,
    handoffUrl: String?,
    sid: String,
): ForumLoginCompletion? {
    if (code == null || !COMPLETION_CODE.matches(code)) return null
    val handoff =
        handoffUrl?.takeIf { url ->
            FORUM_SID_REGEX.matches(sid) &&
                url == "https://$ALLOWED_CONNECT_HOST/handoff#sid=$sid&code=$code"
        }
    return ForumLoginCompletion(code, handoff)
}
