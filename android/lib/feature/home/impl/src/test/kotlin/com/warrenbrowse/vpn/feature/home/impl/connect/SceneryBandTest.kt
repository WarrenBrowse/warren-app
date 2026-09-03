package com.warrenbrowse.vpn.feature.home.impl.connect

import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

/**
 * The geometry of the blurred continuation band under the scenery canvas:
 * the rows the mirror shows below the seam, plus the margin that keeps the
 * blur's edge fade off screen. The margin follows the two blur radii, so a
 * desktop token change that widens a blur cannot silently bring the fade,
 * a dark gradient above the footer, back into view.
 */
class SceneryBandTest {

    private val density = Density(density = 1f, fontScale = 1f)

    @Test
    fun `the band covers the rows the mirror shows below the seam plus the margin`() {
        // A 1080 x 2424 screen: the 1140 x 1706 canvas ends 1616.2 px down,
        // the mirror fills the 807.8 px below it.
        val height = continuationBandHeight(widthPx = 1080f, heightPx = 2424f, density = density)

        val seam = 1080f * 1706f / 1140f
        assertEquals(2424f - seam + CONTINUATION_BAND_MARGIN.value, height, 0.01f)
    }

    @Test
    fun `the band is the margin alone when the canvas reaches the bottom of the screen`() {
        // A short, wide screen: the seam is below the screen and the mirror
        // shows nothing; the band still exists for the feather to blend into.
        val height = continuationBandHeight(widthPx = 2000f, heightPx = 1000f, density = density)

        assertEquals(CONTINUATION_BAND_MARGIN.value, height, 0.01f)
    }

    @Test
    fun `the margin keeps the blur edge fade past the bottom of the screen`() {
        // The layer's edge treatment is Decal and a Gaussian of radius r fades
        // about 1.5 r inward from an edge; the band's blur is the continuation
        // radius plus the connecting radius at its widest.
        val widestRadius = CONTINUATION_BLUR_RADIUS + LANDSCAPE_BLUR_RADIUS
        assertTrue(
            CONTINUATION_BAND_MARGIN >= widestRadius * 1.5f,
            "margin ${CONTINUATION_BAND_MARGIN} against a fade of ${widestRadius * 1.5f}",
        )
        assertEquals(widestRadius * 2, CONTINUATION_BAND_MARGIN)
        assertEquals(84.dp, CONTINUATION_BAND_MARGIN)
    }
}
