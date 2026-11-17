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
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.shape.CircleShape
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
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens

private const val NOTIFICATION_MOTION_MILLIS = 250

private val notice = DesignTokens.Home.Notice

// The notice's drop: 0 8 24 -10 of the notice shadow (mockup).
private val BANNER_SHADOW_BLUR = 24.dp
private val BANNER_SHADOW_SPREAD = (-10).dp
private val BANNER_SHADOW_OFFSET = 8.dp
private val BANNER_LINE = 1.dp
private val BANNER_TRAILING_GAP = 4.dp

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
    onClickShowAlwaysOnVpnGuide: () -> Unit,
    onClickDismissAlwaysOnVpnAdvice: () -> Unit,
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
                    onClickShowAlwaysOnVpnGuide = onClickShowAlwaysOnVpnGuide,
                    onClickDismissAlwaysOnVpnAdvice = onClickDismissAlwaysOnVpnAdvice,
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
    val shape = RoundedCornerShape(notice.Radius)
    // A notice that ends on a button hugs it closer (desktop `:has([data-banner-button])`).
    val end = if (button != null) notice.ButtonInset else notice.PaddingEnd
    val vertical = if (button != null) notice.ButtonInset else notice.PaddingVertical
    Row(
        modifier =
            modifier
                .wrapContentWidth(Alignment.End)
                .widthIn(max = notice.MaxWidth)
                .fillMaxWidth()
                .dropShadow(
                    shape,
                    Shadow(
                        radius = BANNER_SHADOW_BLUR,
                        color = surfaces.homeNoticeShadow,
                        spread = BANNER_SHADOW_SPREAD,
                        offset = DpOffset(0.dp, BANNER_SHADOW_OFFSET),
                    ),
                )
                .clip(shape)
                // The desktop glass also blurs the art behind it; a backdrop blur is not
                // available to a Compose surface, so the notice's own translucency carries it.
                .background(surfaces.homeNotice)
                .border(BANNER_LINE, surfaces.homeNoticeLine, shape)
                .padding(start = notice.PaddingStart, end = end, top = vertical, bottom = vertical)
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
        horizontalArrangement = Arrangement.spacedBy(notice.Gap),
    ) {
        Icon(
            painter = painterResource(lead.glyph.drawable()),
            contentDescription = null,
            tint = lead.tone.color(surfaces),
            modifier = Modifier.size(notice.Icon),
        )
        Column(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(notice.BodyGap),
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
                    color = surfaces.homeNoticeTextSecondary,
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
                action?.let { BannerAction(it, surfaces.homeNoticeTextSecondary) }
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
        NotificationTone.Update -> surfaces.homeNoticeUpdate
        NotificationTone.Announcement -> surfaces.homeNoticeAnnouncement
        NotificationTone.Badge -> surfaces.homeBadge
        NotificationTone.Secondary -> surfaces.homeNoticeTextSecondary
        NotificationTone.Disconnect -> surfaces.homeDisconnectText
    }

/** The mockup's light "Install": the home text as its fill, the page as its label. */
@Composable
private fun BannerButton(button: NotificationButton, surfaces: WarrenSurfaces) {
    Box(
        modifier =
            Modifier.height(notice.ButtonHeight)
                .clip(RoundedCornerShape(notice.ButtonRadius))
                .background(surfaces.homeText)
                .clickable(role = Role.Button, onClick = button.onClick)
                .padding(horizontal = notice.ButtonPaddingHorizontal),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = button.label,
            style = HomeTypography.bannerButton,
            color = surfaces.homePage,
            maxLines = 1,
        )
    }
}

/**
 * A bare glyph, as the mockup puts a banner away: the cross at its 18 size, which the pointer
 * input still answers on the 48 dp touch floor around it.
 */
@Composable
private fun BannerAction(action: NotificationAction, tint: Color) {
    Box(
        modifier =
            Modifier.size(notice.Close)
                .clip(CircleShape)
                .clickable(role = Role.Button, onClick = action.onClick)
                .testTag(NOTIFICATION_BANNER_ACTION_TEST_TAG),
        contentAlignment = Alignment.Center,
    ) {
        val glyph = action.glyph
        val iconModifier = Modifier.size(notice.Close)
        if (glyph != null) {
            Icon(painterResource(glyph), action.contentDescription, iconModifier, tint)
        } else {
            Icon(action.icon, action.contentDescription, iconModifier, tint)
        }
    }
}
