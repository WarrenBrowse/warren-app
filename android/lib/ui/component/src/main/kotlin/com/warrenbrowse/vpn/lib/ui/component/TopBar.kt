@file:OptIn(ExperimentalMaterial3Api::class)

package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.TopAppBarScrollBehavior
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
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

// The desktop warren-wordmark.svg lockup viewport: the ears W and "arren" are
// baked as one glyph run, so the brand always reads as a single word.
private const val WARREN_LOCKUP_ASPECT = 17355f / 5720f

@Composable
fun WarrenTopBar(
    containerColor: Color,
    onSettingsClicked: (() -> Unit)?,
    onAccountClicked: (() -> Unit)?,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    iconTintColor: Color,
    isIconAndLogoVisible: Boolean = true,
    // The desktop header's first slot: the forum bell or lifebuoy, before the
    // account and settings glyphs. Null when the setting removed it.
    forumSlot: ForumHeaderSlot? = null,
    onForumClicked: (() -> Unit)? = null,
    // Wizard steps need a back affordance without losing the brand lockup this
    // bar exists for, so the slot lives here rather than forcing them onto the
    // titled small top bar.
    navigationIcon: @Composable () -> Unit = {},
) {
    TopAppBar(
        modifier = modifier.testTag(TOP_BAR_TEST_TAG),
        navigationIcon = navigationIcon,
        title = {
            if (isIconAndLogoVisible) {
                // Single "Warren" lockup (desktop warren-wordmark.svg): never
                // split into mark + text, the W IS the ears mark.
                BoxWithConstraints {
                    val fits = remember(maxWidth) { maxWidth > 120.dp }
                    if (fits) {
                        Icon(
                            painter = painterResource(id = R.drawable.wordmark_warren),
                            contentDescription = null, // Decorative brand lockup.
                            tint = iconTintColor,
                            modifier =
                                Modifier.height(Dimens.topBarLockupHeight)
                                    .width(Dimens.topBarLockupHeight * WARREN_LOCKUP_ASPECT),
                        )
                    }
                }
            }
        },
        actions = {
            if (forumSlot != null && onForumClicked != null) {
                ForumHeaderButton(
                    slot = forumSlot,
                    tint = iconTintColor,
                    enabled = enabled,
                    onClick = onForumClicked,
                )
            }

            if (onAccountClicked != null) {
                IconButton(
                    modifier = Modifier.testTag(TOP_BAR_ACCOUNT_BUTTON_TEST_TAG),
                    enabled = enabled,
                    onClick = onAccountClicked,
                ) {
                    Icon(
                        painter = painterResource(id = R.drawable.ic_account_outline),
                        tint = iconTintColor,
                        contentDescription = stringResource(id = R.string.settings_account),
                        modifier = Modifier.size(Dimens.topBarActionIconSize),
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
                        painter = painterResource(id = R.drawable.ic_settings_outline),
                        tint = iconTintColor,
                        contentDescription = stringResource(id = R.string.settings),
                        modifier = Modifier.size(Dimens.topBarActionIconSize),
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
    // Wired by the scaffold so the bar takes Material's scroll-linked container
    // transition. The title stays visible: a persistent small title is the
    // Android idiom, and only the container change was missing.
    scrollBehavior: TopAppBarScrollBehavior? = null,
) {
    TopAppBar(
        title = { Text(text = title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        navigationIcon = navigationIcon,
        colors =
            TopAppBarDefaults.topAppBarColors(
                containerColor = MaterialTheme.colorScheme.surface,
                actionIconContentColor = MaterialTheme.colorScheme.onSurface,
            ),
        actions = actions,
        scrollBehavior = scrollBehavior,
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
