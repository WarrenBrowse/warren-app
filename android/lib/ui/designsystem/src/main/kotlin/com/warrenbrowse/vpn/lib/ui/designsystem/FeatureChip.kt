package com.warrenbrowse.vpn.lib.ui.designsystem

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Surface
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.lib.ui.designsystem.preview.PreviewColumn
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha40
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
 * A feature badge above the connection card (desktop FeatureIndicator): a
 * 22 dp pill, 2 x 8 padding, 12/600 label, radius 8, `blue10` fill with the
 * `blue` hairline; the error variant (a port forward the exit refused) is the
 * red fill at 40 % with the red hairline. The visual is small on purpose; the
 * clickable Surface reserves the 48 dp touch floor around it on its own.
 */
@Composable
fun WarrenFeatureChip(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    isError: Boolean = false,
) {
    val containerColor =
        if (isError) MaterialTheme.colorScheme.error.copy(alpha = Alpha40)
        else MaterialTheme.colorScheme.surfaceContainerLowest
    val borderColor =
        if (isError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary
    Surface(
        onClick = onClick,
        modifier = modifier,
        shape = MaterialTheme.shapes.chipShape,
        color = containerColor,
        contentColor = MaterialTheme.colorScheme.onPrimary,
        border = BorderStroke(1.dp, borderColor),
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
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
