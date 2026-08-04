package com.warrenbrowse.vpn.lib.ui.component.listitem

import androidx.compose.foundation.layout.Box
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
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.lib.ui.designsystem.Hierarchy
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

@Preview
@Composable
private fun PreviewInfoListItem() {
    AppTheme {
        InfoListItem(
            title = "Information row title",
            isEnabled = true,
            onCellClicked = {},
            onInfoClicked = {},
        )
    }
}

@Composable
fun InfoListItem(
    modifier: Modifier = Modifier,
    hierarchy: Hierarchy = Hierarchy.Parent,
    position: Position = Position.Single,
    title: String,
    isEnabled: Boolean = true,
    backgroundAlpha: Float = 1f,
    iconContentDescription: String? = null,
    onCellClicked: (() -> Unit)? = null,
    onInfoClicked: (() -> Unit)? = null,
    testTag: String? = null,
) {
    WarrenListItem(
        modifier = modifier,
        hierarchy = hierarchy,
        position = position,
        isEnabled = isEnabled,
        onClick = onCellClicked,
        testTag = testTag,
        backgroundAlpha = backgroundAlpha,
        content = { Text(title) },
        trailingContent = {
            if (onInfoClicked != null) {
                Box(
                    modifier =
                        Modifier.width(ListItemComponentTokens.infoIconContainerWidth)
                            .padding(end = Dimens.smallPadding)
                            .fillMaxHeight(),
                    contentAlignment = Alignment.Center,
                ) {
                    IconButton(onClick = onInfoClicked) {
                        Icon(
                            imageVector = Icons.Rounded.Info,
                            contentDescription = iconContentDescription,
                        )
                    }
                }
            }
        },
    )
}
