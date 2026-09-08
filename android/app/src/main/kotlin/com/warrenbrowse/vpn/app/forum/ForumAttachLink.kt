package com.warrenbrowse.vpn.app.forum

/**
 * A validated `warren://attach-logs` deep link: the forum's "attach your
 * logs" page asking the app to attach its redacted problem report to the
 * topic [topicId], or, when it is 0, to a report still being composed (the
 * forum binds the logs to the topic after creation).
 *
 * [topicId] is null for a session id typed by hand: the broker's status and
 * meta endpoints carry no topic id, so the consent prompt asks for it.
 */
data class ForumAttachLink(val sid: String, val host: String, val topicId: Long?) {
    val isPreTopic: Boolean
        get() = topicId == PRE_TOPIC

    companion object {
        /** The topic id of a report still being composed. */
        const val PRE_TOPIC: Long = 0L
    }
}

/**
 * The largest topic id every platform accepts: a JavaScript safe integer,
 * because the desktop carries the value through a `number` and the rule is
 * the same on every client (`fixtures/client-rules/forum_link.json`).
 */
const val MAX_FORUM_TOPIC_ID: Long = 9_007_199_254_740_991L

private val TOPIC_REGEX = Regex("^[0-9]+$")

/** An attach link's verdict; a rejection names its class only, like the login's. */
sealed interface ForumAttachVerdict {
    data class Accepted(val link: ForumAttachLink) : ForumAttachVerdict

    data class Rejected(val reason: String) : ForumAttachVerdict
}

/**
 * Parse and validate a `warren://attach-logs?sid=..&topic=..&host=..` URL.
 * Mirrors the desktop `parseForumAttachUrl`, with the login parser's
 * rejection classes plus `missing-topic` and `bad-topic`; the Rust layer
 * re-validates the sid and the host before signing, so this is a fail-fast
 * guard, not the security boundary.
 */
fun classifyForumAttachLink(
    rawUrl: String?,
    expectedScheme: String = com.warrenbrowse.vpn.BuildConfig.DEEP_LINK_SCHEME,
): ForumAttachVerdict {
    val uri = rawUrl?.let(::parseForumUri)
    return when {
        rawUrl == null -> ForumAttachVerdict.Rejected("no-data")
        uri == null -> ForumAttachVerdict.Rejected("not-a-uri")
        uri.scheme != expectedScheme -> ForumAttachVerdict.Rejected("wrong-scheme:${uri.scheme ?: "none"}")
        forumUriAction(uri) != ATTACH_LOGS_ACTION -> ForumAttachVerdict.Rejected("wrong-action")
        else -> classifyAttachQuery(parseForumQuery(uri.rawQuery))
    }
}

private fun classifyAttachQuery(params: Map<String, String>): ForumAttachVerdict {
    val sid = params["sid"]
    val host = params["host"]
    val topic = params["topic"]
    val topicId = topic?.let(::parseForumTopicId)
    return when {
        sid == null -> ForumAttachVerdict.Rejected("missing-sid")
        host == null -> ForumAttachVerdict.Rejected("missing-host")
        topic == null -> ForumAttachVerdict.Rejected("missing-topic")
        !FORUM_SID_REGEX.matches(sid) -> ForumAttachVerdict.Rejected("bad-sid-shape")
        topicId == null -> ForumAttachVerdict.Rejected("bad-topic")
        host != ALLOWED_CONNECT_HOST -> ForumAttachVerdict.Rejected("host-not-allowlisted")
        else -> ForumAttachVerdict.Accepted(ForumAttachLink(sid, host, topicId))
    }
}

/**
 * A topic id as the link, or the consent prompt's field, spells it: decimal
 * digits only (so no sign), within [MAX_FORUM_TOPIC_ID]; null otherwise.
 */
fun parseForumTopicId(text: String): Long? {
    if (!TOPIC_REGEX.matches(text)) return null
    val value = text.toLongOrNull() ?: return null
    return value.takeIf { it <= MAX_FORUM_TOPIC_ID }
}

/**
 * The attach request a session id typed by hand stands for, against the one
 * allowlisted host. The topic is unknown: the attach page prints its session
 * id in the same shape as the sign-in code, and nothing the broker answers
 * without a signature names the topic, so the prompt asks for it.
 */
fun forumAttachLinkFromCode(sid: String): ForumAttachLink =
    ForumAttachLink(sid, ALLOWED_CONNECT_HOST, topicId = null)
