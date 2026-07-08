package com.warrenbrowse.vpn.app.service

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class AutoRecoveryTrackerTest {

    @Test
    fun `connected after an automation retry counts one recovery`() {
        val tracker = AutoRecoveryTracker()
        tracker.armAutomation()
        assertTrue(tracker.onConnected())
        assertEquals(1, tracker.count)
        // The landing consumed the pending flag: a later Connected (e.g. a
        // user reconnect) must not count again.
        assertFalse(tracker.onConnected())
        assertEquals(1, tracker.count)
    }

    @Test
    fun `user action clears a pending automation retry`() {
        val tracker = AutoRecoveryTracker()
        tracker.armAutomation()
        tracker.onUserAction()
        assertFalse(tracker.onConnected())
        assertEquals(0, tracker.count)
    }

    @Test
    fun `connected without a pending retry never counts`() {
        val tracker = AutoRecoveryTracker()
        assertFalse(tracker.onConnected())
        assertEquals(0, tracker.count)
    }

    @Test
    fun `successive automation recoveries accumulate`() {
        val tracker = AutoRecoveryTracker()
        tracker.armAutomation()
        tracker.onConnected()
        tracker.armAutomation()
        tracker.onConnected()
        assertEquals(2, tracker.count)
    }
}
