package com.warrenbrowse.vpn.lib.ui.component

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.StatusLevel
import com.warrenbrowse.vpn.lib.model.WarrenAnnouncement
import com.warrenbrowse.vpn.lib.model.WarrenNoticeLevel
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test

/**
 * The glyph a home banner leads with in place of the coloured dot (desktop
 * `NotificationLeadIcon`, mockup 8d): the amber download for an update, the
 * megaphone for an announcement, otherwise an alert or an info by level.
 */
class NotificationLeadTest {

    @Test
    fun `an update leads with the download glyph in the badge amber`() {
        val update = InAppNotification.UpdateAvailable("2026.11")
        assertEquals(NotificationKind.Update, update.bannerKind())
        assertEquals(
            NotificationLead(NotificationGlyph.Download, NotificationTone.Badge),
            notificationLead(update.bannerKind(), update.statusLevel),
        )
    }

    @Test
    fun `an announcement leads with the megaphone in the secondary text, whatever its level`() {
        val announcement =
            InAppNotification.LaunchAnnouncement(
                WarrenAnnouncement(
                    id = "a",
                    headline = "Hello",
                    body = "World",
                    level = WarrenNoticeLevel.WARNING,
                )
            )
        assertEquals(NotificationKind.Announcement, announcement.bannerKind())
        assertEquals(
            NotificationLead(NotificationGlyph.Megaphone, NotificationTone.Secondary),
            notificationLead(announcement.bannerKind(), announcement.statusLevel),
        )
    }

    @Test
    fun `any other banner leads with its level`() {
        assertNull(InAppNotification.HostOffline.bannerKind())
        assertEquals(
            NotificationLead(NotificationGlyph.Alert, NotificationTone.Disconnect),
            notificationLead(null, StatusLevel.Error),
        )
        assertEquals(
            NotificationLead(NotificationGlyph.Alert, NotificationTone.Badge),
            notificationLead(null, StatusLevel.Warning),
        )
        assertEquals(
            NotificationLead(NotificationGlyph.Info, NotificationTone.Secondary),
            notificationLead(null, StatusLevel.Info),
        )
        assertEquals(
            NotificationLead(NotificationGlyph.Info, NotificationTone.Secondary),
            notificationLead(null, StatusLevel.None),
        )
    }

    @Test
    fun `an update that failed reads as an error`() {
        assertEquals(
            NotificationLead(NotificationGlyph.Download, NotificationTone.Disconnect),
            notificationLead(NotificationKind.Update, StatusLevel.Error),
        )
    }
}
