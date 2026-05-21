package com.warrenbrowse.vpn.feature.location.impl

import kotlinx.coroutines.channels.Channel
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayListType

typealias ScrollEvent = Pair<RelayListType, RelayItem>

class RelayListScrollConnection {
    val scrollEvents: Channel<ScrollEvent> = Channel()
}
