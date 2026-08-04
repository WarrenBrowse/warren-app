package com.warrenbrowse.vpn.feature.settings.impl

import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The exit rate-limits port allocations per source, so the client must stop
 * issuing requests while a window is open instead of walking itself into a
 * ban. The block is derived purely from the status snapshot plus how long ago
 * it arrived, so it clears on the clock alone: a stale snapshot never strands
 * the controls.
 */
class NatPmpPortBlockTest {

    @Test
    fun `a healthy mapping blocks nothing`() {
        val block = natPmpPortBlock("""{"state":"mapped","external_port":51820}""", elapsedSecs = 0)
        assertFalse(block.blocked)
        assertFalse(block.lastChance)
        assertEquals(0, block.remainingSecs)
    }

    @Test
    fun `a rate-limited status blocks for its retry window`() {
        val json = """{"state":"rate_limited","retry_after_secs":90}"""
        val block = natPmpPortBlock(json, elapsedSecs = 30)
        assertTrue(block.blocked)
        assertEquals(60, block.remainingSecs)
    }

    @Test
    fun `an exhausted budget blocks until a slot frees`() {
        val json = """{"state":"mapped","attempts_remaining":0,"window_reset_secs":45}"""
        val block = natPmpPortBlock(json, elapsedSecs = 5)
        assertTrue(block.blocked)
        assertEquals(40, block.remainingSecs)
    }

    @Test
    fun `one remaining slot warns without blocking`() {
        val json = """{"state":"mapped","attempts_remaining":1,"window_reset_secs":60}"""
        val block = natPmpPortBlock(json, elapsedSecs = 10)
        assertFalse(block.blocked)
        assertTrue(block.lastChance)
        assertEquals(50, block.remainingSecs)
    }

    @Test
    fun `a comfortable budget neither blocks nor warns`() {
        val json = """{"state":"mapped","attempts_remaining":4,"window_reset_secs":60}"""
        val block = natPmpPortBlock(json, elapsedSecs = 0)
        assertFalse(block.blocked)
        assertFalse(block.lastChance)
    }

    @Test
    fun `an elapsed window clears the block so a stale snapshot cannot strand the user`() {
        val json = """{"state":"rate_limited","retry_after_secs":30}"""
        val block = natPmpPortBlock(json, elapsedSecs = 31)
        assertFalse(block.blocked)
        assertEquals(0, block.remainingSecs)
    }

    @Test
    fun `a zero-length window carries no live information`() {
        val json = """{"state":"mapped","attempts_remaining":0,"window_reset_secs":0}"""
        val block = natPmpPortBlock(json, elapsedSecs = 0)
        assertFalse(block.blocked)
        assertFalse(block.lastChance)
    }

    @Test
    fun `a pre-trailer exit that reports no budget blocks nothing`() {
        val json = """{"state":"mapped","external_port":51820,"lifetime_secs":3600}"""
        assertFalse(natPmpPortBlock(json, elapsedSecs = 0).blocked)
        assertFalse(natPmpPortBlock(json, elapsedSecs = 0).lastChance)
    }

    @Test
    fun `the countdown renders as minutes and seconds`() {
        assertEquals("00:00", formatCountdown(0))
        assertEquals("00:09", formatCountdown(9))
        assertEquals("01:30", formatCountdown(90))
        assertEquals("10:05", formatCountdown(605))
    }
}
