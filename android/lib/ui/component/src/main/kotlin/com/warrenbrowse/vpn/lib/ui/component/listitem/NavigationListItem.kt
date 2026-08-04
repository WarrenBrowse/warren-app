package com.warrenbrowse.vpn.lib.ui.component.listitem

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Error
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextDirection
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.lib.ui.component.R
import com.warrenbrowse.vpn.lib.ui.component.preview.PreviewColumn
import com.warrenbrowse.vpn.lib.ui.designsystem.Hierarchy
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.Position
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

@Preview
@Composable
private fun PreviewNavigationListItem() {
    PreviewColumn(Modifier.background(MaterialTheme.colorScheme.surface)) {
        NavigationListItem(title = "Navigation sample", showWarning = false, onClick = {})
        NavigationListItem(
            hierarchy = Hierarchy.Child1,
            title = "Navigation sample",
            showWarning = true,
            onClick = {},
        )
        NavigationListItem(
            hierarchy = Hierarchy.Child1,
            title = "Navigation sample",
            subtitle = "Navigation sample",
            showWarning = false,
            onClick = {},
        )
    }
}

@Suppress("ComposableLambdaParameterNaming")
@Composable
fun NavigationListItem(
    modifier: Modifier = Modifier,
    hierarchy: Hierarchy = Hierarchy.Parent,
    position: Position = Position.Single,
    title: String,
    subtitle: String? = null,
    subTitleTextDirection: TextDirection = TextDirection.Unspecified,
    trailingText: String? = null,
    showWarning: Boolean = false,
    isRowEnabled: Boolean = true,
    onClick: () -> Unit,
    testTag: String? = null,
) {
    WarrenListItem(
        modifier = modifier,
        hierarchy = hierarchy,
        position = position,
        onClick = onClick,
        isEnabled = isRowEnabled,
        testTag = testTag,
        leadingContent = {
            if (showWarning) {
                Icon(
                    imageVector = Icons.Rounded.Error,
                    modifier = Modifier.padding(end = Dimens.smallPadding),
                    contentDescription = stringResource(R.string.warning),
                    tint = MaterialTheme.colorScheme.error,
                )
            }
        },
        content = {
            TitleAndSubtitle(
                title = title,
                subtitle = subtitle,
                subTitleTextDirection = subTitleTextDirection,
            )
        },
        trailingContent = {
            // A status value ("On"/"Off") belongs right-aligned before the
            // chevron, where the rows form a scannable value column and stay
            // single-height; the subtitle slot is for a second line of label.
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (trailingText != null) {
                    Text(
                        text = trailingText,
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(end = Dimens.smallPadding),
                    )
                }
                Icon(
                    modifier = Modifier,
                    imageVector = Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                    contentDescription = stringResource(R.string.navigate),
                )
            }
        },
    )
}
