package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenAlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenTextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.graphics.Color
import androidx.compose.foundation.layout.wrapContentWidth
import androidx.compose.foundation.border
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.color.warning

private const val FORUM_URL = "https://forum.warrenbrowse.com/"
private const val BPS_PER_MBPS = 1_000_000L

/**
 * The beta identity badge: a "BETA" chip plus a one-line
 * degraded-service note, tappable to a small explanation dialog with a
 * forum link. Callers gate it on the compiled flavor
 * ([com.warrenbrowse.vpn.lib.repository.WarrenProductFlags.isBeta]);
 * the badge itself only renders what it is given. [capBps] is the
 * bandwidth cap in bits per second, null for "no cap".
 *
 * [capResolved] says whether the cap is KNOWN at all. Until it is, the
 * note is held rather than rendered: the two wordings differ, so
 * printing the cap-unknown one first swapped the line under the user's
 * eyes a second into every cold start.
 */
/**
 * Where the badge sits (desktop BetaBadge): [Overlay] is the glass pill over
 * the scenery on the home screen, left aligned and only as wide as its line;
 * [Row] is the full-width settings row on a charcoal screen.
 */
enum class BetaBadgeVariant {
    Overlay,
    Row,
}

@Composable
fun BetaBadge(
    capBps: Long?,
    capResolved: Boolean,
    modifier: Modifier = Modifier,
    variant: BetaBadgeVariant = BetaBadgeVariant.Row,
) {
    var dialogVisible by remember { mutableStateOf(false) }
    val capMbps = capBps?.let { (it / BPS_PER_MBPS).toInt() }?.takeIf { it > 0 }

    // The pill is 32 dp tall and the row 44: both keep their visual size while
    // the node reserves the 48 dp touch floor around them, the way a Material
    // chip does (minimumInteractiveComponentSize before the clip, so the
    // reserved space stays unpainted).
    val container =
        when (variant) {
            BetaBadgeVariant.Overlay -> {
                val shape = RoundedCornerShape(14.dp)
                Modifier.minimumInteractiveComponentSize()
                    .wrapContentWidth()
                    .clip(shape)
                    .background(Color.Black.copy(alpha = Alpha60))
                    .border(1.dp, Color.White.copy(alpha = Alpha20), shape)
                    .clickable { dialogVisible = true }
                    .padding(horizontal = 12.dp, vertical = 6.dp)
            }
            BetaBadgeVariant.Row ->
                Modifier.minimumInteractiveComponentSize()
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(12.dp))
                    .background(MaterialTheme.colorScheme.surfaceContainerLowest)
                    .clickable { dialogVisible = true }
                    .padding(horizontal = 16.dp, vertical = 12.dp)
        }
    Row(modifier = modifier.then(container), verticalAlignment = Alignment.CenterVertically) {
        Text(
            text = stringResource(R.string.beta_badge_label),
            // Desktop labelTinySemiBold: 12/600.
            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
            // Desktop chip: brand ocre with charcoal text. The Material
            // `tertiary` role resolves to the deepest neutral in this scheme,
            // which painted the chip near-black.
            color = MaterialTheme.colorScheme.surface,
            modifier = Modifier
                .clip(RoundedCornerShape(8.dp))
                .background(MaterialTheme.colorScheme.warning)
                .padding(horizontal = 8.dp, vertical = 2.dp),
        )
        if (capResolved) {
            Text(
                text = if (capMbps != null) {
                    stringResource(R.string.beta_badge_line_capped, capMbps)
                } else {
                    stringResource(R.string.beta_badge_line)
                },
                // Desktop footnoteMini: the smallest semibold at 60 % white.
                style = MaterialTheme.typography.labelSmall.copy(fontWeight = FontWeight.SemiBold),
                color = MaterialTheme.colorScheme.onSurface.copy(alpha = Alpha60),
                modifier = Modifier.padding(start = 10.dp),
            )
        }
    }

    if (dialogVisible) {
        BetaInfoDialog(capMbps = capMbps, onDismiss = { dialogVisible = false })
    }
}

@Composable
private fun BetaInfoDialog(capMbps: Int?, onDismiss: () -> Unit) {
    val uriHandler = LocalUriHandler.current
    WarrenAlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(text = stringResource(R.string.beta_dialog_title)) },
        text = {
            Column {
                Text(
                    text = stringResource(R.string.beta_dialog_intro),
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    text = if (capMbps != null) {
                        stringResource(R.string.beta_dialog_cap_capped, capMbps)
                    } else {
                        stringResource(R.string.beta_dialog_cap)
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.padding(top = 8.dp),
                )
                Text(
                    text = stringResource(R.string.beta_dialog_terms),
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.padding(top = 8.dp),
                )
                // Both text buttons carry an explicit content colour. Material
                // defaults them to colorScheme.primary, which in the Warren
                // palette is a NEUTRAL SURFACE grey (PaletteTokens.Blue), not an
                // accent, so the default rendered both labels a shade away from
                // the dialog surface and left them all but invisible.
                WarrenTextButton(
                    onClick = { uriHandler.openUri(FORUM_URL) },
                    colors =
                        ButtonDefaults.textButtonColors(
                            contentColor = MaterialTheme.colorScheme.warning,
                        ),
                ) {
                    Text(text = stringResource(R.string.beta_dialog_forum))
                }
            }
        },
        confirmButton = {
            WarrenTextButton(
                onClick = onDismiss,
                colors =
                    ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.onSurface,
                    ),
            ) {
                Text(text = stringResource(R.string.got_it))
            }
        },
        containerColor = MaterialTheme.colorScheme.surface,
    )
}
