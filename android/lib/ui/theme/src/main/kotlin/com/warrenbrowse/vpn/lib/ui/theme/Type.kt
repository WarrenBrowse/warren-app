package com.warrenbrowse.vpn.lib.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import com.warrenbrowse.vpn.lib.ui.theme.R

/*
The app currently uses the following text styles directly in the code:
headlineLarge (32sp 700 weight) -> Used for title in PrivacyDisclaimer, Welcome and Login
headlineSmall (24sp 600 weight) -> Used for title in DeviceRevoked, ReportAProblem etc
titleLarge (22sp 600 weight) -> Used for Connection status and location
titleMedium (16sp 600 weight) -> Used for cell header text and button text
bodyLarge (16sp 400 weight) -> Used for title in two row cells and some other non-standard cells
bodyMedium (14sp 400 weight) -> Used for descriptions in screens and descriptions for cells
bodySmall (12sp 400 weight) -> Disclaimer texts and error texts under inputs
labelLarge (14sp 500 weight) -> Cell that are not header cells, Dialog texts, device name and expiry
 */

/**
 * The desktop typefaces, bundled from the same files the Electron app ships
 * (`desktop/packages/mullvad-vpn/assets/fonts`): Instrument Sans for every
 * title, body and label, JetBrains Mono for keys, codes and host names. Neither
 * carries Cyrillic; Android falls back to the system face for those glyphs on
 * its own, which is what the desktop's Open Sans fallback does there.
 */
object WarrenFonts {
    val sans: FontFamily =
        FontFamily(
            Font(R.font.instrument_sans_regular, FontWeight.Normal),
            Font(R.font.instrument_sans_medium, FontWeight.Medium),
            Font(R.font.instrument_sans_semibold, FontWeight.SemiBold),
            Font(R.font.instrument_sans_bold, FontWeight.Bold),
        )

    val mono: FontFamily =
        FontFamily(
            Font(R.font.jetbrains_mono_regular, FontWeight.Normal),
            Font(R.font.jetbrains_mono_medium, FontWeight.Medium),
        )
}

internal val MullvadMaterial3Typography =
    with(Typography()) {
        this.copy(
            displayLarge = displayLarge.merge(fontFamily = WarrenFonts.sans),
            displayMedium = displayMedium.merge(fontFamily = WarrenFonts.sans),
            displaySmall = displaySmall.merge(fontFamily = WarrenFonts.sans),
            headlineLarge =
                headlineLarge.merge(fontFamily = WarrenFonts.sans, fontWeight = FontWeight.Bold),
            headlineMedium =
                headlineMedium.merge(fontFamily = WarrenFonts.sans, fontWeight = FontWeight.SemiBold),
            headlineSmall =
                headlineSmall.merge(fontFamily = WarrenFonts.sans, fontWeight = FontWeight.SemiBold),
            titleLarge =
                titleLarge.merge(fontFamily = WarrenFonts.sans, fontWeight = FontWeight.SemiBold),
            titleMedium =
                titleMedium.merge(fontFamily = WarrenFonts.sans, fontWeight = FontWeight.SemiBold),
            titleSmall = titleSmall.merge(fontFamily = WarrenFonts.sans),
            bodyLarge = bodyLarge.merge(fontFamily = WarrenFonts.sans),
            bodyMedium = bodyMedium.merge(fontFamily = WarrenFonts.sans),
            bodySmall = bodySmall.merge(fontFamily = WarrenFonts.sans),
            labelLarge = labelLarge.merge(fontFamily = WarrenFonts.sans),
            labelMedium = labelMedium.merge(fontFamily = WarrenFonts.sans),
            labelSmall = labelSmall.merge(fontFamily = WarrenFonts.sans),
        )
    }

/**
 * The home screen's text, at the desktop home sizes (`homeTokens`): the status pill, the title and
 * its subtitle, the location row, the action button, the chips, the beta line and the footer.
 * Colours come from the surfaces of the resolved theme, so none is set here.
 */
object HomeTypography {
    val pill =
        TextStyle(
            fontFamily = WarrenFonts.sans,
            fontWeight = FontWeight(DesignTokens.Home.Pill.TextWeight),
            fontSize = DesignTokens.Home.Pill.TextSize,
            lineHeight = DesignTokens.Home.Pill.TextSize * LINE_HEIGHT_TIGHT,
        )
    val title =
        TextStyle(
            fontFamily = WarrenFonts.sans,
            fontWeight = FontWeight(DesignTokens.Home.Title.TextWeight),
            fontSize = DesignTokens.Home.Title.TextSize,
            lineHeight = DesignTokens.Home.Title.TextSize * LINE_HEIGHT_TIGHT,
            letterSpacing = DesignTokens.Home.Title.LetterSpacingEm.em,
        )
    val subtitle =
        TextStyle(
            fontFamily = WarrenFonts.sans,
            fontWeight = FontWeight.Normal,
            fontSize = DesignTokens.Home.Subtitle.TextSize,
        )
    val subtitleMono =
        TextStyle(
            fontFamily = WarrenFonts.mono,
            fontWeight = FontWeight.Normal,
            fontSize = DesignTokens.Home.Subtitle.MonoSize,
        )
    val location =
        TextStyle(
            fontFamily = WarrenFonts.sans,
            fontWeight = FontWeight(DesignTokens.Home.Controls.LabelWeight),
            fontSize = DesignTokens.Home.Controls.LabelSize,
        )
    val change =
        TextStyle(
            fontFamily = WarrenFonts.sans,
            fontWeight = FontWeight.Normal,
            fontSize = DesignTokens.Home.Controls.ChangeSize,
        )
    val button =
        TextStyle(
            fontFamily = WarrenFonts.sans,
            fontWeight = FontWeight(DesignTokens.Home.Controls.ButtonTextWeight),
            fontSize = DesignTokens.Home.Controls.ButtonTextSize,
        )
    val chip =
        TextStyle(
            fontFamily = WarrenFonts.sans,
            fontWeight = FontWeight.Normal,
            fontSize = DesignTokens.Home.Chips.TextSize,
        )
    val footer =
        TextStyle(
            fontFamily = WarrenFonts.sans,
            fontWeight = FontWeight.Normal,
            fontSize = DesignTokens.Home.Footer.TextSize,
            lineHeight = FOOTER_LINE_HEIGHT,
        )
    val footerMono = footer.copy(fontFamily = WarrenFonts.mono)
    val exitLoad =
        TextStyle(
            fontFamily = WarrenFonts.sans,
            fontWeight = FontWeight.SemiBold,
            fontSize = DesignTokens.Home.Subtitle.MonoSize,
            fontFeatureSettings = "tnum",
        )
    // The desktop BetaBadge home variant: a 10 px bold chip and a 12 px line.
    val betaChip =
        TextStyle(
            fontFamily = WarrenFonts.sans,
            fontWeight = FontWeight.Bold,
            fontSize = BETA_CHIP_TEXT_SIZE,
            letterSpacing = BETA_CHIP_LETTER_SPACING,
        )
    val betaLine = footer.copy(lineHeight = TextUnit.Unspecified)
}

private const val LINE_HEIGHT_TIGHT = 1.2f
private val FOOTER_LINE_HEIGHT = 16.sp
private val BETA_CHIP_TEXT_SIZE = 10.sp
private val BETA_CHIP_LETTER_SPACING = 0.5.sp
