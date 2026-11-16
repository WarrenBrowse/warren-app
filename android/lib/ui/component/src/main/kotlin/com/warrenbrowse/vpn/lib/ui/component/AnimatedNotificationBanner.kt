package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.tween
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.draw.dropShadow
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.shadow.Shadow
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.StatusLevel
import com.warrenbrowse.vpn.lib.ui.tag.NOTIFICATION_BANNER_ACTION_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.NOTIFICATION_BANNER_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.NOTIFICATION_BANNER_TEXT_ACTION_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.HomeTypography
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces

private const val NOTIFICATION_MOTION_MILLIS = 250

// Mockup 8d's banner: the toolbar's own glass under it, on its right edge, reaching as far left
// as the desktop's 272 px card where the screen allows.
private val BANNER_MAX_WIDTH = 272.dp
private val BANNER_RADIUS = 12.dp
private val BANNER_LINE = 1.dp
private val BANNER_PADDING_VERTICAL = 10.dp
private val BANNER_PADDING_HORIZONTAL = 12.dp
private val BANNER_GAP = 10.dp
private val BANNER_TEXT_GAP = 1.dp
private val BANNER_GLYPH = 16.dp
private val BANNER_ACTION_BOX = 24.dp
private val BANNER_ACTION_RADIUS = 6.dp
private val BANNER_TRAILING_GAP = 4.dp
private val BANNER_SHADOW_BLUR = 24.dp
private val BANNER_SHADOW_SPREAD = (-8).dp
private val BANNER_SHADOW_OFFSET = 8.dp
private const val BANNER_SHADOW_ALPHA = 0.45f
private val BUTTON_RADIUS = 8.dp
private val BUTTON_PADDING_VERTICAL = 6.dp
private val BUTTON_PADDING_HORIZONTAL = 10.dp

@Composable
fun AnimatedNotificationBanner(
    modifier: Modifier = Modifier,
    notificationModifier: Modifier = Modifier,
    notification: InAppNotification?,
    isPlayBuild: Boolean,
    openAppListing: () -> Unit,
    contentFocusRequester: FocusRequester,
    onClickShowAccount: () -> Unit,
    onClickShowChangelog: () -> Unit,
    onClickShowAndroid16UpgradeInfo: () -> Unit,
    onClickDismissChangelog: () -> Unit,
    onClickDismissAndroid16UpgradeWarning: () -> Unit,
    onClickDismissUpdateAvailable: () -> Unit,
    onClickDismissExitSwitched: () -> Unit,
    onClickDismissNotice: () -> Unit,
    onClickDismissAnnouncement: () -> Unit,
    onClickReEnableAfterStandDown: () -> Unit,
    onClickManageDevices: () -> Unit,
) {
    // Fix for animating to invisible state
    val previous = rememberPrevious(current = notification, shouldUpdate = { _, _ -> true })

    val isVisible = notification != null

    val isNotificationDismissed = !isVisible && previous != null
    val notificationHasFocus = remember { mutableStateOf(false) }
    LaunchedEffect(isNotificationDismissed) {
        // If the notification is dismissed, we want to reset the previous notification
        if (isNotificationDismissed && notificationHasFocus.value) {
            contentFocusRequester.requestFocus()
        }
    }
    AnimatedVisibility(
        // clipToBounds guarantees the slide never draws outside the banner's
        // own slot: it can never ride over the transparent header above it.
        modifier = modifier.clipToBounds().onFocusChanged { notificationHasFocus.value = it.hasFocus },
        visible = isVisible,
        // Desktop NotificationBanner: a 250 ms translate, not a spring.
        enter = slideInVertically(tween(NOTIFICATION_MOTION_MILLIS), initialOffsetY = { -it }),
        exit = slideOutVertically(tween(NOTIFICATION_MOTION_MILLIS), targetOffsetY = { -it }),
    ) {
        val visibleNotification = notification ?: previous
        if (visibleNotification != null)
            Notification(
                modifier = notificationModifier,
                lead = notificationLead(visibleNotification.bannerKind(), visibleNotification.statusLevel),
                notificationBannerData = visibleNotification.toNotificationData(
                    isPlayBuild = isPlayBuild,
                    openAppListing = openAppListing,
                    onClickShowAccount = onClickShowAccount,
                    onClickShowChangelog = onClickShowChangelog,
                    onClickShowAndroid16UpgradeInfo = onClickShowAndroid16UpgradeInfo,
                    onClickDismissChangelog = onClickDismissChangelog,
                    onClickDismissAndroid16UpgradeWarning = onClickDismissAndroid16UpgradeWarning,
                    onClickDismissUpdateAvailable = onClickDismissUpdateAvailable,
                    onClickDismissExitSwitched = onClickDismissExitSwitched,
                    onClickDismissNotice = onClickDismissNotice,
                    onClickDismissAnnouncement = onClickDismissAnnouncement,
                    onClickReEnableAfterStandDown = onClickReEnableAfterStandDown,
                    onClickManageDevices = onClickManageDevices,
                ),
            )
    }
}

