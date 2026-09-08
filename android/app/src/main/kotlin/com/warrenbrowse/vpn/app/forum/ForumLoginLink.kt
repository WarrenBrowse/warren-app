package com.warrenbrowse.vpn.app.forum

import java.net.URI
import java.net.URISyntaxException

/**
 * A validated `warren://forum-login` deep link.
 *
 * [crossDevice] means the link came from the QR on the approval page, so the
 * browser signing in is on another device. That is also exactly what a relayed
 * (phished) approval looks like, and nothing on the wire tells the two apart,
 * so the consent prompt says which one it is and lets the person decide.
 */
data class ForumLoginLink(val sid: String, val host: String, val crossDevice: Boolean = false)

// The single connect host accepted from a forum deep link. A hard allowlist:
// a hostile link must not be able to point the wallet-signed request at an
// attacker-controlled server (Rust re-checks this too).
internal const val ALLOWED_CONNECT_HOST = "connect.warrenbrowse.com"

// The Discourse SSO session id shape: exactly 32 lowercase hex chars.
internal val FORUM_SID_REGEX = Regex("^[0-9a-f]{32}$")

/** The deep-link actions the manifest registers, one per flow. */
internal const val FORUM_LOGIN_ACTION = "forum-login"
internal const val ATTACH_LOGS_ACTION = "attach-logs"

/**
 * The action of a forum deep link (`forum-login`, `attach-logs`), or null
 * when the data is not a URI at all. Read before either parser: one intent
 * filter serves both flows, and a link handed to the wrong parser would be
 * refused as `wrong-action` and dropped.
 */
fun forumDeepLinkAction(rawUrl: String?): String? = rawUrl?.let(::parseForumUri)?.let(::forumUriAction)

// `warren://forum-login?..` parses with authority = "forum-login".
internal fun forumUriAction(uri: URI): String? = uri.authority ?: uri.path?.trimStart('/')

/**
 * Parse and validate a `warren://forum-login?sid=..&host=..` URL. Returns null
 * for anything that is not a well-formed, allowlisted forum-login link (wrong
 * scheme or action, malformed sid, non-allowlisted host). Mirrors the desktop
 * `parseForumLoginUrl`; the Rust layer re-validates before signing, so this is a
 * fail-fast guard, not the security boundary.
 */
fun parseForumLoginLink(
    rawUrl: String?,
    // Per-flavor scheme (warren / warren-beta) so the beta app only answers
    // its own registered deep links. Parameterized for the JVM tests.
    expectedScheme: String = com.warrenbrowse.vpn.BuildConfig.DEEP_LINK_SCHEME,
): ForumLoginLink? = (classifyForumLoginLink(rawUrl, expectedScheme) as? ForumLinkVerdict.Accepted)?.link

/**
 * A deep link's verdict. A rejection names its class only (never the values):
 * a scheme or host drift between the broker and the app is exactly what a
 * report has to be able to show, and it was invisible while a rejected link
 * was dropped in silence.
 */
sealed interface ForumLinkVerdict {
    data class Accepted(val link: ForumLoginLink) : ForumLinkVerdict

    data class Rejected(val reason: String) : ForumLinkVerdict
}

/** [parseForumLoginLink] with the rejection class kept. */
fun classifyForumLoginLink(
    rawUrl: String?,
    expectedScheme: String = com.warrenbrowse.vpn.BuildConfig.DEEP_LINK_SCHEME,
): ForumLinkVerdict {
    val uri = rawUrl?.let(::parseForumUri)
    return when {
        rawUrl == null -> ForumLinkVerdict.Rejected("no-data")
        uri == null -> ForumLinkVerdict.Rejected("not-a-uri")
        // The received scheme is a product-environment name, not identity
        // material: it is the one fact that tells a prod/beta mismatch apart.
        uri.scheme != expectedScheme -> ForumLinkVerdict.Rejected("wrong-scheme:${uri.scheme ?: "none"}")
        forumUriAction(uri) != FORUM_LOGIN_ACTION -> ForumLinkVerdict.Rejected("wrong-action")
        else -> classifyQuery(parseForumQuery(uri.rawQuery))
    }
}

internal fun parseForumUri(rawUrl: String): URI? =
    try {
        URI(rawUrl)
    } catch (e: URISyntaxException) {
        null
    }

private fun classifyQuery(params: Map<String, String>): ForumLinkVerdict {
    val sid = params["sid"]
    val host = params["host"]
    return when {
        sid == null -> ForumLinkVerdict.Rejected("missing-sid")
        host == null -> ForumLinkVerdict.Rejected("missing-host")
        !FORUM_SID_REGEX.matches(sid) -> ForumLinkVerdict.Rejected("bad-sid-shape")
        host != ALLOWED_CONNECT_HOST -> ForumLinkVerdict.Rejected("host-not-allowlisted")
        // The provider sets `xd=1` on the QR link only. Anything else, an older
        // provider included, is the same-device button and gets the ordinary
        // prompt rather than a warning nobody can act on.
        else -> ForumLinkVerdict.Accepted(ForumLoginLink(sid, host, crossDevice = params["xd"] == "1"))
    }
}

/**
 * The link a sign-in code typed by hand stands for: the same request a deep
 * link would carry, against the one allowlisted host. Browser-independent.
 *
 * Marked cross-device, which is the honest reading: there is no link and no
 * `xd` signal, so the app cannot tell a code the user read off this screen
 * from one an attacker sent them ("paste this in Settings to finish your
 * sign-in"). Only the cross-device prompt says that approving hands the forum
 * identity to whoever sent the code, and that is exactly this case.
 */
fun forumLoginLinkFromCode(sid: String): ForumLoginLink =
    ForumLoginLink(sid, ALLOWED_CONNECT_HOST, crossDevice = true)

internal fun parseForumQuery(rawQuery: String?): Map<String, String> {
    if (rawQuery.isNullOrEmpty()) return emptyMap()
    return rawQuery
        .split('&')
        .mapNotNull { pair ->
            val i = pair.indexOf('=')
            if (i <= 0) null else pair.substring(0, i) to pair.substring(i + 1)
        }
        .toMap()
}
