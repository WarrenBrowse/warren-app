package com.warrenbrowse.vpn.lib.model

sealed interface NotificationChannel {
    val id: NotificationChannelId

    data object TunnelUpdates : NotificationChannel {
        private const val CHANNEL_ID = "vpn_tunnel_status"
        override val id: NotificationChannelId = NotificationChannelId(CHANNEL_ID)
    }

    /**
     * New activity on the community forum (a reply, a like, a mention). Low
     * importance: a badge in the shade, no sound and no heads-up, the band the
     * desktop keeps for the same banner so the system setting can silence it.
     */
    data object ForumActivity : NotificationChannel {
        private const val CHANNEL_ID = "forum_activity"
        override val id: NotificationChannelId = NotificationChannelId(CHANNEL_ID)
    }
}
