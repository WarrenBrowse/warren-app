package com.warrenbrowse.vpn.feature.home.impl.connect

import android.os.Build
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.component.ForumHeaderIcon
import com.warrenbrowse.vpn.lib.ui.component.ForumHeaderSlot
import com.warrenbrowse.vpn.lib.ui.component.forumHeaderLabel
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.TOP_BAR_ACCOUNT_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.TOP_BAR_FORUM_BUTTON_TEST_TAG
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
private const val TOOLBAR_TINT_ALPHA = 0.12f

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
            val label = forumHeaderLabel(forumSlot)
            HomeToolbarButton(
                onClick = onForumClick,
                tint = tint,
                modifier =
                    Modifier.testTag(TOP_BAR_FORUM_BUTTON_TEST_TAG).semantics {
                        contentDescription = label
                    },
            ) {
                ForumHeaderIcon(slot = forumSlot, tint = tint, iconSize = toolbar.Icon)
            }
        }
        HomeToolbarButton(
            onClick = onAccountClick,
            tint = tint,
            modifier = Modifier.testTag(TOP_BAR_ACCOUNT_BUTTON_TEST_TAG),
        ) {
            Icon(
                painter = painterResource(id = R.drawable.ic_account_outline),
                tint = tint,
                contentDescription = stringResource(id = R.string.settings_account),
                modifier = Modifier.size(toolbar.Icon),
            )
        }
        HomeToolbarButton(
            onClick = onSettingsClick,
            tint = tint,
            modifier = Modifier.testTag(TOP_BAR_SETTINGS_BUTTON_TEST_TAG),
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

/**
 * A button of the toolbar (desktop `HomeHeader`, mockup 8d): the glyph centred in its 38 dp square,
 * which a press, a hover or the keyboard focus tints with the glyph colour at 12 %, where a ring or
 * a round ripple around an icon in the glass pill read heavy.
 */
@Composable
private fun HomeToolbarButton(
    onClick: () -> Unit,
    tint: Color,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    val toolbar = DesignTokens.Home.Toolbar
    val interactions = remember { MutableInteractionSource() }
    val pressed by interactions.collectIsPressedAsState()
    val focused by interactions.collectIsFocusedAsState()
    val hovered by interactions.collectIsHoveredAsState()
    Box(
        modifier =
            modifier
                .size(toolbar.Button)
                .clip(RoundedCornerShape(toolbar.Radius - toolbar.PaddingHorizontal))
                .background(
                    if (pressed || focused || hovered) tint.copy(alpha = TOOLBAR_TINT_ALPHA)
                    else Color.Transparent
                )
                .hoverable(interactions)
                .clickable(
                    interactionSource = interactions,
                    indication = null,
                    role = Role.Button,
                    onClick = onClick,
                ),
        contentAlignment = Alignment.Center,
    ) {
        content()
    }
}
