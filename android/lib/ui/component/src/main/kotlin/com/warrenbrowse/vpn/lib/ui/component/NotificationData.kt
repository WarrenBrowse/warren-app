package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.OpenInNew
import androidx.compose.material.icons.rounded.AccountBalanceWallet
import androidx.compose.material.icons.rounded.Clear
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.UnfoldMore
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenTextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.LinkInteractionListener
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.platform.LocalUriHandler
import androidx.core.text.HtmlCompat
import java.net.InetAddress
import com.warrenbrowse.vpn.common.compose.createUriHook
import com.warrenbrowse.vpn.lib.model.AuthFailedError
import com.warrenbrowse.vpn.lib.model.ErrorState
import com.warrenbrowse.vpn.lib.model.ErrorStateCause
import com.warrenbrowse.vpn.lib.model.InAppNotification
import com.warrenbrowse.vpn.lib.model.ParameterGenerationError
import com.warrenbrowse.vpn.lib.model.StatusLevel
import com.warrenbrowse.vpn.lib.model.WarrenNotice
import com.warrenbrowse.vpn.lib.model.WarrenNoticeLevel
import com.warrenbrowse.vpn.lib.ui.component.NotificationMessage.ClickableText
import com.warrenbrowse.vpn.lib.ui.component.dialog.InfoDialog

data class NotificationData(
    val title: AnnotatedString,
    val message: NotificationMessage? = null,
    val statusLevel: StatusLevel,
    val action: NotificationAction? = null,
    /**
     * Lines the message may take before it is ellipsised. Unbounded by
     * default: every banner this app authors is written to fit. Only the
     * operator notice sets it, because its text is not ours and the
     * publication cap allows far more than a banner can hold.
     */
    val messageMaxLines: Int = Int.MAX_VALUE,
) {
    constructor(
        title: String,
        message: String? = null,
        statusLevel: StatusLevel,
        action: NotificationAction? = null,
    ) : this(
        AnnotatedString(title),
        message?.let { NotificationMessage.Text(AnnotatedString(it)) },
        statusLevel,
        action,
    )

    constructor(
        title: String,
        message: NotificationMessage,
        statusLevel: StatusLevel,
        action: NotificationAction? = null,
    ) : this(AnnotatedString(title), message, statusLevel, action)
}

sealed interface NotificationMessage {
    val text: AnnotatedString

    data class Text(override val text: AnnotatedString) : NotificationMessage

    data class ClickableText(
        override val text: AnnotatedString,
        val onClick: () -> Unit,
        val contentDescription: String,
    ) : NotificationMessage
}

data class NotificationAction(
    val icon: ImageVector,
    val onClick: (() -> Unit),
    val contentDescription: String,
)

