package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.cases
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.float
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.floats
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.int
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.obj
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.string
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonPrimitive
import org.junit.jupiter.api.Test

/**
 * The home hero framing replayed from `fixtures/client-rules/scenery_framing.json`, the file the
 * desktop and iOS readers replay too. A framing change means changing that file and every reader in
 * the same commit; a reader is never loosened to pass.
 */
class SceneryFramingFixtureTest {

    private val fixture = ClientRulesFixtures.load("scenery_framing.json")

    private fun frameOf(case: JsonObject): SceneryFraming.Frame {
        val hero = case.floats("hero_px")
        val span = case.floats("span_rows")
        return SceneryFraming.frame(
            width = hero[0],
            height = hero[1],
            clearTop = case.float("clear_top_px"),
            fade = case.float("fade_px"),
            span = SceneryFraming.Span(span[0], span[1]),
        )
    }

    @Test
    fun `the canvas, the feather and the spans are the ones the fixture names`() {
        val canvas = fixture.floats("canvas_px")
        assertEquals(SceneryFraming.CANVAS_WIDTH, canvas[0])
        assertEquals(SceneryFraming.CANVAS_HEIGHT, canvas[1])
        assertEquals(SceneryFraming.FEATHER_ROWS, fixture.float("feather_rows"))
        val framings = fixture.obj("framings")
        val ground = framings.obj("ground")
        assertEquals(
            SceneryFraming.Span(ground.float("top_row"), ground.float("bottom_row")),
            SceneryFraming.GROUND_SPAN,
        )
        assertEquals(SceneryFraming.SKY_BOTTOM_ROW, framings.obj("sky").float("bottom_row"))
        assertEquals(
            SceneryFraming.THUMB_MARGIN_ROWS,
            fixture.obj("thumbnail").int("thumb_margin_rows"),
        )
    }

    @Test
    fun `every fixture case frames the canvas where the shared formula says`() {
        val cases = fixture.cases("cases")
        assertTrue(cases.isNotEmpty(), "the fixture must carry cases")
        cases.forEach { case ->
            val name = case.string("name")
            val expect = case.obj("expect")
            val span = case.floats("span_rows")
            val got = frameOf(case)

            fun check(key: String, actual: Float) =
                assertEquals(expect.float(key), actual, TOLERANCE, "$name.$key")

            check("scale", got.scale)
            check("canvas_left_px", got.left)
            check("canvas_top_px", got.top)
            check("canvas_width_px", got.width)
            check("canvas_height_px", got.height)
            check("headroom_px", got.headroom)
            check("span_top_px", got.top + span[0] * got.scale)
            check("span_bottom_px", got.top + span[1] * got.scale)
        }
    }

    /**
     * The point of the framing, stated as a property: the span sits between the header and the
     * fade, and the canvas is never side cropped, in every case of the fixture.
     */
    @Test
    fun `the span is clear of the header and of the fade, and the canvas is never cropped`() {
        fixture.cases("cases").forEach { case ->
            val name = case.string("name")
            val hero = case.floats("hero_px")
            val span = case.floats("span_rows")
            val got = frameOf(case)
            val spanTop = got.top + span[0] * got.scale
            val spanBottom = got.top + span[1] * got.scale
            assertTrue(spanTop >= case.float("clear_top_px") - TOLERANCE, "$name: under the header")
            assertTrue(
                spanBottom <= hero[1] - case.float("fade_px") + TOLERANCE,
                "$name: in the fade",
            )
            assertTrue(got.left >= -TOLERANCE, "$name: cropped on the left")
            assertTrue(got.left + got.width <= hero[0] + TOLERANCE, "$name: cropped on the right")
        }
    }

    @Test
    fun `every thumbnail case starts at the row the fixture rule gives`() {
        val cases = fixture.obj("thumbnail").cases("cases")
        assertTrue(cases.isNotEmpty(), "the fixture must carry thumbnail cases")
        cases.forEach { case ->
            val skyline = case.int("skyline_top_row")
            assertEquals(
                case.int("top_row"),
                SceneryFraming.thumbnailTopRow(skyline),
                "skyline $skyline",
            )
        }
    }

