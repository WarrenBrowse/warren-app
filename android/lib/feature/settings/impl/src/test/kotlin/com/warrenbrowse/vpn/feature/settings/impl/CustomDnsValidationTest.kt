package com.warrenbrowse.vpn.feature.settings.impl

import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * Custom DNS entries are the resolvers the tunnel is built with, so a
 * half-typed address must never reach the repository. Each comma-separated
 * token has to parse as a numeric IP, and the list must not repeat one.
 *
 * Written as pure Kotlin rather than through `android.net.InetAddresses` so
 * the rule is exercised on the JVM, where the framework stub throws.
 */
class CustomDnsValidationTest {

    @Test
    fun `dotted quads are accepted`() {
        assertTrue(isNumericIpAddress("9.9.9.9"))
        assertTrue(isNumericIpAddress("0.0.0.0"))
        assertTrue(isNumericIpAddress("255.255.255.255"))
        assertTrue(isNumericIpAddress("149.112.112.112"))
    }

    @Test
    fun `malformed or out-of-range dotted quads are rejected`() {
        assertFalse(isNumericIpAddress("9.9.9"))
        assertFalse(isNumericIpAddress("9.9.9.9.9"))
        assertFalse(isNumericIpAddress("256.1.1.1"))
        assertFalse(isNumericIpAddress("9.9.9."))
        assertFalse(isNumericIpAddress("01.1.1.1"))
        assertFalse(isNumericIpAddress("nine.nine.nine.nine"))
        assertFalse(isNumericIpAddress(""))
    }

    @Test
    fun `ipv6 literals are accepted including compressed forms`() {
        assertTrue(isNumericIpAddress("2620:fe::fe"))
        assertTrue(isNumericIpAddress("::1"))
        assertTrue(isNumericIpAddress("::"))
        assertTrue(isNumericIpAddress("2001:0db8:0000:0000:0000:0000:0000:0001"))
    }

    @Test
    fun `malformed ipv6 literals are rejected`() {
        assertFalse(isNumericIpAddress("2001::db8::1"))
        assertFalse(isNumericIpAddress("2001:0db8:0000:0000:0000:0000:0000"))
        assertFalse(isNumericIpAddress("gggg::1"))
        assertFalse(isNumericIpAddress(":"))
    }

    @Test
    fun `a list of valid distinct addresses is accepted`() {
        assertTrue(customDnsInputIsValid("9.9.9.9, 149.112.112.112"))
        assertTrue(customDnsInputIsValid("9.9.9.9\n2620:fe::fe"))
    }

    @Test
    fun `an empty list is valid so the field can be cleared`() {
        assertTrue(customDnsInputIsValid(""))
        assertTrue(customDnsInputIsValid("  ,  "))
        assertEquals(emptyList(), customDnsServersFromInput("  ,  "))
    }

    @Test
    fun `a token that is not an address invalidates the whole list`() {
        assertFalse(customDnsInputIsValid("9.9.9.9, nope"))
        assertFalse(customDnsInputIsValid("9.9"))
    }

    @Test
    fun `a duplicated address invalidates the list`() {
        assertFalse(customDnsInputIsValid("9.9.9.9, 9.9.9.9"))
    }

    @Test
    fun `parsing trims and drops blanks`() {
        assertEquals(
            listOf("9.9.9.9", "149.112.112.112"),
            customDnsServersFromInput(" 9.9.9.9 , ,149.112.112.112\n"),
        )
    }
}
