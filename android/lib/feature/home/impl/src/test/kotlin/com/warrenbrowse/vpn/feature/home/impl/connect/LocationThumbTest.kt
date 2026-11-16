package com.warrenbrowse.vpn.feature.home.impl.connect

import com.warrenbrowse.vpn.lib.ui.resource.R
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The round picture of the location row (desktop `LocationThumb`): a square of the country's
 * landscape crowned by its skyline, the flag of a country without art, the watched plain when no
 * country is named.
 */
class LocationThumbTest {

    @Test
    fun `a country with art shows its landscape from just above the skyline`() {
        val finland = SceneryTable.landscapes.getValue(R.drawable.scenery_finland)
        assertEquals(
            LocationThumb.Art(
                R.drawable.scenery_finland,
                SceneryFraming.thumbnailTopRow(finland.skylineTopRow),
            ),
            locationThumb("FI"),
        )
    }

    @Test
    fun `a country without art shows its flag`() {
        assertEquals(LocationThumb.Flag("se"), locationThumb("SE"))
    }

    @Test
    fun `no country shows the plain`() {
        val plain = SceneryTable.landscapes.getValue(R.drawable.scenery_plaine)
        val expected =
            LocationThumb.Art(
                R.drawable.scenery_plaine,
                SceneryFraming.thumbnailTopRow(plain.skylineTopRow),
            )
        assertEquals(expected, locationThumb(null))
        assertEquals(expected, locationThumb(" "))
    }
}