    @Test
    fun `a phase frames the ground while Bula is out and the skyline once he is in`() {
        val netherlands = SceneryTable.landscapes.getValue(R.drawable.scenery_netherlands)
        assertEquals(
            SceneryFraming.GROUND_SPAN,
            SceneryFraming.span(ConnectionPhase.Exposed, "nl"),
        )
        assertEquals(
            SceneryFraming.GROUND_SPAN,
            SceneryFraming.span(ConnectionPhase.Connecting, "nl"),
        )
        assertEquals(
            SceneryFraming.Span(netherlands.skylineTopRow.toFloat(), SceneryFraming.SKY_BOTTOM_ROW),
            SceneryFraming.span(ConnectionPhase.Protected, "nl"),
        )
        // A country without art is the plain, whose skyline row is its camera row.
        val plain = SceneryTable.landscapes.getValue(R.drawable.scenery_plaine)
        assertEquals(
            SceneryFraming.Span(plain.skylineTopRow.toFloat(), SceneryFraming.SKY_BOTTOM_ROW),
            SceneryFraming.span(ConnectionPhase.Protected, "se"),
        )
    }

    @Test
    fun `the hero keeps the mockup proportions and fades along the mockup curve`() {
        assertEquals(fixture.float("hero_height_ratio"), SceneryFraming.HERO_HEIGHT_RATIO, 1e-6f)
        val stops =
            fixture.getValue("mask_stops").jsonArray.map { stop ->
                val (at, alpha) = stop.jsonArray.map { it.jsonPrimitive.content.toFloat() }
                at to alpha
            }
        assertEquals(stops, SceneryFraming.MASK_STOPS)
        assertEquals(fixture.float("clear_bottom_fraction"), SceneryFraming.CLEAR_BOTTOM_FRACTION)
    }

    @Test
    fun `the clear zone ends where the hero fade reaches half opacity`() {
        assertTrue(
            SceneryFraming.MASK_STOPS.contains(SceneryFraming.CLEAR_BOTTOM_FRACTION to HALF),
            "the clear bottom is not the half-opacity stop of the mask",
        )
        fixture.cases("cases").forEach { case ->
            val height = case.floats("hero_px")[1]
            assertEquals(
                case.float("fade_px"),
                SceneryFraming.fade(height),
                TOLERANCE,
                case.string("name"),
            )
        }
    }

    @Test
    fun `the hero is the mockup ratio of its width under the status bar, unless the panel needs it`() {
        val headerBand = hero.HeaderInset + toolbar.Height + hero.ClearMargin
        fixture.cases("cases").forEach { case ->
            val name = case.string("name")
            val (width, height) = case.floats("hero_px")
            // The fixture's clear_top is the status bar plus the header band.
            val statusBar = case.float("clear_top_px") - headerBand.value
            val natural = SceneryFraming.heroHeight(width, statusBar, available = Float.MAX_VALUE)
            if (name.startsWith("short_hero")) {
                assertTrue(natural > height, "$name: the fixture gives this hero less room")
            } else {
                assertEquals(height, natural, ROUNDING, name)
            }
            assertEquals(height, SceneryFraming.heroHeight(width, statusBar, height), ROUNDING, name)
        }
        assertEquals(0f, SceneryFraming.heroHeight(360f, 0f, available = -10f))
    }

    @Test
    fun `the feather follows the headroom on top and the side margins on the sides`() {
        val wide =
            SceneryFraming.frame(
                width = 900f,
                height = 420f,
                clearTop = 64f,
                fade = 44f,
                span = SceneryFraming.Span(140f, 930f),
            )
        assertTrue(wide.feathersTop)
        assertTrue(wide.feathersSides)
        val flush =
            SceneryFraming.frame(
                width = 412f,
                height = 520f,
                clearTop = 64f,
                fade = 44f,
                span = SceneryFraming.GROUND_SPAN,
            )
        assertTrue(!flush.feathersTop && !flush.feathersSides)
    }

    private companion object {
        const val TOLERANCE = 0.01f
        // The fixture writes hero heights in whole pixels.
        const val ROUNDING = 0.5f
        const val HALF = 0.5f
        val hero = DesignTokens.Home.Hero
        val toolbar = DesignTokens.Home.Toolbar
    }
}
