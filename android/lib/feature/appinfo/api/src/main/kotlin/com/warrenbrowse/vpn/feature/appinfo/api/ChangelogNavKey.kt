package com.warrenbrowse.vpn.feature.appinfo.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2

@Parcelize data class ChangelogNavKey(val isModal: Boolean = false) : NavKey2
