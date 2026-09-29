package com.warrenbrowse.vpn.feature.home.impl.connect.button

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.CardTypography
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces

// The glyph at the size of the card's other glyphs (eye, chevron).
private val SHUFFLE_ICON_SIZE = 18.dp

@Preview
@Composable
private fun PreviewSwitchLocationButton() {
    AppTheme { SwitchLocationButton(text = "Switch location", onSwitchLocation = {}, onShuffle = {}) }
}

/**
 * The location selector and, at its side, the "surprise me" shuffle that picks a random exit: two
 * separate rounded neutral buttons 4 dp apart, present in EVERY connection state, like the
 * desktop `SelectLocationButtons`.
 */
@Composable
fun SwitchLocationButton(
    text: String,
    onSwitchLocation: () -> Unit,
    onShuffle: () -> Unit,
    modifier: Modifier = Modifier,
    // A shuffle with no active exit to land on would be a tap that silently
    // does nothing, so the affordance is disabled instead.
    shuffleEnabled: Boolean = true,
    shuffleButtonTestTag: String = "",
) {
    val colors = CardButtonTone.Neutral.colors(LocalWarrenSurfaces.current)
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(Dimens.cardButtonRowGap),
    ) {
        CardButton(colors = colors, onClick = onSwitchLocation, modifier = modifier.weight(1f)) {
            Text(
                text = text,
                style = CardTypography.button,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(horizontal = Dimens.smallPadding),
            )
        }
        CardButton(
            colors = colors,
            onClick = onShuffle,
            enabled = shuffleEnabled,
            modifier = Modifier.width(Dimens.shuffleButtonWidth).testTag(shuffleButtonTestTag),
        ) {
            Icon(
                imageVector = Icons.Rounded.Shuffle,
                contentDescription = stringResource(id = R.string.random_location),
                modifier = Modifier.size(SHUFFLE_ICON_SIZE),
            )
        }
    }
}
