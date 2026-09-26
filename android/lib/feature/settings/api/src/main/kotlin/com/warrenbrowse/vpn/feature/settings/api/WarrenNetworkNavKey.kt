package com.warrenbrowse.vpn.feature.settings.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * Navigation key for the Warren network page: the live transparency figures of the network (people
 * connected, each exit's load) and how they are computed. Reached from settings and from the load
 * shown on the connection card.
 */
@Parcelize data object WarrenNetworkNavKey : NavKey2
