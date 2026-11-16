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
 * The glyph a home banner leads with, in place of a coloured dot (desktop `NotificationLeadIcon`,
 * mockup 8d). An announcement stays quiet whatever its level: the operator's words carry it.
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
            kind == NotificationKind.Announcement -> NotificationTone.Secondary
            level == StatusLevel.Error -> NotificationTone.Disconnect
            kind == null && (level == StatusLevel.Info || level == StatusLevel.None) ->
                NotificationTone.Secondary
            else -> NotificationTone.Badge
        }
    return NotificationLead(glyph, tone)
}
