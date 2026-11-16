package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures
import com.warrenbrowse.vpn.fixtures.ClientRulesFixtures.booleanOr
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
            compact = case.booleanOr("compact", false),
            fullHeight = case["full_hero_height_px"]?.let { case.float("full_hero_height_px") },
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
            // A compact hero gives the span bottom to the fade on purpose.
            if (!case.booleanOr("compact", false)) {
                assertTrue(
                    spanBottom <= hero[1] - case.float("fade_px") + TOLERANCE,
                    "$name: in the fade",
                )
            }
            assertTrue(got.left >= -TOLERANCE, "$name: cropped on the left")
            assertTrue(got.left + got.width <= hero[0] + TOLERANCE, "$name: cropped on the right")
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
        assertEquals(
            fixture.float("hero_max_height_ratio"),
            SceneryFraming.HERO_MAX_HEIGHT_RATIO,
            1e-6f,
        )
        assertEquals(
            fixture.float("compact_height_ratio"),
            SceneryFraming.COMPACT_HEIGHT_RATIO,
            1e-6f,
        )
        assertEquals(fixture.int("compact_transition_ms"), SceneryFraming.COMPACT_TRANSITION_MS)
        assertEquals(fixture.float("margin_blur_px"), SceneryFraming.MARGIN_BLUR)
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
        assertEquals(
            HALF,
            maskAlpha(SceneryFraming.CLEAR_BOTTOM_FRACTION),
            TOLERANCE,
            "the clear bottom is not where the mask reaches half opacity",
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
    fun `every fixture hero is the height the hero rule gives its width`() {
        val headerBand = hero.HeaderInset + toolbar.Height + hero.ClearMargin
        fixture.cases("cases").forEach { case ->
            val name = case.string("name")
            val (width, height) = case.floats("hero_px")
            // The fixture's clear_top is the status bar plus the header band.
            val statusBar = case.float("clear_top_px") - headerBand.value
            when {
                // A tablet's hero takes its height from the content column, not its own width.
                name.startsWith("wide_") -> Unit
                case.booleanOr("compact", false) ->
                    assertEquals(
                        height,
                        SceneryFraming.compactHeroHeight(width, statusBar),
                        ROUNDING,
                        name,
                    )
                // The tall phone took all the free height a hero may take.
                name.endsWith("_tall_ground") ->
                    assertEquals(
                        height,
                        SceneryFraming.heroHeight(width, statusBar, available = Float.MAX_VALUE),
                        ROUNDING,
                        name,
                    )
                else ->
                    assertEquals(
                        height,
                        SceneryFraming.heroHeight(width, statusBar, available = height),
                        ROUNDING,
                        name,
                    )
            }
        }
    }

    @Test
    fun `a phone hero absorbs the free height between the mockup ratio and its max`() {
        val width = 412f
        val statusBar = 24f
        val natural = width * SceneryFraming.HERO_HEIGHT_RATIO + statusBar
        val max = width * SceneryFraming.HERO_MAX_HEIGHT_RATIO + statusBar
        val compact = width * SceneryFraming.COMPACT_HEIGHT_RATIO + statusBar
        assertEquals(max, SceneryFraming.heroHeight(width, statusBar, available = 2000f))
        assertEquals(natural + 40f, SceneryFraming.heroHeight(width, statusBar, natural + 40f))
        // Short of room, it gives way down to the compact hero and no further.
        assertEquals(natural - 40f, SceneryFraming.heroHeight(width, statusBar, natural - 40f))
        assertEquals(compact, SceneryFraming.heroHeight(width, statusBar, available = 10f))
    }

    @Test
    fun `the blurred backdrop covers the hero and keeps the span where the canvas has it`() {
        val case = fixture.cases("cases").single { it.string("name") == "wide_ground" }
        val (width, height) = case.floats("hero_px")
        val span = case.floats("span_rows").let { SceneryFraming.Span(it[0], it[1]) }
        val frame = frameOf(case)
        assertTrue(frame.feathersSides, "a narrowed canvas has side margins")
        val cover = SceneryFraming.backdrop(width, height, frame, span)
        assertTrue(cover.left <= 0f && cover.left + cover.width >= width, "covers the width")
        assertTrue(cover.top <= 0f && cover.top + cover.height >= height, "covers the height")
        val middle = (span.top + span.bottom) / 2f
        assertEquals(
            frame.top + middle * frame.scale,
            cover.top + middle * cover.scale,
            TOLERANCE,
            "the span's middle row lines up with the canvas",
        )
        val desktop = frameOf(fixture.cases("cases").first())
        assertTrue(!desktop.feathersSides, "a width-fit canvas has no margin to fill")
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

    /** The mask's alpha at [at] of the hero height, between its stops. */
    private fun maskAlpha(at: Float): Float {
        val stops = SceneryFraming.MASK_STOPS
        val upper = stops.indexOfFirst { it.first >= at }
        if (upper <= 0) return stops.first().second
        val (a0, v0) = stops[upper - 1]
        val (a1, v1) = stops[upper]
        return v0 + (v1 - v0) * (at - a0) / (a1 - a0)
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
