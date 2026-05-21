package com.warrenbrowse.vpn.feature.apiaccess.impl.util

import android.content.res.Resources
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.warrenbrowse.vpn.lib.model.ApiAccessMethod
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodSetting
import com.warrenbrowse.vpn.lib.ui.resource.R

@Composable
fun ApiAccessMethodSetting?.toDisplayName() =
    when (this?.apiAccessMethod) {
        ApiAccessMethod.Direct -> stringResource(R.string.direct)
        ApiAccessMethod.Bridges,
        ApiAccessMethod.EncryptedDns,
        is ApiAccessMethod.CustomProxy -> this.name.toString()
        null -> "-"
    }

fun ApiAccessMethodSetting.toDisplayName(resources: Resources) =
    when (this.apiAccessMethod) {
        ApiAccessMethod.Direct -> resources.getString(R.string.direct)
        ApiAccessMethod.Bridges,
        ApiAccessMethod.EncryptedDns,
        is ApiAccessMethod.CustomProxy -> this.name.toString()
    }
