package com.warrenbrowse.vpn.lib.ui.component

import androidx.annotation.DrawableRes
import androidx.compose.foundation.Image
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.sp
import com.warrenbrowse.vpn.lib.ui.resource.FlagAssets
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20

/** What the flag slot draws for a country code. */
sealed interface CountryFlagSource {
    /** The desktop flag artwork, shared with the other clients. */
    data class Artwork(@param:DrawableRes val drawable: Int) : CountryFlagSource

    /** The regional-indicator pair, for a code the shared set has no artwork for. */
    data class Emoji(val glyph: String) : CountryFlagSource
}

/**
 * The flag artwork for an ISO 3166-1 alpha-2 [countryCode], the emoji glyph
 * when the shared set lacks it, and nothing when the code is not a code. Pure,
 * so the decision is unit-testable off-device.
 */
fun countryFlagSource(countryCode: String?): CountryFlagSource? {
    val code = countryCode?.trim()?.lowercase() ?: return null
    if (code.length != 2 || !code.all { it in 'a'..'z' }) return null
    FlagAssets.drawableFor(code)?.let {
        return CountryFlagSource.Artwork(it)
    }
    val base = REGIONAL_INDICATOR_A
    val glyph =
        String(Character.toChars(base + (code[0] - 'a'))) +
            String(Character.toChars(base + (code[1] - 'a')))
    return CountryFlagSource.Emoji(glyph)
}

private const val REGIONAL_INDICATOR_A = 0x1F1E6

/**
 * Round country flag (desktop CurrentCountryFlag): the desktop's own flag
 * artwork in a circle with the white hairline, so every surface shows the same
 * flag on every client. Decorative: the text beside it names the country.
 */
@Composable
fun CountryFlag(
    countryCode: String?,
    modifier: Modifier = Modifier,
    size: Dp = Dimens.countryFlagSize,
) {
    when (val source = countryFlagSource(countryCode)) {
        is CountryFlagSource.Artwork ->
            Image(
                painter = painterResource(source.drawable),
                contentDescription = null, // The location line names the country.
                modifier =
                    modifier
                        .size(size)
                        .clip(CircleShape)
                        .border(
                            Dimens.thinBorderWidth,
                            Color.White.copy(alpha = Alpha20),
                            CircleShape,
                        ),
            )
        is CountryFlagSource.Emoji ->
            Text(text = source.glyph, fontSize = size.value.sp, modifier = modifier)
        null -> Unit
    }
}
