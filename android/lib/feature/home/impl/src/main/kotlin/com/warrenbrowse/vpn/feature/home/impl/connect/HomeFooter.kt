package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.LayoutDirection
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.HomeTypography
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import kotlinx.coroutines.delay

/** Desktop AppMainFooter: the check shows for two seconds after a copy. */
private const val FOOTER_COPIED_MILLIS = 2_000L

/**
 * The footer band of the home screen (desktop `AppMainFooter`): the short account key in the mono
 * face with its copy action, and the time left on the subscription. The band runs under the
 * navigation bar, its content above it.
 */
@Composable
internal fun HomeFooter(shortPubkey: String?, onCopyPubkey: (() -> Unit)?, timeLeft: String?) {
    val footer = DesignTokens.Home.Footer
    val surfaces = LocalWarrenSurfaces.current
    // Keyed on the tap count, not the flag, so a second tap inside the two seconds restarts them
    // the way the desktop reschedules its timer.
    var copyCount by remember { mutableIntStateOf(0) }
    var copied by remember { mutableStateOf(false) }
    LaunchedEffect(copyCount) {
        if (copyCount > 0) {
            copied = true
            delay(FOOTER_COPIED_MILLIS)
            copied = false
        }
    }
    Row(
        modifier =
            Modifier.fillMaxWidth()
                .background(surfaces.homeFooter)
                .windowInsetsPadding(WindowInsets.navigationBars.only(WindowInsetsSides.Bottom))
                .padding(horizontal = footer.PaddingHorizontal, vertical = footer.PaddingVertical),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(footer.Gap),
    ) {
        if (shortPubkey != null) {
            val copyLabel = stringResource(if (copied) R.string.copied else R.string.copy)
            Row(
                modifier =
                    Modifier.clickable(
                            enabled = onCopyPubkey != null,
                            role = Role.Button,
                            onClick = {
                                onCopyPubkey?.invoke()
                                copyCount++
                            },
                        )
                        .semantics(mergeDescendants = true) { contentDescription = copyLabel },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(footer.Gap),
            ) {
                // An address reads left to right in every language; without this the halves on
                // either side of the ellipsis swap places in Arabic and Persian.
                CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Ltr) {
                    Text(
                        text = shortPubkey,
                        style = HomeTypography.footerMono,
                        color = surfaces.homeTextSecondary,
                        maxLines = 1,
                    )
                }
                if (onCopyPubkey != null) {
                    Icon(
                        imageVector = if (copied) Icons.Rounded.Check else Icons.Rounded.ContentCopy,
                        contentDescription = null,
                        tint = if (copied) surfaces.homePillProtected else surfaces.homeTextSecondary,
                        modifier = Modifier.size(footer.Icon),
                    )
                }
            }
        }
        Spacer(Modifier.weight(1f))
        if (timeLeft != null) {
            Text(
                text = timeLeft,
                style = HomeTypography.footer,
                color = surfaces.homeTextSecondary,
                maxLines = 1,
            )
        }
    }
}
