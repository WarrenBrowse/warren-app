package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Error
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.res.stringResource
import com.warrenbrowse.vpn.lib.ui.component.ExpandChevron
import com.warrenbrowse.vpn.lib.ui.designsystem.NegativeButton
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenAlertDialog
import com.warrenbrowse.vpn.lib.ui.designsystem.PrimaryButton
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.onSelected
import com.warrenbrowse.vpn.lib.ui.theme.color.positive
import com.warrenbrowse.vpn.lib.ui.theme.color.primaryDisabled

/**
 * Trust-on-first-use exit-key mismatch warning, aligned with the desktop
 * `WarrenPubKeyWarning`: a title, two explanatory paragraphs, the cryptographic
 * detail rows (exit id, previously pinned key, newly observed key, location) and
 * two actions (trust the new key, or reject and stay disconnected).
 *
 * The desktop's third action, "Report to Warren" (a signed POST to
 * `/v1/incidents/pubkey-mismatch`), is intentionally absent: the Android JNI
 * surface exposes no incident-report endpoint, so wiring it would need new engine
 * plumbing rather than a UI change.
 */
@Suppress("LongMethod")
@Composable
fun WarrenPubKeyWarningDialog(
    exitIdHex: String,
    pinnedPubkeyHex: String,
    observedPubkeyHex: String,
    locationText: String?,
    onTrust: () -> Unit,
    onReject: () -> Unit,
    // The decision runs a pin write plus a re-dial. While that is in flight the
    // choice is locked, and a failure keeps the dialog up with a reason rather
    // than dropping the user on a disconnected screen with no explanation.
    busy: Boolean = false,
    errorText: String? = null,
) {
    var detailsShown by rememberSaveable { mutableStateOf(false) }
    WarrenAlertDialog(
        // A dismissal is a rejection, which must not fire mid-decision.
        onDismissRequest = { if (!busy) onReject() },
        icon = {
            Icon(
                modifier = Modifier.fillMaxWidth().height(Dimens.dialogIconHeight),
                imageVector = Icons.Rounded.Error,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.error,
            )
        },
        title = {
            Text(
                text = stringResource(R.string.warren_pubkey_warning_title),
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface,
            )
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(Dimens.smallPadding)) {
                Text(
                    text = stringResource(R.string.warren_pubkey_warning_body1),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Text(
                    text = stringResource(R.string.warren_pubkey_warning_body2),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                // The hex rows are evidence for the user who wants it, not the
                // first thing the decision should be read through (desktop puts
                // them behind the same disclosure).
                Row(
                    modifier =
                        Modifier.fillMaxWidth()
                            .padding(top = Dimens.smallPadding)
                            .clickable { detailsShown = !detailsShown },
                    horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
                ) {
                    Text(
                        text = stringResource(R.string.warren_pubkey_warning_details),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(1f),
                    )
                    ExpandChevron(isExpanded = detailsShown)
                }
                if (detailsShown) {
                    Column(
                        modifier = Modifier.fillMaxWidth(),
                        verticalArrangement = Arrangement.spacedBy(Dimens.tinyPadding),
                    ) {
                        DetailRow(
                            stringResource(R.string.warren_pubkey_warning_exit_id),
                            truncatePubkeyHex(exitIdHex),
                        )
                        DetailRow(
                            stringResource(R.string.warren_pubkey_warning_pinned_key),
                            truncatePubkeyHex(pinnedPubkeyHex),
                        )
                        DetailRow(
                            stringResource(R.string.warren_pubkey_warning_observed_key),
                            truncatePubkeyHex(observedPubkeyHex),
                        )
                        if (locationText != null) {
                            DetailRow(
                                stringResource(R.string.warren_pubkey_warning_location),
                                locationText,
                            )
                        }
                    }
                }
                if (errorText != null) {
                    Text(
                        text = errorText,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.error,
                        modifier =
                            Modifier.fillMaxWidth().semantics {
                                liveRegion = LiveRegionMode.Assertive
                            },
                    )
                }
                if (busy) {
                    Text(
                        text = stringResource(R.string.warren_pubkey_warning_processing),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier =
                            Modifier.fillMaxWidth().semantics {
                                liveRegion = LiveRegionMode.Polite
                            },
                    )
                }
            }
        },
        // Trust is the affirmative-but-sensitive action (green, desktop parity);
        // rejecting is the safe default and is also what a scrim/back dismiss does.
        confirmButton = {
            PrimaryButton(
                onClick = onTrust,
                isEnabled = !busy,
                text = stringResource(R.string.warren_pubkey_warning_trust),
                colors =
                    ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.positive,
                        contentColor = MaterialTheme.colorScheme.onSelected,
                        disabledContainerColor = MaterialTheme.colorScheme.primaryDisabled,
                        disabledContentColor =
                            MaterialTheme.colorScheme.onSelected.copy(alpha = Alpha20),
                    ),
            )
        },
        dismissButton = {
            NegativeButton(
                onClick = onReject,
                isEnabled = !busy,
                text = stringResource(R.string.warren_pubkey_warning_reject),
            )
        },
        containerColor = MaterialTheme.colorScheme.surface,
    )
}

@Composable
private fun DetailRow(label: String, value: String) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = value,
            style = MaterialTheme.typography.labelSmall,
            fontFamily = FontFamily.Monospace,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.padding(start = 0.dp),
        )
    }
}

// Shortens a hex string to "head8…tail8" for the mismatch detail rows, mirroring
// the desktop truncatePubkeyHex helper. Values of 20 chars or fewer are shown
// whole.
internal fun truncatePubkeyHex(hex: String): String =
    if (hex.length <= 20) hex else "${hex.take(8)}…${hex.takeLast(8)}"