@Composable
@Suppress("LongMethod")
private fun Notification(
    lead: NotificationLead,
    notificationBannerData: NotificationData,
    modifier: Modifier = Modifier,
) {
    val (title, message, statusLevel, action, messageMaxLines, button) = notificationBannerData
    val surfaces = LocalWarrenSurfaces.current
    val shape = RoundedCornerShape(BANNER_RADIUS)
    Row(
        modifier =
            modifier
                .wrapContentWidth(Alignment.End)
                .widthIn(max = BANNER_MAX_WIDTH)
                .fillMaxWidth()
                .dropShadow(
                    shape,
                    Shadow(
                        radius = BANNER_SHADOW_BLUR,
                        color = Color.Black.copy(alpha = BANNER_SHADOW_ALPHA),
                        spread = BANNER_SHADOW_SPREAD,
                        offset = DpOffset(0.dp, BANNER_SHADOW_OFFSET),
                    ),
                )
                .clip(shape)
                .background(surfaces.homeToolbar)
                .border(BANNER_LINE, surfaces.homeToolbarLine, shape)
                .padding(horizontal = BANNER_PADDING_HORIZONTAL, vertical = BANNER_PADDING_VERTICAL)
                .animateContentSize()
                // A banner appearing is news a screen-reader user must get:
                // without a live region TalkBack never announces the switch to
                // NO INTERNET CONNECTION or to an error. Errors interrupt,
                // everything else waits for a pause (desktop role="status"
                // aria-live="polite", "assertive" on the error tier).
                .semantics {
                    liveRegion =
                        if (statusLevel == StatusLevel.Error) {
                            LiveRegionMode.Assertive
                        } else {
                            LiveRegionMode.Polite
                        }
                }
                .testTag(NOTIFICATION_BANNER_TEST_TAG),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(BANNER_GAP),
    ) {
        Icon(
            painter = painterResource(lead.glyph.drawable()),
            contentDescription = null,
            tint = lead.tone.color(surfaces),
            modifier = Modifier.size(BANNER_GLYPH),
        )
        Column(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(BANNER_TEXT_GAP),
        ) {
            Text(
                // Rendered as authored: a runtime case transform is wrong in
                // several of the locales this app ships.
                text = title,
                style = HomeTypography.bannerTitle,
                color = surfaces.homeText,
                // Two lines: a title that interpolates an app name, and long
                // translations, do not fit one.
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            message?.let { message ->
                Text(
                    text = message.text,
                    style = HomeTypography.bannerBody,
                    color = surfaces.homeTextSecondary,
                    // Unbounded for every banner this app authors; the operator
                    // notice clamps, because its length is the publisher's choice
                    // and the connect card must stay on screen.
                    maxLines = messageMaxLines,
                    overflow = TextOverflow.Ellipsis,
                    modifier =
                        if (message is NotificationMessage.ClickableText) {
                            Modifier.clickable(
                                    onClickLabel = message.contentDescription,
                                    role = Role.Button,
                                    onClick = message.onClick,
                                )
                                .testTag(NOTIFICATION_BANNER_TEXT_ACTION_TEST_TAG)
                        } else {
                            Modifier
                        },
                )
            }
        }
        if (button != null || action != null) {
            Row(
                // An announcement keeps its cross on the headline's line (desktop
                // WarrenAnnouncementCard); every other banner centres its end.
                modifier =
                    Modifier.align(
                        if (lead.glyph == NotificationGlyph.Megaphone) Alignment.Top
                        else Alignment.CenterVertically
                    ),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(BANNER_TRAILING_GAP),
            ) {
                button?.let { BannerButton(it, surfaces) }
                action?.let { BannerAction(it, surfaces.homeTextSecondary) }
            }
        }
    }
}

private fun NotificationGlyph.drawable(): Int =
    when (this) {
        NotificationGlyph.Download -> R.drawable.ic_download
        NotificationGlyph.Megaphone -> R.drawable.ic_megaphone
        NotificationGlyph.Alert -> R.drawable.ic_forum_alert_circle
        NotificationGlyph.Info -> R.drawable.ic_forum_info_circle
    }

private fun NotificationTone.color(surfaces: WarrenSurfaces): Color =
    when (this) {
        NotificationTone.Badge -> surfaces.homeBadge
        NotificationTone.Secondary -> surfaces.homeTextSecondary
        NotificationTone.Disconnect -> surfaces.homeDisconnectText
    }

/** The mockup's light "Install": the home text as its fill, the page as its label. */
@Composable
private fun BannerButton(button: NotificationButton, surfaces: WarrenSurfaces) {
    Text(
        text = button.label,
        style = HomeTypography.bannerButton,
        color = surfaces.homePage,
        maxLines = 1,
        modifier =
            Modifier.clip(RoundedCornerShape(BUTTON_RADIUS))
                .background(surfaces.homeText)
                .clickable(role = Role.Button, onClick = button.onClick)
                .padding(horizontal = BUTTON_PADDING_HORIZONTAL, vertical = BUTTON_PADDING_VERTICAL),
    )
}

/**
 * A bare glyph, as the mockup puts a banner away. The box stays 24 dp to keep the card compact;
 * the pointer input still answers the 48 dp touch floor around it.
 */
@Composable
private fun BannerAction(action: NotificationAction, tint: Color) {
    Box(
        modifier =
            Modifier.size(BANNER_ACTION_BOX)
                .clip(RoundedCornerShape(BANNER_ACTION_RADIUS))
                .clickable(role = Role.Button, onClick = action.onClick)
                .testTag(NOTIFICATION_BANNER_ACTION_TEST_TAG),
        contentAlignment = Alignment.Center,
    ) {
        val glyph = action.glyph
        val iconModifier = Modifier.size(BANNER_GLYPH)
        if (glyph != null) {
            Icon(painterResource(glyph), action.contentDescription, iconModifier, tint)
        } else {
            Icon(action.icon, action.contentDescription, iconModifier, tint)
        }
    }
}
