package com.warrenbrowse.vpn.lib.ui.component.listitem

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.component.preview.PreviewColumn
import com.warrenbrowse.vpn.lib.ui.designsystem.Hierarchy
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenCircularProgressIndicatorMedium
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenTextButton
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.positive

@Preview
@Composable
private fun PreviewDeviceListItem() {
    PreviewColumn(Modifier.background(MaterialTheme.colorScheme.surface)) {
        DeviceListItem(
            position = Position.Top,
            name = "brave otter",
            thisDeviceBadge = "This device",
            platformLine = "Android · 1.1.40",
            activity = "Active today",
            actionLabel = "Log out",
            isLoading = false,
            onRename = {},
            onAction = {},
        )
        DeviceListItem(
            position = Position.Bottom,
            name = "quiet heron",
            thisDeviceBadge = null,
            platformLine = "Windows · 1.1.38",
            activity = "Last active 10/21/26",
            actionLabel = "Disconnect",
            isLoading = true,
            onRename = {},
            onAction = {},
        )
    }
}

/**
 * One device of the account (warren-core doc 115): its name, with a badge when it is this device,
 * its platform and version, and when it was last active. Tapping the row renames it; the trailing
 * action disconnects it, or logs this device out.
 */
@Composable
fun DeviceListItem(
    modifier: Modifier = Modifier,
    position: Position,
    name: String,
    thisDeviceBadge: String?,
    platformLine: String,
    activity: String?,
    actionLabel: String,
    isLoading: Boolean,
    onRename: () -> Unit,
    onAction: () -> Unit,
) {
    WarrenListItem(
        modifier = modifier,
        hierarchy = Hierarchy.Parent,
        position = position,
        isEnabled = !isLoading,
        onClick = onRename,
        content = {
            Column(
                modifier = Modifier.padding(vertical = Dimens.smallPadding),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
                ) {
                    Text(text = name, style = MaterialTheme.typography.titleSmall)
                    if (thisDeviceBadge != null) {
                        Text(
                            text = thisDeviceBadge,
                            style = MaterialTheme.typography.labelSmall,
                            color = Color.White,
                            modifier =
                                Modifier.background(
                                        MaterialTheme.colorScheme.positive,
                                        RoundedCornerShape(4.dp),
                                    )
                                    .padding(horizontal = Dimens.tinyPadding, vertical = 2.dp),
                        )
                    }
                }
                SecondaryLine(platformLine)
                activity?.let { SecondaryLine(it) }
            }
        },
        trailingContent = {
            if (isLoading) {
                WarrenCircularProgressIndicatorMedium(
                    modifier = Modifier.padding(Dimens.smallPadding)
                )
            } else {
                WarrenTextButton(
                    onClick = onAction,
                    colors =
                        ButtonDefaults.textButtonColors(
                            contentColor = MaterialTheme.colorScheme.error
                        ),
                ) {
                    Text(text = actionLabel, style = MaterialTheme.typography.titleSmall)
                }
            }
        },
    )
}

@Composable
private fun SecondaryLine(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
    )
}
