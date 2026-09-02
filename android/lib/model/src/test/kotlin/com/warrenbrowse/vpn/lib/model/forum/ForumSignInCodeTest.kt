package com.warrenbrowse.vpn.lib.model.forum

import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class ForumSignInCodeTest {
    private val sid = "0123456789abcdef0123456789abcdef"

    @Test
    fun `a canonical sid passes through`() {
        assertEquals(sid, normalizeForumSignInCode(sid))
    }

    @Test
    fun `grouping spaces dashes and capitals are normalized away`() {
        assertEquals(sid, normalizeForumSignInCode(" 0123 4567-89AB cdef\n0123456789abcdef "))
    }

    @Test
    fun `anything that is not 32 hex characters is refused`() {
        assertNull(normalizeForumSignInCode("0123456789abcdef"))
        assertNull(normalizeForumSignInCode("0123456789abcdef0123456789abcdeg"))
        assertNull(normalizeForumSignInCode(""))
    }
}
