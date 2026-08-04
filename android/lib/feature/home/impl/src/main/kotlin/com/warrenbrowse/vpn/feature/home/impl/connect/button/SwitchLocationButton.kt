package com.warrenbrowse.vpn.feature.home.impl.connect.button

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.constraintlayout.compose.ConstraintLayout
import androidx.constraintlayout.compose.Dimension
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20

@Preview
@Composable
private fun PreviewConnectionButton() {
    AppTheme {
        Column(verticalArrangement = Arrangement.spacedBy(Dimens.mediumSpacer)) {
            SwitchLocationButton(
                text = "Switch Location",
                onSwitchLocation = {},
                onShuffle = {},
            )
        }
    }
}

/**
 * The location selector, split with a "surprise me" shuffle side button that
 * picks a random exit. The shuffle is present in EVERY connection state,
 * matching the desktop `SelectLocationButtons` (which always pairs the selector
 * with a ShuffleButton). Android used to swap the side button to a reconnect
 * action while a tunnel was up; that reconnect affordance does not exist on the
 * desktop home screen, so the side button is now always the shuffle.
 */
@Composable
@Suppress("LongMethod")
fun SwitchLocationButton(
    text: String,
    onSwitchLocation: () -> Unit,
    onShuffle: () -> Unit,
    modifier: Modifier = Modifier,
    // A shuffle with no active exit to land on would be a tap that silently
    // does nothing, so the affordance goes away instead.
    shuffleEnabled: Boolean = true,
    shuffleButtonTestTag: String = "",
) {
    ConstraintLayout(
        modifier = modifier.padding(vertical = Dimens.connectButtonExtraPadding).fillMaxWidth()
    ) {
        // initial height set at 0.dp
        var componentHeight by remember { mutableStateOf(0.dp) }

        // get local density from composable
        val density = LocalDensity.current

        val (connectionButton, shuffleButton) = createRefs()
        CompositionLocalProvider(
            LocalMinimumInteractiveComponentSize provides
                Dimens.reconnectButtonMinInteractiveComponentSize
        ) {
            val dividerSize = Dimens.reconnectButtonDivider

            Button(
                onClick = onSwitchLocation,
                // 4dp square corners to match the Connect button (design-system
                // WarrenButtonShape). Split with the side button, only the outer
                // (start) corners are rounded; the inner edge is square.
                shape = RoundedCornerShape(topStart = 4.dp, bottomStart = 4.dp),
                colors =
                    ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.primaryContainer,
                        contentColor = MaterialTheme.colorScheme.onPrimary,
                    ),
                modifier =
                    Modifier.constrainAs(connectionButton) {
                            start.linkTo(parent.start)
                            end.linkTo(shuffleButton.start)
                            width = Dimension.fillToConstraints
                            height = Dimension.wrapContent
                        }
                        .onGloballyPositioned {
                            componentHeight = with(density) { it.size.height.toDp() }
                        },
            ) {
                // Offset to compensate for the side button.
                Text(
                    text = text,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier =
                        Modifier.padding(
                            start = componentHeight + Dimens.listItemDivider + Dimens.smallPadding
                        ),
                )
            }

            FilledIconButton(
                shape = RoundedCornerShape(topEnd = 4.dp, bottomEnd = 4.dp),
                colors =
                    IconButtonDefaults.filledIconButtonColors(
                        containerColor = MaterialTheme.colorScheme.primaryContainer,
                        contentColor = MaterialTheme.colorScheme.onPrimary,
                        disabledContainerColor =
                            MaterialTheme.colorScheme.primaryContainer.copy(alpha = Alpha20),
                        disabledContentColor =
                            MaterialTheme.colorScheme.onPrimary.copy(alpha = Alpha20),
                    ),
                onClick = onShuffle,
                enabled = shuffleEnabled,
                modifier =
                    Modifier.testTag(shuffleButtonTestTag)
                        .constrainAs(shuffleButton) {
                            start.linkTo(connectionButton.end, margin = dividerSize)
                            top.linkTo(connectionButton.top)
                            bottom.linkTo(connectionButton.bottom)
                            end.linkTo(parent.end)
                            height = Dimension.fillToConstraints
                        }
                        .defaultMinSize(minWidth = Dimens.switchLocationRetryMinWidth),
            ) {
                Icon(
                    imageVector = Icons.Rounded.Shuffle,
                    contentDescription = stringResource(id = R.string.random_location),
                )
            }
        }
    }
}
