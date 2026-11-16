package com.warrenbrowse.vpn.lib.ui.designsystem

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.lib.ui.designsystem.preview.PreviewColumn
import com.warrenbrowse.vpn.lib.ui.theme.HomeTypography
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens

@Preview
@Composable
private fun PreviewWarrenFeatureChip() {
    PreviewColumn {
        WarrenFeatureChip(text = "DAITA", onClick = {})
        WarrenFeatureChip(text = "Local Network Sharing", onClick = {})
        WarrenFeatureChip(text = "Port forwarding blocked", onClick = {}, isError = true)
    }
}

/**
 * A feature chip under the home controls (desktop `FeatureIndicator`): an outlined pill on the
 * page, no fill. The error variant (a port forward the exit refused) takes the disconnect line, so
 * it reads on the home screen without opening settings.
 */
@Composable
fun WarrenFeatureChip(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    isError: Boolean = false,
) {
    val surfaces = LocalWarrenSurfaces.current
    val chips = DesignTokens.Home.Chips
    Surface(
        onClick = onClick,
        modifier = modifier.heightIn(min = chips.Height),
        shape = CircleShape,
        color = Color.Transparent,
        contentColor = surfaces.homeChipText,
        border =
            BorderStroke(
                chips.BorderWidth,
                if (isError) surfaces.homeDisconnectLine else surfaces.homeChipLine,
            ),
    ) {
        Text(
            text = text,
            style = HomeTypography.chip,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier =
                Modifier.padding(
                    horizontal = chips.PaddingHorizontal,
                    vertical = chips.PaddingVertical,
                ),
        )
    }
}