@Suppress("LongMethod", "CyclomaticComplexMethod")
@Composable
fun InAppNotification.toNotificationData(
    isPlayBuild: Boolean,
    openAppListing: () -> Unit,
    onClickShowAccount: () -> Unit,
    onClickShowChangelog: () -> Unit,
    onClickShowAndroid16UpgradeInfo: () -> Unit,
    onClickDismissChangelog: () -> Unit,
    onClickDismissAndroid16UpgradeWarning: () -> Unit,
    onClickDismissUpdateAvailable: () -> Unit,
    onClickDismissExitSwitched: () -> Unit,
    onClickDismissNotice: () -> Unit,
    onClickReEnableAfterStandDown: () -> Unit,
) =
    when (this) {
        InAppNotification.EnvStandDown ->
            NotificationData(
                title = stringResource(id = R.string.env_stand_down_title),
                // Two plain resources, rendered verbatim: no operator-authored
                // text reaches this banner, so nothing here goes through
                // HtmlCompat. The way back is the link rather than the single
                // action slot, because an icon alone would carry no reason.
                message =
                    ClickableText(
                        text =
                            buildAnnotatedString {
                                append(stringResource(id = R.string.env_stand_down_message))
                                append(SPACE_CHAR)
                                withStyle(
                                    SpanStyle(
                                        textDecoration = TextDecoration.Underline,
                                        color = MaterialTheme.colorScheme.onSurface,
                                    )
                                ) {
                                    append(stringResource(id = R.string.env_stand_down_re_enable))
                                }
                                append(DOT_CHAR)
                            },
                        onClick = onClickReEnableAfterStandDown,
                        contentDescription =
                            stringResource(id = R.string.env_stand_down_re_enable),
                    ),
                statusLevel = statusLevel,
            )
        is InAppNotification.OperatorNotice ->
            operatorNoticeBannerData(statusLevel, notice, onClickDismissNotice)
        InAppNotification.HostOffline ->
            NotificationData(
                title = stringResource(id = R.string.no_internet_connection),
                message = stringResource(id = R.string.host_offline_notification_message),
                statusLevel = statusLevel,
            )
        InAppNotification.ExitEgressDead ->
            NotificationData(
                title = stringResource(id = R.string.exit_egress_dead_title),
                message = stringResource(id = R.string.exit_egress_dead_message),
                statusLevel = statusLevel,
            )
        InAppNotification.ConnectingStuck ->
            NotificationData(
                title = stringResource(id = R.string.connecting_stuck_title),
                message = stringResource(id = R.string.connecting_stuck_message),
                statusLevel = statusLevel,
                action = forumAction(),
            )
        InAppNotification.ExitSwitched ->
            NotificationData(
                title = stringResource(id = R.string.exit_switched_title),
                message = stringResource(id = R.string.exit_switched_message),
                statusLevel = statusLevel,
                action =
                    NotificationAction(
                        Icons.Rounded.Clear,
                        onClickDismissExitSwitched,
                        stringResource(id = R.string.dismiss),
                    ),
            )
        InAppNotification.TunnelStateBlocked ->
            NotificationData(
                title = stringResource(id = R.string.banner_blocking_internet),
                statusLevel = StatusLevel.None,
            )
        is InAppNotification.CloseToExpiry ->
            NotificationData(
                title =
                    if (daysLeft <= 0L) {
                        stringResource(id = R.string.subscription_expired_title)
                    } else {
                        stringResource(id = R.string.subscription_expires_soon_title)
                    },
                message =
                    when {
                        daysLeft <= 0L -> stringResource(id = R.string.connect_subscription_expired)
                        daysLeft == 1L ->
                            stringResource(
                                id = R.string.connect_subscription_expires_in_day,
                                daysLeft,
                            )
                        else ->
                            stringResource(
                                id = R.string.connect_subscription_expires_in_days,
                                daysLeft,
                            )
                    },
                statusLevel = statusLevel,
                action =
                    NotificationAction(
                        Icons.Rounded.AccountBalanceWallet,
                        onClickShowAccount,
                        stringResource(id = R.string.settings_account),
                    ),
            )
        is InAppNotification.TunnelStateError ->
            errorMessageBannerData(statusLevel, error)
        is InAppNotification.UnsupportedVersion ->
            NotificationData(
                title = stringResource(id = R.string.unsupported_version),
                message = stringResource(id = R.string.unsupported_version_description),
                statusLevel = statusLevel,
                action =
                    NotificationAction(
                        Icons.AutoMirrored.Rounded.OpenInNew,
                        openAppListing,
                        stringResource(id = R.string.open_url),
                    ),
            )
        is InAppNotification.UpdateAvailable ->
            NotificationData(
                title = stringResource(id = R.string.update_available),
                // The single action slot holds the dismiss, so the update
                // itself moves onto the message the way the changelog banner
                // already does: a banner the user cannot put away is a banner
                // that also hides whatever ranks below it.
                message =
                    ClickableText(
                        text =
                            buildAnnotatedString {
                                withStyle(SpanStyle(textDecoration = TextDecoration.Underline)) {
                                    append(
                                        stringResource(
                                            id = R.string.update_available_description,
                                            version,
                                        )
                                    )
                                }
                            },
                        onClick = openAppListing,
                        contentDescription = stringResource(id = R.string.open_url),
                    ),
                statusLevel = statusLevel,
                action =
                    NotificationAction(
                        Icons.Rounded.Clear,
                        onClickDismissUpdateAvailable,
                        stringResource(id = R.string.dismiss),
                    ),
            )
        is InAppNotification.NewVersionChangelog ->
            NotificationData(
                title = stringResource(id = R.string.new_changelog_notification_title),
                message =
                    ClickableText(
                        text =
                            buildAnnotatedString {
                                withStyle(SpanStyle(textDecoration = TextDecoration.Underline)) {
                                    append(
                                        stringResource(
                                            id = R.string.new_changelog_notification_message
                                        )
                                    )
                                }
                            },
                        onClick = onClickShowChangelog,
                        contentDescription =
                            stringResource(id = R.string.new_changelog_notification_message),
                    ),
                statusLevel = statusLevel,
                action =
                    NotificationAction(
                        Icons.Rounded.Clear,
                        onClickDismissChangelog,
                        stringResource(id = R.string.dismiss),
                    ),
            )

        InAppNotification.Android16UpgradeWarning ->
            NotificationData(
                title = stringResource(id = R.string.banner_android_16_upgrade_warning_title),
                message =
                    ClickableText(
                        text =
                            buildAnnotatedString {
                                append(
                                    stringResource(id = R.string.android_16_upgrade_warning_message)
                                )
                                append(SPACE_CHAR)
                                withStyle(
                                    SpanStyle(
                                        textDecoration = TextDecoration.Underline,
                                        color = MaterialTheme.colorScheme.onSurface,
                                    )
                                ) {
                                    append(stringResource(R.string.click_here))
                                }
                                append(DOT_CHAR)
                            },
                        onClick = onClickShowAndroid16UpgradeInfo,
                        contentDescription =
                            stringResource(id = R.string.new_changelog_notification_message),
                    ),
                statusLevel = statusLevel,
                action =
                    NotificationAction(
                        Icons.Rounded.Clear,
                        onClickDismissAndroid16UpgradeWarning,
                        stringResource(id = R.string.dismiss),
                    ),
            )
    }

