package com.warrenbrowse.vpn.lib.pushnotification

import kotlinx.coroutines.flow.Flow
import com.warrenbrowse.vpn.lib.model.NotificationUpdate

interface NotificationProvider<D> {
    val notifications: Flow<NotificationUpdate<D>>

    /**
     * Whether a burst of updates may collapse to the last one. True for a slot
     * that shows a state; false for a stream of distinct events, each of which
     * must reach the shade.
     */
    val debounced: Boolean
        get() = true
}
