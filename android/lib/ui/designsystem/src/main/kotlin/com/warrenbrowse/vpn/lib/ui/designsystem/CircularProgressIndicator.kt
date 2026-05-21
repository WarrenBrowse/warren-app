package com.warrenbrowse.vpn.lib.ui.designsystem

import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.tooling.preview.Preview
import com.warrenbrowse.vpn.lib.ui.designsystem.preview.PreviewColumn
import com.warrenbrowse.vpn.lib.ui.tag.CIRCULAR_PROGRESS_INDICATOR_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20

@Preview
@Composable
private fun PreviewMullvadProgressIndicator() {
    PreviewColumn {
        WarrenCircularProgressIndicatorLarge()
        WarrenCircularProgressIndicatorMedium()
        WarrenCircularProgressIndicatorSmall()
    }
}

@Composable
fun WarrenCircularProgressIndicatorLarge(
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.onSurface,
    trackColor: Color = color.copy(alpha = Alpha20),
) {
    CircularProgressIndicator(
        modifier
            .padding(Dimens.tinyPadding)
            .size(Dimens.circularProgressBarLargeSize)
            .testTag(CIRCULAR_PROGRESS_INDICATOR_TEST_TAG),
        color,
        Dimens.circularProgressBarLargeStrokeWidth,
        trackColor,
        StrokeCap.Round,
    )
}

@Composable
fun WarrenCircularProgressIndicatorMedium(
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.onSurface,
    trackColor: Color = color.copy(alpha = Alpha20),
) {
    CircularProgressIndicator(
        modifier
            .size(Dimens.circularProgressBarMediumSize)
            .testTag(CIRCULAR_PROGRESS_INDICATOR_TEST_TAG),
        color,
        Dimens.circularProgressBarMediumStrokeWidth,
        trackColor,
        StrokeCap.Round,
    )
}

@Composable
fun WarrenCircularProgressIndicatorSmall(
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.onSurface,
    trackColor: Color = color.copy(alpha = Alpha20),
) {
    CircularProgressIndicator(
        modifier
            .size(Dimens.circularProgressBarSmallSize)
            .testTag(CIRCULAR_PROGRESS_INDICATOR_TEST_TAG),
        color,
        Dimens.circularProgressBarSmallStrokeWidth,
        trackColor,
        StrokeCap.Round,
    )
}
