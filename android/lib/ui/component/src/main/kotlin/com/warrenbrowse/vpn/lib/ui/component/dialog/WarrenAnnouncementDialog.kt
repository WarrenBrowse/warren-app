package com.warrenbrowse.vpn.lib.ui.component.dialog

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.window.DialogProperties
import com.warrenbrowse.vpn.common.compose.createCopyToClipboardHandle
import com.warrenbrowse.vpn.common.compose.createUriHook
import com.warrenbrowse.vpn.lib.model.WarrenAnnouncement
import com.warrenbrowse.vpn.lib.model.WarrenAnnouncementCta
import com.warrenbrowse.vpn.lib.model.WarrenNoticeLevel
import com.warrenbrowse.vpn.lib.ui.component.drawVerticalScrollbar
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenAlertDialog
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha40
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaScrollbar
import kotlinx.coroutines.delay

/**
 * How long the copy confirmation stays up: long enough to be read after the eye
 * has moved back to the code, short enough that it never looks stuck.
 */
private const val COPIED_FEEDBACK_MS = 2000L

/**
 * The launch announcement in full: the operator's headline and body, the
 * voucher code drawn for this account, and the call to action.
 *
 * The banner slot holds one card with a two-line title and a clamped subtitle,
 * which a headline, a body, a 16 character code and a link cannot share. So the
 * banner carries the compact entry and this dialog carries the announcement,
 * the pattern the operator notice already uses for its own overflow. Giving the
 * announcement a permanent surface of its own on the connect screen was the
 * alternative, and it would push the connect card down for every user for as
 * long as the campaign runs, including those who have read it.
 *
 * Every string that comes from the server ([WarrenAnnouncement.headline],
 * [WarrenAnnouncement.body], the call-to-action label) is rendered as plain
 * text. Never `formatWithHtml` and never `HtmlCompat`: the signed channel
 * exists so that what the operator wrote is what the user reads, and markup in
 * a broadcast document is a formatting surface nobody needs.
 */
@Composable
fun WarrenAnnouncementDialog(announcement: WarrenAnnouncement, onDismiss: () -> Unit) {
    val uriHandler = LocalUriHandler.current
    WarrenAlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                text = announcement.headline,
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        },
        text = {
            val scrollState = rememberScrollState()
            Column(
                Modifier.drawVerticalScrollbar(
                        scrollState,
                        MaterialTheme.colorScheme.onPrimary.copy(alpha = AlphaScrollbar),
                    )
                    .verticalScroll(scrollState)
            ) {
                Text(
                    text = announcement.body,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    style = MaterialTheme.typography.labelLarge,
                    modifier = Modifier.fillMaxWidth(),
                )
                announcement.voucherCode?.let { code ->
                    Spacer(Modifier.height(Dimens.verticalSpace))
                    VoucherWell(code)
                }
                announcement.cta?.let { cta ->
                    Spacer(Modifier.height(Dimens.verticalSpace))
                    CallToAction(cta, onClick = uriHandler.createUriHook(cta.url))
                }
            }
        },
        confirmButton = {
            PrimaryButton(
                modifier = Modifier.wrapContentHeight().fillMaxWidth(),
                text = stringResource(R.string.got_it),
                onClick = onDismiss,
            )
        },
        properties = DialogProperties(dismissOnClickOutside = true, dismissOnBackPress = true),
        containerColor = MaterialTheme.colorScheme.surface,
    )
}

/**
 * The code this account was pre-assigned, in a well of its own so it reads as a
 * field to act on rather than as more prose.
 *
 * Selectable, and in a monospace face: a 16 character code has to be
 * transcribable by eye when the clipboard is not where the reader needs it. The
 * clipboard entry is marked sensitive, so the system preview does not put a
 * bearer token worth a month of service on a locked screen.
 */
@Composable
private fun VoucherWell(code: String) {
    var copied by remember(code) { mutableStateOf(false) }
    val copyToClipboard = createCopyToClipboardHandle(isSensitive = true)
    if (copied) {
        LaunchedEffect(code) {
            delay(COPIED_FEEDBACK_MS)
            copied = false
        }
    }
    Column(
        modifier =
            Modifier.fillMaxWidth()
                .background(
                    color = Color.Black.copy(alpha = Alpha40),
                    shape = RoundedCornerShape(Dimens.dialogCornerRadius),
                )
                .border(
                    width = Dimens.listItemDivider,
                    color = Color.White.copy(alpha = Alpha20),
                    shape = RoundedCornerShape(Dimens.dialogCornerRadius),
                )
                .padding(Dimens.smallPadding),
        verticalArrangement = Arrangement.spacedBy(Dimens.tinyPadding),
    ) {
        Text(
            text =
                if (copied) {
                    stringResource(R.string.announcement_voucher_copied)
                } else {
                    stringResource(R.string.announcement_voucher_label)
                },
            style =
                MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
            color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
            SelectionContainer(modifier = Modifier.weight(1f)) {
                Text(
                    text = code,
                    style =
                        MaterialTheme.typography.labelLarge.copy(fontFamily = FontFamily.Monospace),
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
            IconButton(
                onClick = {
                    copyToClipboard(code, null)
                    copied = true
                }
            ) {
                Icon(
                    imageVector = if (copied) Icons.Rounded.Check else Icons.Rounded.ContentCopy,
                    // The confirmation reaches a screen reader too: the label
                    // is the only thing that changes for it when the icon does.
                    contentDescription =
                        if (copied) {
                            stringResource(R.string.announcement_voucher_copied)
                        } else {
                            stringResource(R.string.announcement_copy_code)
                        },
                    tint = MaterialTheme.colorScheme.onSurface,
                )
            }
        }
    }
}

/**
 * The call to action, opened through the app's own external-link path. Rust has
 * already refused a URL that is not safe to render as a link, so what arrives
 * here is `https` with a plain host; the label is the operator's, rendered
 * verbatim.
 */
@Composable
private fun CallToAction(cta: WarrenAnnouncementCta, onClick: () -> Unit) {
    PrimaryButton(
        modifier = Modifier.wrapContentHeight().fillMaxWidth(),
        text = cta.label,
        onClick = onClick,
    )
}

@Preview
@Composable
private fun PreviewWarrenAnnouncementDialog() {
    AppTheme {
        WarrenAnnouncementDialog(
            announcement =
                WarrenAnnouncement(
                    id = "a1",
                    headline = "Production is open",
                    body =
                        "Warren is out of beta. Your account gets a free month on the " +
                            "production service, and the code below redeems it.",
                    level = WarrenNoticeLevel.WARNING,
                    cta = WarrenAnnouncementCta("Get Warren", "https://warren.ro/download"),
                    voucherCampaignId = "prod-launch",
                    voucherCode = "ABCD1234EFGH5678",
                ),
            onDismiss = {},
        )
    }
}
