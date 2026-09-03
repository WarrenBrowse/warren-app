package com.warrenbrowse.vpn.lib.model

import kotlin.test.assertContains
import kotlin.test.assertFalse
import org.junit.jupiter.api.Test

/**
 * The voucher code is a bearer token worth a month of service, and the Rust
 * twin (`warren_announcements_updater::DisplayAnnouncement`) hand-writes its
 * Debug rendering to keep it out of one. A data class renders every field it
 * holds, so one `Logger.d { "banner: $notification" }` anywhere on the way to
 * the card would put the code in logcat, and `collectProblemReport` pipes a
 * logcat dump straight into what the user uploads.
 */
class WarrenAnnouncementTest {

    private val announcement =
        WarrenAnnouncement(
            id = "a1",
            headline = "Warren production is open",
            body = "body",
            level = WarrenNoticeLevel.INFO,
            voucherCampaignId = "launch",
            voucherCode = "ABCD1234EFGH5678",
        )

    @Test
    fun renders_that_a_code_is_held_and_never_the_code_itself() {
        val rendered = "$announcement"

        assertFalse(rendered.contains("ABCD1234EFGH5678"), "the code is renderable: $rendered")
        assertContains(rendered, "hasVoucherCode=true")
        assertContains(rendered, "a1")
    }

    @Test
    fun says_when_no_code_is_held() {
        val rendered = "${announcement.copy(voucherCode = null)}"

        assertContains(rendered, "hasVoucherCode=false")
    }

    @Test
    fun the_banner_wrapping_it_cannot_render_the_code_either() {
        val rendered = "${InAppNotification.LaunchAnnouncement(announcement)}"

        assertFalse(rendered.contains("ABCD1234EFGH5678"), "the code is renderable: $rendered")
    }
}
