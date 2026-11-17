package com.warrenbrowse.vpn.common.compose

import android.content.pm.PackageManager
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.booleanResource
import com.warrenbrowse.vpn.lib.ui.resource.R

@Composable
fun isTv(): Boolean {
    val context = LocalContext.current
    // A Binder call into the package manager: asked once per composition site, not per frame.
    val leanback =
        remember(context) {
            context.packageManager.hasSystemFeature(PackageManager.FEATURE_LEANBACK)
        }
    return booleanResource(R.bool.isTv) || leanback
}
