package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.cases
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.float
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.floats
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.obj
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.string
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.serialization.json.JsonObject
import org.junit.jupiter.api.Test

/**
 * The scenery placement replayed from `fixtures/client-rules/scenery_layout.json`, the file the
 * desktop and iOS readers replay too. A geometry change means changing that file and every reader
 * in the same commit; a reader is never loosened to pass.
 */
class SceneryLayoutFixtureTest {

    private val fixture = ClientRulesFixtures.load("scenery_layout.json")

    private fun placementOf(case: JsonObject): SceneryLayout.Placement {
        val screen = case.floats("screen_px")
        val density = case.float("density")
        return SceneryLayout.placement(
            screenWidth = screen[0],
            screenHeight = screen[1],
            cardTop = case.float("card_top_dp") * density,
            density = density,
        )
    }

    @Test
    fun `the canvas and the rows the formula keys on are the ones the assets carry`() {
        val canvas = fixture.floats("canvas_px")
        assertEquals(SceneryLayout.CANVAS_WIDTH, canvas[0])
        assertEquals(SceneryLayout.CANVAS_HEIGHT, canvas[1])
        assertEquals(SceneryLayout.FEET_ROW, fixture.float("feet_row"))
        assertEquals(SceneryLayout.GROUND_ROW, fixture.float("ground_row"))
        assertEquals(SceneryLayout.GAP_DP, fixture.float("gap_dp"))
        assertEquals(SceneryLayout.MAX_CANVAS_PAN_DP, fixture.float("max_canvas_pan_dp"))
    }

    @Test
    fun `every fixture case places the layers where the shared formula says`() {
        val cases = fixture.cases("cases")
        assertTrue(cases.isNotEmpty(), "the fixture must carry cases")
        cases.forEach { case ->
            val name = case.string("name")
            val expect = case.obj("expect")
            val got = placementOf(case)

            fun check(key: String, actual: Float) =
                assertEquals(expect.float(key), actual, TOLERANCE, "$name.$key")

            check("scale", got.scale)
            check("canvas_height_px", got.canvasHeight)
            check("canvas_pan_px", got.canvasPan)
            check("foreground_shift_px", got.foregroundShift)
            check("landscape_top_px", got.landscapeTop)
            check("foreground_top_px", got.foregroundTop)
            check("landscape_bottom_px", got.landscapeBottom)
            check("foreground_bottom_px", got.foregroundBottom)
            check("foreground_band_top_px", got.bandTop)
            check("foreground_band_height_px", got.bandHeight)
            check("foreground_band_left_px", got.bandLeft)
            check("foreground_band_width_px", got.bandWidth)
        }
    }

    @Test
    fun `the band overflows the screen on three sides, carrying the paper margin off it`() {
        fixture.cases("cases").forEach { case ->
            val name = case.string("name")
            val screen = case.floats("screen_px")
            val got = placementOf(case)
            assertTrue(got.bandLeft < 0f, "$name: the band does not overhang the left edge")
            assertTrue(
                got.bandLeft + got.bandWidth > screen[0],
                "$name: the band does not overhang the right edge",
            )
            assertTrue(
                got.bandTop + got.bandHeight >= screen[1] - TOLERANCE,
                "$name: the band stops above the screen bottom",
            )
        }
    }

    @Test
    fun `exactly one of the pan and the slide is ever non-zero`() {
        fixture.cases("cases").forEach { case ->
            val name = case.string("name")
            val got = placementOf(case)
            assertTrue(got.canvasPan <= 0f, "$name: the canvas may only ever pan up")
            assertTrue(got.foregroundShift >= 0f, "$name: the foreground may only ever slide down")
            assertTrue(
                got.canvasPan == 0f || got.foregroundShift == 0f,
                "$name: panning and sliding at once would move the foreground twice",
            )
        }
    }

    @Test
    fun `the stretch never reaches Bula or the burrow mouth, only the meadow under them`() {
        val maxStretch = fixture.float("max_band_stretch")
        fixture.cases("cases").forEach { case ->
            val name = case.string("name")
            val got = placementOf(case)
            val feetY = SceneryLayout.FEET_ROW * got.scale
            assertTrue(got.groundOffset >= feetY, "$name: the split row would cut through Bula")
            val stretch = got.bandHeight / (got.canvasHeight - got.groundOffset)
            assertTrue(stretch >= 1f, "$name: the band was compressed ($stretch)")
            assertTrue(stretch <= maxStretch, "$name: the band stretched $stretch")
        }
    }

    @Test
    fun `the landscape is drawn whole, never stretched`() {
        fixture.cases("cases").forEach { case ->
            val name = case.string("name")
            val got = placementOf(case)
            assertEquals(
                got.canvasHeight,
                got.landscapeBottom - got.landscapeTop,
                TOLERANCE,
                "$name: the landscape was scaled",
            )
            // And the burrow's opaque rows still overlap it, which is what lets it stay whole.
            assertTrue(
                got.foregroundTop + got.groundOffset <= got.landscapeBottom + TOLERANCE,
                "$name: a window opened between the landscape and the opaque ground",
            )
        }
    }

    @Test
    fun `the foreground always reaches the screen bottom, so no band is ever left to fill`() {
        fixture.cases("cases").forEach { case ->
            val name = case.string("name")
            val screenHeight = case.floats("screen_px")[1]
            assertTrue(
                placementOf(case).foregroundBottom >= screenHeight - TOLERANCE,
                "$name: the foreground stops above the screen bottom",
            )
        }
    }

    @Test
    fun `a backdrop laid out before the card leaves the canvas where it is painted`() {
        val got =
            SceneryLayout.placement(
                screenWidth = 1080f,
                screenHeight = 2400f,
                cardTop = Float.NaN,
                density = 2.625f,
            )
        assertEquals(0f, got.canvasPan)
        assertEquals(0f, got.foregroundShift)
    }

    private companion object {
        const val TOLERANCE = 0.02f
    }
}
