package com.warrenbrowse.vpn.lib.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
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
 * (`desktop/packages/mullvad-vpn/assets/fonts`): Source Sans Pro for every
 * title, Open Sans for body and label text. Before this the whole app rendered
 * in Roboto, which was the one thing a side-by-side of the two clients showed
 * first.
 */
object WarrenFonts {
    val title: FontFamily =
        FontFamily(
            Font(R.font.source_sans_pro_semibold, FontWeight.SemiBold),
            Font(R.font.source_sans_pro_bold, FontWeight.Bold),
        )

    val body: FontFamily =
        FontFamily(
            Font(R.font.open_sans_regular, FontWeight.Normal),
            Font(R.font.open_sans_semibold, FontWeight.SemiBold),
            Font(R.font.open_sans_bold, FontWeight.Bold),
        )
}

internal val MullvadMaterial3Typography =
    with(Typography()) {
        this.copy(
            displayLarge = displayLarge.merge(fontFamily = WarrenFonts.title),
            displayMedium = displayMedium.merge(fontFamily = WarrenFonts.title),
            displaySmall = displaySmall.merge(fontFamily = WarrenFonts.title),
            headlineLarge =
                headlineLarge.merge(fontFamily = WarrenFonts.title, fontWeight = FontWeight.Bold),
            headlineMedium =
                headlineMedium.merge(fontFamily = WarrenFonts.title, fontWeight = FontWeight.SemiBold),
            headlineSmall =
                headlineSmall.merge(fontFamily = WarrenFonts.title, fontWeight = FontWeight.SemiBold),
            titleLarge =
                titleLarge.merge(fontFamily = WarrenFonts.title, fontWeight = FontWeight.SemiBold),
            titleMedium =
                titleMedium.merge(fontFamily = WarrenFonts.title, fontWeight = FontWeight.SemiBold),
            titleSmall = titleSmall.merge(fontFamily = WarrenFonts.title),
            bodyLarge = bodyLarge.merge(fontFamily = WarrenFonts.body),
            bodyMedium = bodyMedium.merge(fontFamily = WarrenFonts.body),
            bodySmall = bodySmall.merge(fontFamily = WarrenFonts.body),
            labelLarge = labelLarge.merge(fontFamily = WarrenFonts.body),
            labelMedium = labelMedium.merge(fontFamily = WarrenFonts.body),
            labelSmall = labelSmall.merge(fontFamily = WarrenFonts.body),
        )
    }

/**
 * The connect screen's card, chips and beta banner, written in Open Sans at the desktop sizes
 * (desktop ConnectionStatus, Location, Hostname, CardButton, FeatureIndicatorText, BetaBadge).
 * Colours come from the surfaces of the resolved theme, so none is set here.
 */
object CardTypography {
    val statusTitle =
        TextStyle(
            fontFamily = WarrenFonts.body,
            fontWeight = FontWeight.SemiBold,
            fontSize = 16.sp,
            lineHeight = 19.2.sp,
        )
    val statusSubtitle =
        TextStyle(
            fontFamily = WarrenFonts.body,
            fontWeight = FontWeight.Normal,
            fontSize = 11.5.sp,
            lineHeight = 15.sp,
        )
    val location =
        TextStyle(
            fontFamily = WarrenFonts.body,
            fontWeight = FontWeight.Bold,
            fontSize = 15.sp,
            lineHeight = 19.sp,
        )
    val hostname =
        TextStyle(
            fontFamily = WarrenFonts.body,
            fontWeight = FontWeight.Normal,
            fontSize = 12.sp,
            lineHeight = 16.5.sp,
        )
    val exitLoad =
        TextStyle(
            fontFamily = WarrenFonts.body,
            fontWeight = FontWeight.SemiBold,
            fontSize = 11.sp,
            lineHeight = 15.sp,
            fontFeatureSettings = "tnum",
        )
    val button =
        TextStyle(
            fontFamily = WarrenFonts.body,
            fontWeight = FontWeight.SemiBold,
            fontSize = 13.sp,
            lineHeight = 18.sp,
        )
    val chip =
        TextStyle(
            fontFamily = WarrenFonts.body,
            fontWeight = FontWeight.SemiBold,
            fontSize = 11.sp,
            lineHeight = 15.sp,
        )
    val betaPill =
        TextStyle(
            fontFamily = WarrenFonts.body,
            fontWeight = FontWeight.Bold,
            fontSize = 11.sp,
            letterSpacing = 0.5.sp,
        )
    val betaLine =
        TextStyle(fontFamily = WarrenFonts.body, fontWeight = FontWeight.SemiBold, fontSize = 11.sp)
}
