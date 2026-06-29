package com.warrenbrowse.vpn.lib.tv

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AccountCircle
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusDirection
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.tooling.preview.PreviewParameter
import androidx.compose.ui.tooling.preview.PreviewParameterProvider
import androidx.compose.ui.unit.dp
import androidx.tv.material3.DrawerValue
import androidx.tv.material3.ModalNavigationDrawer
import androidx.tv.material3.NavigationDrawerItem
import androidx.tv.material3.NavigationDrawerItemDefaults
import androidx.tv.material3.NavigationDrawerScope
import androidx.tv.material3.rememberDrawerState
import com.warrenbrowse.vpn.lib.ui.component.WarrenLogoMark
import com.warrenbrowse.vpn.lib.ui.component.WarrenLogoState
import com.warrenbrowse.vpn.lib.ui.component.WarrenLogoTone
import com.warrenbrowse.vpn.lib.ui.component.WarrenWordmark
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens

private class DrawerValueProvider : PreviewParameterProvider<DrawerValue> {
    override val values: Sequence<DrawerValue>
        get() = sequenceOf(DrawerValue.Closed, DrawerValue.Open)
}

@Preview("Closed|Open")
@Composable
fun PreviewNavigationDrawerTvClosed(
    @PreviewParameter(DrawerValueProvider::class) drawerValue: DrawerValue
) {
    AppTheme {
        NavigationDrawerTv(
            initialDrawerValue = drawerValue,
            onSettingsClick = {},
            onAccountClick = {},
        ) {}
    }
}

@OptIn(ExperimentalComposeUiApi::class)
@Composable
@Suppress("LongMethod")
fun NavigationDrawerTv(
    initialDrawerValue: DrawerValue = DrawerValue.Closed,
    onSettingsClick: (() -> Unit),
    onAccountClick: (() -> Unit),
    content: @Composable () -> Unit,
) {
    val drawerState = rememberDrawerState(initialDrawerValue)
    val brush = remember { Brush.horizontalGradient(listOf(Color.Black, Color.Transparent)) }

    val focusManager = LocalFocusManager.current

    if (drawerState.currentValue == DrawerValue.Open) {
        BackHandler(
            onBack = {
                drawerState.setValue(DrawerValue.Closed)
                focusManager.moveFocus(FocusDirection.Right)
            }
        )
    }

    ModalNavigationDrawer(
        drawerState = drawerState,
        scrimBrush = brush,
        drawerContent = {
            Box(
                Modifier.fillMaxHeight()
                    .background(brush)
                    .padding(
                        top = Dimens.screenBottomMargin,
                        bottom = Dimens.screenBottomMargin,
                        start = Dimens.tvDrawerHorizontalPadding,
                        end = Dimens.tvDrawerHorizontalPadding,
                    )
                    .focusRestorer()
                    .focusGroup()
            ) {
                val animatedPadding =
                    animateDpAsState(
                        if (hasFocus) Dimens.tvDrawerHeaderWithFocusStartPadding
                        else Dimens.tvDrawerHeaderStartPadding
                    )

                NavigationDrawerTvHeader(
                    modifier =
                        Modifier.align(Alignment.TopStart).padding(start = animatedPadding.value),
                    isExpanded = hasFocus,
                )
                DrawerItemTv(
                    modifier = Modifier.align(Alignment.CenterStart),
                    icon = Icons.Rounded.AccountCircle,
                    text = stringResource(R.string.settings_account),
                    onClick = onAccountClick,
                )
                DrawerItemTv(
                    modifier = Modifier.align(Alignment.BottomStart),
                    icon = Icons.Rounded.Settings,
                    text = stringResource(R.string.settings),
                    onClick = onSettingsClick,
                )
            }
        },
        content = content,
    )
}

@Composable
private fun NavigationDrawerScope.DrawerItemTv(
    modifier: Modifier = Modifier,
    icon: ImageVector,
    text: String,
    onClick: () -> Unit,
) {
    NavigationDrawerItem(
        modifier = modifier,
        onClick = onClick,
        selected = false,
        leadingContent = {
            Icon(
                tint = MaterialTheme.colorScheme.onPrimary,
                imageVector = icon,
                contentDescription = null,
            )
        },
    ) {
        Text(
            modifier = Modifier.fillMaxWidth(),
            color = MaterialTheme.colorScheme.onPrimary,
            text = text,
            maxLines = 1,
            overflow = TextOverflow.Clip,
        )
    }
}

@Composable
private fun NavigationDrawerTvHeader(
    modifier: Modifier = Modifier,
    isExpanded: Boolean,
) {
    Column(
        modifier =
            modifier.width(
                if (isExpanded) NavigationDrawerItemDefaults.ExpandedDrawerItemWidth
                else NavigationDrawerItemDefaults.CollapsedDrawerItemWidth
            )
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Dimens.tvMullvadLogoTextStartPadding),
        ) {
            WarrenLogoMark(
                state = WarrenLogoState.Exposed,
                tone = WarrenLogoTone.Light,
                height = Dimens.mediumIconSize,
            )
            if (isExpanded) {
                WarrenWordmark(color = Color.White)
            }
        }
        Spacer(Modifier.height(8.dp))
    }
}
