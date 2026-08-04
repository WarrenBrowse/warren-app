package com.warrenbrowse.vpn.lib.ui.component.listitem

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.lib.ui.component.R
import com.warrenbrowse.vpn.lib.ui.designsystem.Hierarchy
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenSwitch
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.util.applyIfNotNull

@Preview
@Composable
private fun PreviewSwitchListItem() {
    AppTheme {
        SwitchListItem(
            title = "Checkbox Title",
            isEnabled = true,
            isToggled = true,
            onCellClicked = {},
            onInfoClicked = {},
        )
    }
}

@Composable
fun SwitchListItem(
    modifier: Modifier = Modifier,
    hierarchy: Hierarchy = Hierarchy.Parent,
    position: Position = Position.Single,
    title: String,
    isToggled: Boolean,
    isEnabled: Boolean = true,
    testTag: String? = null,
    backgroundAlpha: Float = 1f,
    onCellClicked: (Boolean) -> Unit,
    onInfoClicked: (() -> Unit)? = null,
) {
    WarrenListItem(
        modifier = modifier.applyIfNotNull(onInfoClicked) { focusProperties { canFocus = false } },
        hierarchy = hierarchy,
        position = position,
        isEnabled = isEnabled,
        testTag = testTag,
        backgroundAlpha = backgroundAlpha,
        onClick = { onCellClicked(!isToggled) },
        content = { Text(title) },
        trailingContent = {
            Row(
                modifier = Modifier.fillMaxHeight(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (onInfoClicked != null) {
                    Box(
                        modifier =
                            Modifier.width(ListItemComponentTokens.infoIconContainerWidth)
                                .fillMaxHeight(),
                        contentAlignment = Alignment.Center,
                    ) {
                        IconButton(onClick = onInfoClicked) {
                            Icon(
                                imageVector = Icons.Rounded.Info,
                                contentDescription = stringResource(id = R.string.more_information),
                            )
                        }
                    }
                }

                Box(modifier = Modifier.fillMaxHeight().padding(end = Dimens.smallPadding)) {
                    WarrenSwitch(
                        modifier = Modifier.align(Alignment.Center),
                        checked = isToggled,
                        onCheckedChange = onCellClicked,
                        enabled = isEnabled,
                    )
                }
            }
        },
    )
}
