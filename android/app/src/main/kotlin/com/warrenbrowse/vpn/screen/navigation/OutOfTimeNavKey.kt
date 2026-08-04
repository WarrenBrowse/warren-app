package com.warrenbrowse.vpn.screen.navigation

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2

/**
 * The out-of-time gate, raised over the main flow when the subscription lapses
 * (desktop `RoutePath.expired`). A destination rather than a UI-wide branch, so
 * Settings and the account page stay reachable from it.
 */
@Parcelize object OutOfTimeNavKey : NavKey2
