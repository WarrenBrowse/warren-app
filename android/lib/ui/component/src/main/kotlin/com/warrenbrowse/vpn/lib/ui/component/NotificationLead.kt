package com.warrenbrowse.vpn.lib.ui.component

import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.StatusLevel

/** What a banner is about, when that and not its level picks the glyph it leads with. */
enum class NotificationKind {
    Update,
    Announcement,
}

enum class NotificationGlyph {
    Download,
    Megaphone,
    Alert,
    Info,
}

/** The home surface a lead glyph is tinted with. */
enum class NotificationTone {
    Update,
    Announcement,
    Badge,
    Secondary,
    Disconnect,
}

data class NotificationLead(val glyph: NotificationGlyph, val tone: NotificationTone)

fun InAppNotification.bannerKind(): NotificationKind? =
    when (this) {
        is InAppNotification.UpdateAvailable -> NotificationKind.Update
        is InAppNotification.LaunchAnnouncement -> NotificationKind.Announcement
        else -> null
    }

/**
 * The glyph a home banner leads with, in place of a coloured dot (desktop `NotificationLeadIcon`
 * and `noticeIconColor`). An update and an announcement keep their own colour whatever their level.
 */
fun notificationLead(kind: NotificationKind?, level: StatusLevel): NotificationLead {
    val glyph =
        when (kind) {
            NotificationKind.Update -> NotificationGlyph.Download
            NotificationKind.Announcement -> NotificationGlyph.Megaphone
            null ->
                when (level) {
                    StatusLevel.Error,
                    StatusLevel.Warning -> NotificationGlyph.Alert
                    StatusLevel.Info,
                    StatusLevel.None -> NotificationGlyph.Info
                }
        }
    val tone =
        when {
            kind == NotificationKind.Update -> NotificationTone.Update
            kind == NotificationKind.Announcement -> NotificationTone.Announcement
            level == StatusLevel.Error -> NotificationTone.Disconnect
            level == StatusLevel.Warning -> NotificationTone.Badge
            else -> NotificationTone.Secondary
        }
    return NotificationLead(glyph, tone)
}
