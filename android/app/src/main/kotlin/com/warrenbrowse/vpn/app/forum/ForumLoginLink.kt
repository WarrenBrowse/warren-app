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
): ForumLoginLink? {
    if (rawUrl == null) return null
    val uri = try {
        URI(rawUrl)
    } catch (e: URISyntaxException) {
        return null
    }
    if (uri.scheme != expectedScheme) return null
    // `warren://forum-login?..` parses with authority = "forum-login".
    val action = uri.authority ?: uri.path?.trimStart('/')
    if (action != "forum-login") return null
    val params = parseQuery(uri.rawQuery)
    val sid = params["sid"] ?: return null
    val host = params["host"] ?: return null
    if (!SID_REGEX.matches(sid)) return null
    if (host != ALLOWED_CONNECT_HOST) return null
    // The provider sets `xd=1` on the QR link only. Anything else, an older
    // provider included, is the same-device button and gets the ordinary
    // prompt rather than a warning nobody can act on.
    return ForumLoginLink(sid, host, crossDevice = params["xd"] == "1")
}

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
