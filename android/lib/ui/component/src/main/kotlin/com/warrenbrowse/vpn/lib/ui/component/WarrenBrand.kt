package com.warrenbrowse.vpn.lib.ui.component

import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

// The desktop warren-wordmark.svg lockup viewport: the ears W and "arren" are
// baked as one glyph run (mirrors TopBar's WARREN_LOCKUP_ASPECT), so the brand
// reads as a single word rather than mark + text side by side.
private const val WORDMARK_LOCKUP_ASPECT = 17355f / 5720f

/**
 * The one Warren brand primitive, kept in lockstep with the desktop
 * `warren-wordmark.svg`: the ears mark fused with the wordmark, the W being the
 * ears. Never split into a mark and a text; the desktop deleted that pair and
 * so did Android.
 */
@Composable
fun WarrenWordmarkLockup(
    color: Color,
    modifier: Modifier = Modifier,
    height: Dp = 40.dp,
) {
    Icon(
        painter = painterResource(id = R.drawable.wordmark_warren),
        contentDescription = null, // Decorative; the screen text conveys the name.
        tint = color,
        modifier = modifier.height(height).width(height * WORDMARK_LOCKUP_ASPECT),
    )
}
