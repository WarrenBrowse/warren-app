package com.warrenbrowse.vpn.lib.ui.component.listitem

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenListItem
import com.warrenbrowse.vpn.lib.ui.designsystem.Position

@Composable
fun EditCustomListListItem(
    modifier: Modifier = Modifier,
    title: String,
    subtitle: String,
    position: Position,
    onClick: () -> Unit,
) {
    WarrenListItem(
        modifier = modifier,
        position = position,
        content = { TitleAndSubtitle(title = title, subtitle = subtitle) },
        onClick = onClick,
    )
}
