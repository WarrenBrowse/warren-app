package com.warrenbrowse.vpn.feature.daita.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2

@Parcelize data class DaitaNavKey(val isModal: Boolean = false) : NavKey2
