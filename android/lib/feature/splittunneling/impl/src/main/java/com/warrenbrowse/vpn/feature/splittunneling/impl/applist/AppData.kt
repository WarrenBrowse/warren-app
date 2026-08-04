package com.warrenbrowse.vpn.feature.splittunneling.impl.applist

import com.warrenbrowse.vpn.lib.model.PackageName

data class AppData(
    val packageName: PackageName,
    val iconRes: Int,
    val name: String,
    val isSystemApp: Boolean = false,
)
