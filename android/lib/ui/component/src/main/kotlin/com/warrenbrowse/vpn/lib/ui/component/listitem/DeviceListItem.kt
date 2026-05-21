package com.warrenbrowse.vpn.lib.ui.component.listitem

import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import com.warrenbrowse.vpn.lib.common.util.formatDate
import com.warrenbrowse.vpn.lib.model.Device
import com.warrenbrowse.vpn.lib.ui.designsystem.Hierarchy
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorMedium
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

@Composable
fun DeviceListItem(
    modifier: Modifier = Modifier,
    position: Position,
    device: Device,
    isLoading: Boolean,
    isCurrentDevice: Boolean = false,
    onDeviceRemovalClicked: () -> Unit,
) {
    WarrenListItem(
        modifier = modifier,
        hierarchy = Hierarchy.Parent,
        position = position,
        isEnabled = true,
        content = {
            TitleAndSubtitle(
                title = device.displayName(),
                subtitle = stringResource(id = R.string.created_x, device.creationDate.formatDate()),
            )
        },
        trailingContent = {
            if (isLoading) {
                WarrenCircularProgressIndicatorMedium(
                    modifier = Modifier.padding(Dimens.smallPadding)
                )
            } else if (isCurrentDevice) {
                Text(
                    modifier = Modifier.padding(Dimens.smallPadding),
                    text = stringResource(R.string.current_device),
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            } else {
                IconButton(onClick = onDeviceRemovalClicked) {
                    Icon(
                        imageVector = Icons.Rounded.Clear,
                        contentDescription = stringResource(id = R.string.remove_button),
                        tint = MaterialTheme.colorScheme.onPrimary,
                        modifier = Modifier.size(size = Dimens.deleteIconSize),
                    )
                }
            }
        },
    )
}
