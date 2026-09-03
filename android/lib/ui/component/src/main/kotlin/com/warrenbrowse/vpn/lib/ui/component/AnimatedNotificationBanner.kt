package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.constraintlayout.compose.ConstraintLayout
import androidx.constraintlayout.compose.Dimension
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.StatusLevel
import com.warrenbrowse.vpn.lib.ui.tag.NOTIFICATION_BANNER_ACTION_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.NOTIFICATION_BANNER_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.NOTIFICATION_BANNER_TEXT_ACTION_TEST_TAG
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.draw.drawBehind
import androidx.compose.animation.core.tween
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.warning

private const val NOTIFICATION_CARD_ALPHA = 0.6f
private const val NOTIFICATION_MOTION_MILLIS = 250

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
                visibleNotification.toNotificationData(
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
                ),
            )
    }
}

@Composable
@Suppress("LongMethod")
private fun Notification(modifier: Modifier = Modifier, notificationBannerData: NotificationData) {
    val (title, message, statusLevel, action, messageMaxLines) = notificationBannerData
    // Floating dark rounded card over the backdrop (desktop NotificationBanner),
    // instead of an edge-to-edge opaque bar.
    val shape = RoundedCornerShape(Dimens.notificationBannerRadius)
    val edgeColor = statusLevel.accent()
    val edge = Dimens.notificationBannerEdge
    ConstraintLayout(
        modifier =
            modifier
                .padding(horizontal = Dimens.mediumPadding, vertical = Dimens.tinyPadding)
                .shadow(Dimens.notificationBannerElevation, shape, clip = false)
                .clip(shape)
                .background(color = Color.Black.copy(alpha = NOTIFICATION_CARD_ALPHA))
                .border(width = 1.dp, color = Color.White.copy(alpha = 0.2f), shape = shape)
                // The status colour as a top edge, the desktop's accent line.
                .drawBehind {
                    drawRect(
                        color = edgeColor,
                        size = Size(size.width, edge.toPx()),
                    )
                }
                .padding(
                    start = Dimens.notificationBannerStartPadding,
                    end = Dimens.notificationBannerEndPadding,
                    top = Dimens.notificationBannerVerticalPadding,
                    bottom = Dimens.notificationBannerVerticalPadding,
                )
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
                .testTag(NOTIFICATION_BANNER_TEST_TAG)
    ) {
        val (status, textTitle, textMessage, actionIcon) = createRefs()
        NotificationDot(
            statusLevel,
            Modifier.constrainAs(status) {
                top.linkTo(textTitle.top)
                start.linkTo(parent.start)
                bottom.linkTo(textTitle.bottom)
            },
        )
        Text(
            // Rendered as authored: the title resources carry the desktop's
            // uppercase, because a runtime toUpperCase is wrong in several of
            // the locales this app ships and leaves translators guessing.
            text = title,
            modifier =
                Modifier.constrainAs(textTitle) {
                        top.linkTo(parent.top)
                        start.linkTo(status.end)
                        if (message != null) {
                            bottom.linkTo(textMessage.top)
                        } else {
                            bottom.linkTo(parent.bottom)
                        }
                        if (action != null) {
                            end.linkTo(actionIcon.start)
                        } else {
                            end.linkTo(parent.end)
                        }
                        width = Dimension.fillToConstraints
                    }
                    .padding(start = Dimens.smallPadding),
            // Desktop tinyText: 12/18 semibold.
            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
            color = MaterialTheme.colorScheme.onSurface,
            // Two lines: a title that interpolates an app name, and long
            // translations of the version banners, do not fit one. The card
            // animates its own size, so it grows cleanly.
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        message?.let { message ->
            Text(
                text = message.text,
                modifier =
                    Modifier.constrainAs(textMessage) {
                            top.linkTo(textTitle.bottom)
                            start.linkTo(textTitle.start)
                            if (action != null) {
                                end.linkTo(actionIcon.start)
                                bottom.linkTo(parent.bottom)
                            } else {
                                end.linkTo(parent.end)
                                bottom.linkTo(parent.bottom)
                            }
                            width = Dimension.fillToConstraints
                            height = Dimension.wrapContent
                        }
                        .padding(start = Dimens.smallPadding, top = Dimens.tinyPadding)
                        .wrapContentWidth(Alignment.Start)
                        .let {
                            if (message is NotificationMessage.ClickableText) {
                                it.clickable(
                                        onClickLabel = message.contentDescription,
                                        role = Role.Button,
                                    ) {
                                        message.onClick()
                                    }
                                    .testTag(NOTIFICATION_BANNER_TEXT_ACTION_TEST_TAG)
                            } else {
                                it
                            }
                        },
                // Desktop NotificationSubtitleText: 12/600 at 60 % white. The
                // weight is explicit because the body face ships 400/600/700
                // only, so labelMedium's 500 would resolve to regular.
                color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
                style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                // Unbounded for every banner this app authors; the operator
                // notice clamps, because its length is the publisher's choice
                // and the connect card must stay on screen.
                maxLines = messageMaxLines,
                overflow = TextOverflow.Ellipsis,
            )
        }
        action?.let {
            NotificationAction(
                it.icon,
                onClick = it.onClick,
                contentDescription = it.contentDescription,
                modifier =
                    Modifier.constrainAs(actionIcon) {
                        top.linkTo(parent.top)
                        end.linkTo(parent.end)
                        bottom.linkTo(parent.bottom)
                    },
            )
        }
    }
}

@Composable
private fun StatusLevel.accent(): Color =
    when (this) {
        StatusLevel.Error -> MaterialTheme.colorScheme.error
        StatusLevel.Warning -> MaterialTheme.colorScheme.warning
        StatusLevel.Info -> MaterialTheme.colorScheme.positive
        StatusLevel.None -> Color.Transparent
    }

@Composable
private fun NotificationDot(statusLevel: StatusLevel, modifier: Modifier) {
    Box(
        modifier =
            modifier
                .background(color = statusLevel.accent(), shape = CircleShape)
                .size(Dimens.notificationStatusIconSize)
    )
}

@Composable
private fun NotificationAction(
    imageVector: ImageVector,
    contentDescription: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {

    IconButton(
        modifier = modifier.testTag(NOTIFICATION_BANNER_ACTION_TEST_TAG),
        onClick = onClick,
    ) {
        Icon(
            modifier = Modifier.padding(Dimens.smallPadding),
            imageVector = imageVector,
            contentDescription = contentDescription,
            tint = MaterialTheme.colorScheme.onSurface,
        )
    }
}
