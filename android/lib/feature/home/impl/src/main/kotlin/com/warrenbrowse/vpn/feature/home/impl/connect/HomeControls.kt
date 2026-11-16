package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Shuffle
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.feature.home.impl.connect.button.CardButton
import com.warrenbrowse.vpn.feature.home.impl.connect.button.CardButtonTone
import com.warrenbrowse.vpn.feature.home.impl.connect.button.ConnectionButton
import com.warrenbrowse.vpn.feature.home.impl.connect.button.colors
import com.warrenbrowse.vpn.lib.ui.component.CountryFlag
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.CONNECT_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SELECT_LOCATION_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SHUFFLE_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.HomeTypography
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

private const val CONNECT_BUTTON_THROTTLE_MILLIS = 1000
private val SHUFFLE_ICON_SIZE = 20.dp

// The well a thumbnail sits in until its landscape is decoded.
private const val THUMB_WELL_ALPHA = 0.1f

/**
 * The home controls (desktop `HomeControls`): the location row with its round thumbnail and the
 * square shuffle beside it, then the full-width action button.
 */
@Composable
@Suppress("LongParameterList")
internal fun HomeControls(
    state: ConnectUiState,
    locationName: String,
    thumb: LocationThumb,
    focusRequester: FocusRequester,
    shuffleEnabled: Boolean,
    onSwitchLocationClick: () -> Unit,
    onShuffleClick: () -> Unit,
    onConnectClick: () -> Unit,
    onCancelClick: () -> Unit,
    onDisconnectClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val controls = DesignTokens.Home.Controls
    var lastActionAt by remember { mutableLongStateOf(0L) }

    // Every tunnel action shares the throttle: a Disconnect-then-Connect double tap would
    // otherwise dispatch a connect into a teardown that is still in flight.
    fun throttled(action: () -> Unit) {
        val now = System.currentTimeMillis()
        if (now - lastActionAt > CONNECT_BUTTON_THROTTLE_MILLIS) {
            lastActionAt = now
            action()
        }
    }

    val field = CardButtonTone.Neutral.colors(LocalWarrenSurfaces.current)
    Column(
        modifier = modifier.fillMaxWidth().padding(top = controls.MarginTop),
        verticalArrangement = Arrangement.spacedBy(controls.Gap),
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(controls.Gap)) {
            val description = stringResource(R.string.home_select_location, locationName)
            CardButton(
                colors = field,
                onClick = onSwitchLocationClick,
                height = controls.RowHeight,
                modifier =
                    Modifier.weight(1f)
                        .testTag(SELECT_LOCATION_BUTTON_TEST_TAG)
                        .focusRequester(focusRequester)
                        .semantics(mergeDescendants = true) { contentDescription = description },
            ) {
                LocationRowContent(locationName, thumb)
            }
            CardButton(
                colors = field,
                onClick = { throttled(onShuffleClick) },
                enabled = shuffleEnabled,
                height = controls.RowHeight,
                modifier = Modifier.size(controls.RowHeight).testTag(SHUFFLE_BUTTON_TEST_TAG),
            ) {
                Icon(
                    imageVector = Icons.Rounded.Shuffle,
                    contentDescription = stringResource(id = R.string.random_location),
                    tint = LocalWarrenSurfaces.current.homeFieldIcon,
                    modifier = Modifier.size(SHUFFLE_ICON_SIZE),
                )
            }
        }
        ConnectionButton(
            modifier = Modifier.fillMaxWidth().testTag(CONNECT_BUTTON_TEST_TAG),
            state = state.tunnelState,
            disconnectClick = { throttled(onDisconnectClick) },
            cancelClick = { throttled(onCancelClick) },
            connectClick = { throttled(onConnectClick) },
        )
    }
}

@Composable
private fun LocationRowContent(name: String, thumb: LocationThumb) {
    val controls = DesignTokens.Home.Controls
    val surfaces = LocalWarrenSurfaces.current
    Row(
        modifier =
            Modifier.fillMaxSize().padding(start = controls.PaddingStart, end = controls.PaddingEnd),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(controls.RowGap),
    ) {
        LocationThumbnail(thumb)
        Text(
            text = name,
            style = HomeTypography.location,
            color = surfaces.homeText,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        Text(
            text = stringResource(R.string.home_change),
            style = HomeTypography.change,
            color = surfaces.homeTextSecondary,
            maxLines = 1,
        )
    }
}

/**
 * The round picture of a location (desktop `LocationThumb`): a square of its landscape crowned by
 * the skyline, decoded off the main thread from the shared scenery cache, or its flag.
 */
@Composable
private fun LocationThumbnail(thumb: LocationThumb) {
    val size = DesignTokens.Home.Controls.Thumb
    when (thumb) {
        is LocationThumb.Flag -> CountryFlag(countryCode = thumb.countryCode, size = size)
        is LocationThumb.Art -> {
            val context = LocalContext.current
            val image by
                produceState<ImageBitmap?>(null, thumb.landscape) {
                    value =
                        withContext(Dispatchers.IO) {
                            runCatching { SceneryBitmaps.of(context).get(thumb.landscape) }
                                .getOrNull()
                        }
                }
            Box(
                Modifier.size(size)
                    .clip(CircleShape)
                    .background(Color.Black.copy(alpha = THUMB_WELL_ALPHA))
            ) {
                image?.let { bitmap ->
                    Canvas(Modifier.fillMaxSize()) {
                        val rowScale = bitmap.width / SceneryFraming.CANVAS_WIDTH
                        val top = (thumb.topRow * rowScale).roundToInt()
                        drawImage(
                            image = bitmap,
                            srcOffset = IntOffset(0, top),
                            srcSize = IntSize(bitmap.width, minOf(bitmap.width, bitmap.height - top)),
                            dstSize = IntSize(this.size.width.roundToInt(), this.size.height.roundToInt()),
                        )
                    }
                }
            }
        }
    }
}
