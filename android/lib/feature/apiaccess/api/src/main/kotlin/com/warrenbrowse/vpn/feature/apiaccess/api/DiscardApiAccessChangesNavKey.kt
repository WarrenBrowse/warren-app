package com.warrenbrowse.vpn.feature.apiaccess.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.NavResult

@Parcelize object DiscardApiAccessChangesNavKey : NavKey2

@Parcelize data object DiscardApiAccessChangesConfirmedNavResult : NavResult
