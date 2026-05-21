package com.warrenbrowse.vpn.feature.multihop.api

import kotlinx.parcelize.Parcelize
import com.warrenbrowse.vpn.core.NavKey2

@Parcelize data class MultihopNavKey(val isModal: Boolean = false) : NavKey2