/**
 * The operator's own words, shown as authored. Only the severity label is
 * translated: the message is rendered as plain text, never through HtmlCompat
 * and never through a resource, because the signed channel exists precisely so
 * that what the operator wrote is what the user reads.
 *
 * The banner clamps it and the action opens the full text in a dialog (the
 * desktop's expand-text action): the publication cap is 500 characters, which
 * no banner can hold without pushing the connect card off the screen.
 */
@Composable
private fun operatorNoticeBannerData(
    statusLevel: StatusLevel,
    notice: WarrenNotice,
    onDismiss: () -> Unit,
): NotificationData {
    val title =
        when (notice.level) {
            WarrenNoticeLevel.ERROR -> stringResource(R.string.notice_title_important)
            WarrenNoticeLevel.WARNING -> stringResource(R.string.notice_title_notice)
            WarrenNoticeLevel.INFO -> stringResource(R.string.notice_title_warren)
        }
    // Keyed on the id so a second notice replacing the first opens closed,
    // rather than showing its text under the previous one's dialog.
    var expanded by remember(notice.id) { mutableStateOf(false) }
    if (expanded) {
        InfoDialog(title = title, message = notice.message, onDismiss = { expanded = false })
    }
    val expand = { expanded = true }
    val readInFull = stringResource(R.string.notice_read_in_full)
    // An informational notice spends its single action slot on the dismiss, and
    // the clamped text itself opens the rest (the stand-down banner's pattern).
    // The slot cannot hold both, and of the two only the dismiss frees the
    // banner for the cards ranked under the notice.
    val dismissible = notice.level == WarrenNoticeLevel.INFO
    return NotificationData(
        // The primary constructor, because the convenience ones do not carry
        // the clamp; the title is a plain resource all the same.
        title = AnnotatedString(title),
        message =
            if (dismissible) {
                ClickableText(
                    text = AnnotatedString(notice.message),
                    onClick = expand,
                    contentDescription = readInFull,
                )
            } else {
                NotificationMessage.Text(AnnotatedString(notice.message))
            },
        statusLevel = statusLevel,
        action =
            if (dismissible) {
                NotificationAction(
                    Icons.Rounded.Clear,
                    onClick = onDismiss,
                    contentDescription = stringResource(R.string.dismiss),
                )
            } else {
                NotificationAction(
                    Icons.Rounded.UnfoldMore,
                    onClick = expand,
                    contentDescription = readInFull,
                )
            },
        messageMaxLines = NOTICE_BANNER_MAX_LINES,
    )
}

