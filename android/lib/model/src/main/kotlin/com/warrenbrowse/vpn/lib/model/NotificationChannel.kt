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

    /**
     * Port-forward abuse warnings on the account (warren-core doc 105). Default
     * importance: three of them revoke the account, so the first must be seen,
     * unlike the forum's low band.
     */
    data object AccountStanding : NotificationChannel {
        private const val CHANNEL_ID = "account_standing"
        override val id: NotificationChannelId = NotificationChannelId(CHANNEL_ID)
    }
}
