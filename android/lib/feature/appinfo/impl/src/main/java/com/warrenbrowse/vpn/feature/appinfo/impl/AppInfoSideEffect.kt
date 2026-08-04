package com.warrenbrowse.vpn.feature.appinfo.impl

import android.net.Uri

sealed interface AppInfoSideEffect {
    data class OpenUri(val uri: Uri, val errorMessage: String) : AppInfoSideEffect
}
