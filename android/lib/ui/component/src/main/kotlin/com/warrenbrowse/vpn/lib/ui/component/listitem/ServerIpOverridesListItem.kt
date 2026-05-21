package com.warrenbrowse.vpn.lib.ui.component.listitem

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.lib.ui.component.R
import com.warrenbrowse.vpn.lib.ui.component.preview.PreviewColumn
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorSmall
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.positive

@Preview
@Composable
private fun PreviewServerIpOverridesListItem() {
    PreviewColumn {
        ServerIpOverridesListItem(active = true)
        ServerIpOverridesListItem(active = false)
        ServerIpOverridesListItem(active = null)
    }
}

@Composable
fun ServerIpOverridesListItem(
    active: Boolean?,
    modifier: Modifier = Modifier,
    activeColor: Color = MaterialTheme.colorScheme.positive,
    inactiveColor: Color = MaterialTheme.colorScheme.error,
) {
    WarrenListItem(
        modifier = modifier,
        isEnabled = active == true,
        leadingContent = {
            if (active == null) {
                WarrenCircularProgressIndicatorSmall()
            } else {
                Box(
                    modifier =
                        Modifier.size(Dimens.relayCircleSize)
                            .background(
                                color =
                                    when {
                                        active -> activeColor
                                        else -> inactiveColor
                                    },
                                shape = CircleShape,
                            )
                )
            }
        },
        content = {
            if (active != null) {
                Text(
                    text =
                        if (active) stringResource(id = R.string.server_ip_overrides_active)
                        else stringResource(id = R.string.server_ip_overrides_inactive),
                    modifier = Modifier.padding(horizontal = Dimens.smallPadding),
                )
            }
        },
    )
}
