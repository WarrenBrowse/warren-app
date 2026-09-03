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

// The single connect host accepted from a forum-login deep link. A hard
// allowlist: a hostile link must not be able to point the wallet-signed request
// at an attacker-controlled server (Rust re-checks this too).
private const val ALLOWED_CONNECT_HOST = "connect.warrenbrowse.com"

// The Discourse SSO session id shape: exactly 32 lowercase hex chars.
private val SID_REGEX = Regex("^[0-9a-f]{32}$")

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
    val uri = rawUrl?.let(::parseUri)
    // `warren://forum-login?..` parses with authority = "forum-login".
    val action = uri?.authority ?: uri?.path?.trimStart('/')
    return when {
        rawUrl == null -> ForumLinkVerdict.Rejected("no-data")
        uri == null -> ForumLinkVerdict.Rejected("not-a-uri")
        // The received scheme is a product-environment name, not identity
        // material: it is the one fact that tells a prod/beta mismatch apart.
        uri.scheme != expectedScheme -> ForumLinkVerdict.Rejected("wrong-scheme:${uri.scheme ?: "none"}")
        action != "forum-login" -> ForumLinkVerdict.Rejected("wrong-action")
        else -> classifyQuery(parseQuery(uri.rawQuery))
    }
}

private fun parseUri(rawUrl: String): URI? =
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
        !SID_REGEX.matches(sid) -> ForumLinkVerdict.Rejected("bad-sid-shape")
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
 */
fun forumLoginLinkFromCode(sid: String): ForumLoginLink =
    ForumLoginLink(sid, ALLOWED_CONNECT_HOST, crossDevice = false)

private fun parseQuery(rawQuery: String?): Map<String, String> {
    if (rawQuery.isNullOrEmpty()) return emptyMap()
    return rawQuery
        .split('&')
        .mapNotNull { pair ->
            val i = pair.indexOf('=')
            if (i <= 0) null else pair.substring(0, i) to pair.substring(i + 1)
        }
        .toMap()
}