/**
 * Lines the banner gives an operator notice before the expand action carries
 * the rest. Three: enough for a sentence the operator can rely on being read
 * at a glance, short enough to leave the connect card on screen.
 */
private const val NOTICE_BANNER_MAX_LINES = 3

@Composable
private fun errorMessageBannerData(statusLevel: StatusLevel, error: ErrorState) =
    NotificationData(
        title = error.title().formatWithHtml(),
        message = NotificationMessage.Text(error.message()),
        statusLevel = statusLevel,
        action = error.errorLinkAction(),
    )

/** Opens the community forum, the only support door the app still has. */
@Composable
private fun forumAction(): NotificationAction {
    val forumUrl = stringResource(R.string.community_forum_url)
    val openForum = LocalUriHandler.current.createUriHook(forumUrl)
    return NotificationAction(
        icon = Icons.AutoMirrored.Rounded.OpenInNew,
        onClick = openForum,
        contentDescription = stringResource(R.string.open_url),
    )
}

// The in-app problem-report flow was removed; the community forum is the only
// support door left. Every error message that tells the user to report the
// problem (see failed_to_block_internet, auth_failed, set_firewall_policy_error,
// set_dns_error, start_tunnel_error) must give them a way to do it, mirroring
// desktop's error.tsx getActions(). Kept in sync with ErrorState.message():
// when not blocking successfully (the leak state), the generic
// failed_to_block_internet copy is shown regardless of cause, so the action
// must follow the same fallback.
//
// A cause with known remedies opens the troubleshoot dialog first (desktop
// troubleshoot-dialog action): sending a user to the forum before telling them
// what to try is a support ticket for a problem they could have fixed.
//
// A forwarded-port suspension is the exception: the message names an appeal
// page, so the action opens that page rather than the generic forum.
@Composable
private fun ErrorState.errorLinkAction(): NotificationAction? {
    val uriHandler = LocalUriHandler.current
    val reportsUrl = stringResource(R.string.reports_url)
    val forumUrl = stringResource(R.string.community_forum_url)
    val openReports = uriHandler.createUriHook(reportsUrl)
    val openForum = uriHandler.createUriHook(forumUrl)

    var troubleshootShown by remember { mutableStateOf(false) }
    val steps = troubleshootSteps()
    if (steps != null) {
        if (troubleshootShown) {
            InfoDialog(
                title = stringResource(R.string.troubleshoot),
                message = steps,
                onDismiss = { troubleshootShown = false },
                dismissButton = {
                    WarrenTextButton(
                        onClick = {
                            troubleshootShown = false
                            openForum()
                        }
                    ) {
                        Text(text = stringResource(R.string.troubleshoot_report_forum))
                    }
                },
            )
        }
        return NotificationAction(
            icon = Icons.Rounded.Info,
            onClick = { troubleshootShown = true },
            contentDescription = stringResource(R.string.troubleshoot),
        )
    }

    val onClick = when {
        isPortForwardingBan() -> openReports
        isReportWorthy() -> openForum
        else -> return null
    }
    return NotificationAction(
        icon = Icons.AutoMirrored.Rounded.OpenInNew,
        onClick = onClick,
        contentDescription = stringResource(R.string.open_url),
    )
}

