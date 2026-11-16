package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.widthIn
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
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.HomeTypography
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import kotlinx.coroutines.delay

/** Desktop AppMainFooter: the check shows for two seconds after a copy. */
private const val FOOTER_COPIED_MILLIS = 2_000L

// Desktop StyledEnd: the beta cap, a middle dot and the time left, 4 px apart.
private val FOOTER_END_GAP = 4.dp
private const val FOOTER_DOT = "\u00B7"

/**
 * The footer band of the home screen (desktop `AppMainFooter`): the short account key in the mono
 * face with its copy action, and at its end the time left on the subscription, after the [beta]
 * label and its cap on a beta build ("BETA 20 Mbps · 14 days", mockup 8d). The band runs under the
 * navigation bar, its content above it.
 */
@Composable
internal fun HomeFooter(
    shortPubkey: String?,
    onCopyPubkey: (() -> Unit)?,
    timeLeft: String?,
    beta: (@Composable () -> Unit)? = null,
) {
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
    // The band spans the screen; on a wide one its content keeps the home's centred column.
    Box(
        Modifier.fillMaxWidth()
            .background(surfaces.homeFooter)
            .windowInsetsPadding(WindowInsets.navigationBars.only(WindowInsetsSides.Bottom))
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal)),
        contentAlignment = Alignment.TopCenter,
    ) {
        Row(
            modifier =
                Modifier.widthIn(max = HomeLayout.COLUMN_MAX_WIDTH.dp)
                    .fillMaxWidth()
                    .padding(
                        horizontal = footer.PaddingHorizontal,
                        vertical = footer.PaddingVertical,
                    ),
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
                            imageVector =
                                if (copied) Icons.Rounded.Check else Icons.Rounded.ContentCopy,
                            contentDescription = null,
                            tint =
                                if (copied) surfaces.homePillProtected
                                else surfaces.homeTextSecondary,
                            modifier = Modifier.size(footer.Icon),
                        )
                    }
                }
            }
            Spacer(Modifier.weight(1f))
            FooterEnd(beta, timeLeft)
        }
    }
}

/** The footer's end: "14 days left", or on a beta build "BETA 20 Mbps · 14 days". */
@Composable
private fun FooterEnd(beta: (@Composable () -> Unit)?, timeLeft: String?) {
    val surfaces = LocalWarrenSurfaces.current
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(FOOTER_END_GAP),
    ) {
        beta?.invoke()
        if (beta != null && timeLeft != null) {
            Text(
                text = FOOTER_DOT,
                style = HomeTypography.footer,
                color = surfaces.homeTextSecondary,
                modifier = Modifier.clearAndSetSemantics {},
            )
        }
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
