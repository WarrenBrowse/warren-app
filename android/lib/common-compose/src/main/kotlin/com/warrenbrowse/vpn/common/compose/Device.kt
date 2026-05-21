package com.warrenbrowse.vpn.common.compose

import android.content.pm.PackageManager
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.booleanResource
import com.warrenbrowse.vpn.lib.ui.resource.R

@Composable
fun isTv(): Boolean {
    return booleanResource(R.bool.isTv) ||
        LocalContext.current.packageManager.hasSystemFeature(PackageManager.FEATURE_LEANBACK)
}