/**
 * The self-service steps for the causes an Android user can actually act on.
 * Null means there is nothing to try, so the banner keeps the direct forum link.
 * The leak state is excluded on purpose: its copy is the generic
 * "unable to block all traffic" line, whose only remedy is a report.
 */
@Composable
private fun ErrorState.troubleshootSteps(): String? {
    val cause = this.cause
    return when {
        !isBlocking -> null
        cause is ErrorStateCause.DnsError -> stringResource(R.string.troubleshoot_dns_error)
        cause is ErrorStateCause.StartTunnelError ->
            stringResource(R.string.troubleshoot_start_tunnel_error)
        cause is ErrorStateCause.NotPrepared ||
            cause is ErrorStateCause.OtherAlwaysOnApp ||
            cause is ErrorStateCause.OtherLegacyAlwaysOnApp ->
            stringResource(R.string.troubleshoot_vpn_permission)
        else -> null
    }
}

private fun ErrorState.isPortForwardingBan(): Boolean {
    val cause = this.cause
    return isBlocking &&
        cause is ErrorStateCause.AuthFailed &&
        cause.error == AuthFailedError.BannedPortForwarding
}

private fun ErrorState.isReportWorthy(): Boolean {
    val cause = this.cause
    return when {
        !isBlocking -> true
        cause is ErrorStateCause.AuthFailed -> cause.error is AuthFailedError.Unknown
        cause is ErrorStateCause.FirewallPolicyError -> true
        cause is ErrorStateCause.DnsError -> true
        cause is ErrorStateCause.StartTunnelError -> true
        else -> false
    }
}

@Composable
private fun String.formatWithHtml(): AnnotatedString =
    HtmlCompat.fromHtml(this, HtmlCompat.FROM_HTML_MODE_COMPACT)
        .toAnnotatedString(
            boldSpanStyle =
                SpanStyle(
                    color = MaterialTheme.colorScheme.onSurface,
                    fontWeight = FontWeight.ExtraBold,
                )
        )

// The banner titles are the uppercase-authored banner_* resources, not the
// sentence-case ones the system notification reuses: the banner applies no case
// transform, so each surface needs its own authoring.
@Composable
private fun ErrorState.title(): String {
    val cause = this.cause
    return when {
        cause is ErrorStateCause.InvalidDnsServers ->
            stringResource(R.string.banner_blocking_internet)
        cause is ErrorStateCause.NotPrepared ->
            stringResource(R.string.banner_vpn_permission_error)
        cause is ErrorStateCause.OtherAlwaysOnApp ->
            stringResource(R.string.banner_always_on_vpn_error, cause.appName)
        cause is ErrorStateCause.OtherLegacyAlwaysOnApp ->
            stringResource(R.string.banner_legacy_always_on_vpn_error)
        cause is ErrorStateCause.WarrenTunnelFlapping ->
            stringResource(R.string.warren_tunnel_flapping_title)
        isBlocking -> stringResource(R.string.banner_blocking_internet)
        else -> stringResource(R.string.banner_critical_error)
    }
}

@Composable
private fun ErrorState.message(): AnnotatedString {
    val cause = this.cause
    return when {
        isBlocking -> cause.errorMessageId().formatWithHtml()
        else -> stringResource(R.string.failed_to_block_internet).formatWithHtml()
    }
}

