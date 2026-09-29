package com.warrenbrowse.vpn.lib.ui.designsystem

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.dropShadow
import androidx.compose.ui.graphics.shadow.Shadow
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.designsystem.preview.PreviewColumn
import com.warrenbrowse.vpn.lib.ui.theme.CardTypography
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.shape.chipShape

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
 * A feature badge above the connection card (desktop FeatureIndicator): an opaque pill in the
 * card's paper with its hairline and a soft shadow, 5.5 x 8 padding, 11/600 label, radius 7. A
 * translucent chip took the hue of whatever landscape it floated over. The error variant (a port
 * forward the exit refused) is the exposed well with the exposed hairline.
 */
@Composable
fun WarrenFeatureChip(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    isError: Boolean = false,
) {
    val surfaces = LocalWarrenSurfaces.current
    val shape = MaterialTheme.shapes.chipShape
    Surface(
        onClick = onClick,
        modifier =
            modifier.dropShadow(
                shape,
                Shadow(
                    radius = Dimens.chipShadowBlur,
                    color = surfaces.shadowSoft,
                    offset = DpOffset(0.dp, Dimens.chipShadowOffsetY),
                ),
            ),
        shape = shape,
        color = if (isError) surfaces.exposedWell else surfaces.card,
        contentColor = surfaces.text,
        border =
            BorderStroke(
                Dimens.surfaceBorderWidth,
                if (isError) surfaces.exposed else surfaces.line,
            ),
    ) {
        Text(
            text = text,
            style = CardTypography.chip,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier =
                Modifier.padding(
                    horizontal = Dimens.chipHorizontalPadding,
                    vertical = Dimens.chipVerticalPadding,
                ),
        )
    }
}
