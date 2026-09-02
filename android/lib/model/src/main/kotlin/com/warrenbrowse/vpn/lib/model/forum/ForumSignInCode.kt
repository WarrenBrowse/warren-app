package com.warrenbrowse.vpn.lib.model.forum

/** The forum SSO session id shape: exactly 32 lowercase hex characters. */
private val SID_REGEX = Regex("^[0-9a-f]{32}$")

/**
 * A sign-in code as a person types it: the 32 hex characters of the session id
 * shown on the approval page, in any case, with any spaces or dashes a display
 * may have grouped them with. Returns the canonical sid, or null for anything
 * else. Mirrors `warren_forum::normalize_sign_in_code`, the rule the Rust side
 * enforces again before signing, so this is a fail-fast guard for the field.
 */
fun normalizeForumSignInCode(typed: String): String? {
    val cleaned =
        typed.filterNot { it.isWhitespace() || it == '-' }.lowercase()
    return if (SID_REGEX.matches(cleaned)) cleaned else null
}