@Composable
private fun ErrorStateCause.errorMessageId(): String =
    when (this) {
        is ErrorStateCause.AuthFailed -> error.authFailedMessage()
        is ErrorStateCause.Ipv6Unavailable -> stringResource(R.string.ipv6_unavailable)
        is ErrorStateCause.FirewallPolicyError -> stringResource(R.string.set_firewall_policy_error)
        is ErrorStateCause.DnsError -> stringResource(R.string.set_dns_error)
        is ErrorStateCause.StartTunnelError -> stringResource(R.string.start_tunnel_error)
        is ErrorStateCause.WarrenTunnelFlapping ->
            stringResource(R.string.warren_tunnel_flapping)
        is ErrorStateCause.WarrenKillSwitchActive ->
            stringResource(R.string.warren_kill_switch_active)
        is ErrorStateCause.IsOffline -> stringResource(R.string.is_offline)
        is ErrorStateCause.TunnelParameterError -> stringResource(error.errorMessageId())
        is ErrorStateCause.NotPrepared ->
            stringResource(R.string.vpn_permission_error_notification_message)
        is ErrorStateCause.OtherAlwaysOnApp ->
            stringResource(R.string.always_on_vpn_error_notification_content, appName)
        is ErrorStateCause.OtherLegacyAlwaysOnApp ->
            stringResource(R.string.legacy_always_on_vpn_error_notification_content)
        is ErrorStateCause.InvalidDnsServers ->
            stringResource(
                R.string.invalid_dns_servers,
                addresses.joinToString { address -> address.addressString() },
            )
        is ErrorStateCause.NoRelaysMatchSelectedPort ->
            stringResource(R.string.no_matching_relay)
        is ErrorStateCause.InvalidIpv6Config -> stringResource(R.string.invalid_ipv6_config)
    }

/**
 * The suspension copy for a forwarded-port ban names the appeal page, so the
 * URL is interpolated: telling the user to contact support with no channel is
 * the dead end this replaces. Every other auth failure is a plain resource.
 */
@Composable
private fun AuthFailedError.authFailedMessage(): String =
    if (this == AuthFailedError.BannedPortForwarding) {
        stringResource(
            R.string.auth_failed_banned_port_forwarding,
            stringResource(R.string.reports_url),
        )
    } else {
        stringResource(errorMessageId())
    }

private fun AuthFailedError.errorMessageId(): Int =
    when (this) {
        AuthFailedError.ExpiredAccount -> R.string.account_credit_has_expired
        AuthFailedError.InvalidAccount -> R.string.auth_failed_invalid_account
        AuthFailedError.TooManyConnections -> R.string.auth_failed_too_many_connections
        // A ban is a suspension, not a renewable expiry: distinct copy so the
        // user contacts support rather than trying to top up. The
        // port-forwarding ban names the forwarded-port cause specifically.
        AuthFailedError.Banned -> R.string.auth_failed_banned
        AuthFailedError.BannedPortForwarding -> R.string.auth_failed_banned_port_forwarding
        // Only the truly unknown cause is generic enough to ask the user to
        // report it; the other causes above have a specific, actionable copy.
        AuthFailedError.Unknown -> R.string.auth_failed
    }

private fun ParameterGenerationError.errorMessageId(): Int =
    when (this) {
        ParameterGenerationError.NoMatchingRelay,
        ParameterGenerationError.NoMatchingBridgeRelay -> {
            R.string.no_matching_relay
        }
        ParameterGenerationError.NoMatchingRelayExit -> {
            R.string.no_matching_relay_exit
        }
        ParameterGenerationError.NoMatchingRelayEntry -> {
            R.string.no_matching_relay_entry
        }
        ParameterGenerationError.CustomTunnelHostResolutionError ->
            R.string.custom_tunnel_host_resolution_error
        ParameterGenerationError.Ipv4_Unavailable -> R.string.ip_version_v4_unavailable
        ParameterGenerationError.Ipv6_Unavailable -> R.string.ip_version_v6_unavailable
    }

private fun InetAddress.addressString(): String {
    val hostNameAndAddress = this.toString().split('/', limit = 2)
    val address = hostNameAndAddress[1]

    return address
}

