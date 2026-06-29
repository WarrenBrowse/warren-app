@file:OptIn(ExperimentalMaterial3Api::class)

package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.rounded.AccountCircle
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.tag.TOP_BAR_ACCOUNT_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.TOP_BAR_SETTINGS_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.TOP_BAR_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.positive

@Preview
@Composable
private fun PreviewTopBar() {
    AppTheme {
        WarrenTopBar(
            containerColor = MaterialTheme.colorScheme.positive,
            iconTintColor = MaterialTheme.colorScheme.onSurface,
            onSettingsClicked = null,
            onAccountClicked = {},
        )
    }
}

@Preview(widthDp = 260)
@Composable
private fun PreviewSlimTopBar() {
    AppTheme {
        WarrenTopBar(
            containerColor = MaterialTheme.colorScheme.positive,
            iconTintColor = MaterialTheme.colorScheme.onSurface,
            onSettingsClicked = null,
            onAccountClicked = {},
        )
    }
}

@Preview
@Composable
private fun PreviewNoIconAndLogoTopBar() {
    AppTheme {
        WarrenTopBar(
            containerColor = MaterialTheme.colorScheme.positive,
            iconTintColor = MaterialTheme.colorScheme.onSurface,
            isIconAndLogoVisible = false,
            onSettingsClicked = {},
            onAccountClicked = null,
        )
    }
}

@Preview
@Composable
private fun PreviewNothingTopBar() {
    AppTheme {
        WarrenTopBar(
            containerColor = MaterialTheme.colorScheme.positive,
            iconTintColor = MaterialTheme.colorScheme.onSurface,
            isIconAndLogoVisible = false,
            onSettingsClicked = null,
            onAccountClicked = null,
        )
    }
}

@Composable
fun WarrenTopBar(
    containerColor: Color,
    onSettingsClicked: (() -> Unit)?,
    onAccountClicked: (() -> Unit)?,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    iconTintColor: Color,
    isIconAndLogoVisible: Boolean = true,
    logoState: WarrenLogoState = WarrenLogoState.Exposed,
    logoTone: WarrenLogoTone = WarrenLogoTone.Light,
) {
    TopAppBar(
        modifier = modifier.testTag(TOP_BAR_TEST_TAG),
        title = {
            if (isIconAndLogoVisible) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    // Bula ears-in-burrow mark, stateful (ears only when connected),
                    // mirroring the desktop AppMainHeader logo.
                    WarrenLogoMark(state = logoState, tone = logoTone, height = 40.dp)
                    // Show the "WARREN" wordmark only when it fits, to avoid
                    // crowding the action icons on narrow widths.
                    BoxWithConstraints {
                        val shouldShowText = remember(maxWidth) { maxWidth > 120.dp }
                        if (shouldShowText) {
                            WarrenWordmark(
                                color = iconTintColor,
                                modifier = Modifier.padding(horizontal = Dimens.smallPadding),
                            )
                        }
                    }
                }
            }
        },
        actions = {
            if (onAccountClicked != null) {
                IconButton(
                    modifier = Modifier.testTag(TOP_BAR_ACCOUNT_BUTTON_TEST_TAG),
                    enabled = enabled,
                    onClick = onAccountClicked,
                ) {
                    Icon(
                        imageVector = Icons.Rounded.AccountCircle,
                        tint = iconTintColor,
                        contentDescription = stringResource(id = R.string.settings_account),
                    )
                }
            }

            if (onSettingsClicked != null) {
                IconButton(
                    modifier = Modifier.testTag(TOP_BAR_SETTINGS_BUTTON_TEST_TAG),
                    enabled = enabled,
                    onClick = onSettingsClicked,
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Settings,
                        tint = iconTintColor,
                        contentDescription = stringResource(id = R.string.settings),
                    )
                }
            }
        },
        colors =
            TopAppBarDefaults.topAppBarColors(
                containerColor = containerColor,
                actionIconContentColor = iconTintColor,
            ),
    )
}

@Composable
fun WarrenSmallTopBar(
    title: String,
    navigationIcon: @Composable () -> Unit = {},
    actions: @Composable RowScope.() -> Unit = {},
) {
    TopAppBar(
        title = { Text(text = title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        navigationIcon = navigationIcon,
        colors =
            TopAppBarDefaults.topAppBarColors(
                containerColor = MaterialTheme.colorScheme.surface,
                scrolledContainerColor = MaterialTheme.colorScheme.surface,
                actionIconContentColor = MaterialTheme.colorScheme.onSurface,
            ),
        actions = actions,
    )
}


/**
 * Second header row for the home screen, reproducing the desktop AppMainHeader:
 * the short public key with a copy action on the left, and the remaining
 * subscription time on the right, both over the same header background.
 */
@Composable
fun WarrenMainHeaderSubRow(
    containerColor: Color,
    tintColor: Color,
    shortPubkey: String?,
    timeLeft: String?,
    onCopyPubkey: (() -> Unit)?,
) {
    Row(
        modifier =
            Modifier.fillMaxWidth()
                .background(containerColor)
                .padding(
                    start = Dimens.mediumPadding,
                    end = Dimens.smallPadding,
                    bottom = Dimens.smallPadding,
                ),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (shortPubkey != null) {
            Text(
                text = shortPubkey,
                style = MaterialTheme.typography.labelMedium,
                color = tintColor,
                maxLines = 1,
            )
            if (onCopyPubkey != null) {
                IconButton(onClick = onCopyPubkey, modifier = Modifier.size(Dimens.mediumPadding * 2)) {
                    Icon(
                        imageVector = Icons.Rounded.ContentCopy,
                        contentDescription = stringResource(R.string.copy),
                        tint = tintColor,
                        modifier = Modifier.size(Dimens.mediumPadding),
                    )
                }
            }
        }
        Spacer(modifier = Modifier.weight(1f))
        if (timeLeft != null) {
            Text(
                text = timeLeft,
                style = MaterialTheme.typography.labelMedium,
                color = tintColor,
                maxLines = 1,
            )
        }
    }
}
