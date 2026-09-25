package com.warrenbrowse.vpn.lib.model

sealed interface Notification {
    val actions: List<NotificationAction>
    val ongoing: Boolean
    val channelId: NotificationChannelId

    data class Tunnel(
        override val channelId: NotificationChannelId,
        val state: NotificationTunnelState,
        override val actions: List<NotificationAction.Tunnel>,
        override val ongoing: Boolean,
    ) : Notification

    /**
     * Unread community-forum activity, [unread] being all the broadcast digest
     * carries: nothing here names a topic or an author, which is what keeps the
     * badge free of any per-user request.
     */
    data class Forum(override val channelId: NotificationChannelId, val unread: Int) : Notification {
        override val actions: List<NotificationAction> = emptyList()
        override val ongoing: Boolean = false
    }

    /**
     * A new port-forward strike on the account, raised once per strike: Rust
     * hands each one over exactly once, and remembers across process deaths
     * which it already handed over.
     */
    data class AccountStrike(
        override val channelId: NotificationChannelId,
        val notice: StrikeNotice,
    ) : Notification {
        override val actions: List<NotificationAction> = emptyList()
        override val ongoing: Boolean = false
    }
}
