package com.warrenbrowse.vpn.feature.home.impl.connect

import android.os.Build
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.dropShadow
import androidx.compose.ui.graphics.BlurEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.shadow.Shadow
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.component.ForumHeaderButton
import com.warrenbrowse.vpn.lib.ui.component.ForumHeaderSlot
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.TOP_BAR_ACCOUNT_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.TOP_BAR_SETTINGS_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.TOP_BAR_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens

// The desktop warren-wordmark.svg lockup viewport: the ears W and "arren" are one glyph run.
private const val WARREN_LOCKUP_ASPECT = 17355f / 5720f

// The paper halo the desktop draws with three drop shadows (1, 4 and 14 px); one blurred copy of
// the wordmark at the middle radius carries it here.
private val LOGO_HALO_BLUR = 4.dp
private val LOGO_PADDING = 4.dp
private val TOOLBAR_SHADOW_BLUR = 14.dp
private val TOOLBAR_SHADOW_SPREAD = (-6).dp
private val TOOLBAR_SHADOW_OFFSET = 4.dp

/**
 * The floating header over the hero (desktop `HomeHeader`): the inked wordmark on its paper halo,
 * dark in both themes like the art, and the translucent toolbar with the forum, account and
 * settings buttons. The toolbar is a dark glass in the dark theme and a paper one in the light
 * theme, so its glyphs take the home text colour, which is the opposite tone in each.
 */
@Composable
internal fun HomeHeader(
    forumSlot: ForumHeaderSlot?,
    onForumClick: () -> Unit,
    onAccountClick: () -> Unit,
    onSettingsClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val toolbar = DesignTokens.Home.Toolbar
    Row(
        modifier = modifier.fillMaxWidth().height(toolbar.Height).testTag(TOP_BAR_TEST_TAG),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        HomeLogo()
        HomeToolbar(forumSlot, onForumClick, onAccountClick, onSettingsClick)
    }
}

@Composable
private fun HomeLogo() {
    val height = DesignTokens.Home.Hero.LogoHeight
    val surfaces = LocalWarrenSurfaces.current
    val wordmark = painterResource(id = R.drawable.wordmark_warren)
    val logoModifier = Modifier.height(height).width(height * WARREN_LOCKUP_ASPECT)
    Box(Modifier.padding(horizontal = LOGO_PADDING), contentAlignment = Alignment.Center) {
        // RenderEffect needs API 31; below it the radial glow drawn by the hero carries the halo.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            Icon(
                painter = wordmark,
                contentDescription = null,
                tint = surfaces.homeLogoGlow.copy(alpha = 1f),
                modifier =
                    logoModifier.graphicsLayer {
                        val radius = LOGO_HALO_BLUR.toPx()
                        renderEffect = BlurEffect(radius, radius, TileMode.Decal)
                    },
            )
        }
        Icon(
            painter = wordmark,
            contentDescription = null, // Decorative brand lockup.
            tint = Color.Black,
            modifier = logoModifier,
        )
    }
}

@Composable
private fun HomeToolbar(
    forumSlot: ForumHeaderSlot?,
    onForumClick: () -> Unit,
    onAccountClick: () -> Unit,
    onSettingsClick: () -> Unit,
) {
    val toolbar = DesignTokens.Home.Toolbar
    val surfaces = LocalWarrenSurfaces.current
    val shape = RoundedCornerShape(toolbar.Radius)
    val tint = surfaces.homeText
    Row(
        modifier =
            Modifier.height(toolbar.Height)
                .dropShadow(
                    shape,
                    Shadow(
                        radius = TOOLBAR_SHADOW_BLUR,
                        color = surfaces.homeToolbarShadow,
                        spread = TOOLBAR_SHADOW_SPREAD,
                        offset = DpOffset(0.dp, TOOLBAR_SHADOW_OFFSET),
                    ),
                )
                .clip(shape)
                .background(surfaces.homeToolbar)
                .border(1.dp, surfaces.homeToolbarLine, shape)
                .padding(horizontal = toolbar.PaddingHorizontal),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (forumSlot != null) {
            ForumHeaderButton(
                slot = forumSlot,
                tint = tint,
                enabled = true,
                onClick = onForumClick,
                modifier = Modifier.size(toolbar.Button),
                iconSize = toolbar.Icon,
            )
        }
        IconButton(
            modifier = Modifier.size(toolbar.Button).testTag(TOP_BAR_ACCOUNT_BUTTON_TEST_TAG),
            onClick = onAccountClick,
        ) {
            Icon(
                painter = painterResource(id = R.drawable.ic_account_outline),
                tint = tint,
                contentDescription = stringResource(id = R.string.settings_account),
                modifier = Modifier.size(toolbar.Icon),
            )
        }
        IconButton(
            modifier = Modifier.size(toolbar.Button).testTag(TOP_BAR_SETTINGS_BUTTON_TEST_TAG),
            onClick = onSettingsClick,
        ) {
            Icon(
                painter = painterResource(id = R.drawable.ic_settings_outline),
                tint = tint,
                contentDescription = stringResource(id = R.string.settings),
                modifier = Modifier.size(toolbar.Icon),
            )
        }
    }
}
