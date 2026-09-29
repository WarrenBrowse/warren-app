package com.warrenbrowse.vpn.lib.ui.theme.tokens

import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.theme.CardTypography
import com.warrenbrowse.vpn.lib.ui.theme.WarrenFonts
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.ColorDarkTokens
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.dimensions.defaultDimensions
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The connect-screen primitives Android draws with, enumerated against the generated desktop
 * tokens: a "let me just bump this to 16" edit fails here on the platform that moved. Where Android
 * deliberately deviates, the deviation is pinned next to the desktop value it deviates from.
 */
class DesignParityTest {

    private val dims = defaultDimensions

    @Test
    fun `the connection card keeps the desktop geometry`() {
        assertEquals(DesignTokens.ConnectionCard.PaddingVertical, dims.connectionCardVerticalPadding)
        assertEquals(
            DesignTokens.ConnectionCard.PaddingHorizontal,
            dims.connectionCardHorizontalPadding,
        )
        assertEquals(DesignTokens.ConnectionCard.Radius, dims.connectionCardRadius)
        assertEquals(DesignTokens.Radius.Radius16, dims.connectionCardRadius)
        assertEquals(DesignTokens.ConnectionCard.BorderWidth, dims.surfaceBorderWidth)
        assertEquals(
            DesignTokens.ConnectionCard.MarginHorizontal,
            dims.connectionCardMarginHorizontal,
        )
        assertEquals(DesignTokens.ConnectionCard.MarginBottom, dims.connectionCardMarginBottom)
        assertEquals(DesignTokens.ConnectionCard.BlockGap, dims.connectionCardBlockGap)
        assertEquals(DesignTokens.ConnectionCard.ShadowOffsetY, dims.connectionCardShadowOffsetY)
        assertEquals(DesignTokens.ConnectionCard.ShadowBlur, dims.connectionCardShadowBlur)
    }

    @Test
    fun `the card chevron keeps the desktop box and glyph`() {
        assertEquals(DesignTokens.ConnectionCard.ChevronButtonSize, dims.connectionCardChevronSize)
        assertEquals(
            DesignTokens.ConnectionCard.ChevronIconSize,
            dims.connectionCardChevronIconSize,
        )
    }

    @Test
    fun `the feature chip keeps the desktop padding, radius and shadow`() {
        assertEquals(DesignTokens.FeatureChip.PaddingVertical, dims.chipVerticalPadding)
        assertEquals(DesignTokens.FeatureChip.PaddingHorizontal, dims.chipHorizontalPadding)
        assertEquals(DesignTokens.FeatureChip.Radius, dims.chipCornerRadius)
        assertEquals(DesignTokens.FeatureChip.BorderWidth, dims.surfaceBorderWidth)
        assertEquals(DesignTokens.FeatureChip.ShadowOffsetY, dims.chipShadowOffsetY)
        assertEquals(DesignTokens.FeatureChip.ShadowBlur, dims.chipShadowBlur)
    }

    @Test
    fun `the badge stack gaps are the desktop gaps, reachable because chips carry no touch inflation`() {
        // Measured on a 1080x2400 emulator before the inflation was turned
        // off: the pills fell exactly 48.00 dp centre to centre, because each
        // chip sat in its own minimum-interactive row, which read as a stack
        // several times too loose.
        //
        // With the row inflation off (chipInteractiveMinSize) the desktop gap
        // is reachable exactly. A pill is 27 dp tall (5.5 dp padding on each
        // side of a 15 sp line, plus the hairline), so each chip meets WCAG 2.2
        // AA 2.5.8 through its own size, 24 dp being the floor. The chips are
        // shortcuts, and every one of them is also reachable from Settings.
        assertEquals(2.dp, DesignTokens.ConnectionCard.BadgeGap)
        assertEquals(DesignTokens.ConnectionCard.BadgeGap, dims.chipStackGap)
        assertEquals(DesignTokens.ConnectionCard.BadgesToCardGap, dims.chipsToCardGap)
        assertEquals(0.dp, dims.chipInteractiveMinSize)
    }

    @Test
    fun `the card buttons keep the desktop geometry`() {
        // 32 dp is under the 48 dp finger floor, and it stays the desktop
        // height on purpose: the buttons carry no Material inflation, and
        // Compose extends the touch bounds of any pointer target smaller than
        // the platform minimum to that minimum, resolving an overlap in favour
        // of the nearest target. The 10.5 dp between the two rows keeps the
        // targets from ever claiming each other's centre.
        assertEquals(DesignTokens.CardButton.Height, dims.cardButtonHeight)
        assertEquals(DesignTokens.CardButton.Radius, dims.cardButtonRadius)
        assertEquals(DesignTokens.CardButton.BorderWidth, dims.surfaceBorderWidth)
        assertEquals(DesignTokens.CardButton.RowGap, dims.cardButtonRowGap)
        assertEquals(DesignTokens.CardButton.ShuffleWidth, dims.shuffleButtonWidth)
    }

    @Test
    fun `the beta banner keeps the desktop overlay card`() {
        assertEquals(DesignTokens.BetaBanner.MarginTop, dims.betaBannerMarginTop)
        assertEquals(DesignTokens.BetaBanner.MarginStart, dims.betaBannerMarginStart)
        assertEquals(DesignTokens.BetaBanner.Height, dims.betaBannerHeight)
        assertEquals(DesignTokens.BetaBanner.PaddingStart, dims.betaBannerPaddingStart)
        assertEquals(DesignTokens.BetaBanner.PaddingEnd, dims.betaBannerPaddingEnd)
        assertEquals(DesignTokens.BetaBanner.Gap, dims.betaBannerGap)
        assertEquals(DesignTokens.BetaBanner.Radius, dims.betaBannerRadius)
        assertEquals(DesignTokens.BetaBanner.BorderWidth, dims.surfaceBorderWidth)
        assertEquals(DesignTokens.BetaBanner.ShadowOffsetY, dims.betaBannerShadowOffsetY)
        assertEquals(DesignTokens.BetaBanner.ShadowBlur, dims.betaBannerShadowBlur)
        assertEquals(DesignTokens.BetaBanner.PillHeight, dims.betaPillHeight)
        assertEquals(DesignTokens.BetaBanner.PillPaddingHorizontal, dims.betaPillPaddingHorizontal)
        assertEquals(DesignTokens.BetaBanner.PillRadius, dims.betaPillRadius)
    }

    @Test
    fun `the footer keeps the desktop surface`() {
        assertEquals(DesignTokens.Footer.PaddingVertical, dims.footerVerticalPadding)
        assertEquals(DesignTokens.Footer.PaddingHorizontal, dims.mediumPadding)
        assertEquals(DesignTokens.Footer.SurfaceAlpha, Alpha60)
        assertEquals(DesignTokens.Footer.BorderWidth, dims.thinBorderWidth)
        assertEquals(DesignTokens.Footer.BorderAlpha, Alpha20)
    }

    @Test
    fun `the notification banner keeps the desktop card`() {
        assertEquals(DesignTokens.NotificationBanner.Radius, dims.notificationBannerRadius)
        assertEquals(DesignTokens.NotificationBanner.EdgeWidth, dims.notificationBannerEdge)
        assertEquals(
            DesignTokens.NotificationBanner.PaddingVertical,
            dims.notificationBannerVerticalPadding,
        )
        assertEquals(
            DesignTokens.NotificationBanner.PaddingStart,
            dims.notificationBannerStartPadding,
        )
        assertEquals(DesignTokens.NotificationBanner.PaddingEnd, dims.notificationBannerEndPadding)
        assertEquals(DesignTokens.NotificationBanner.Elevation, dims.notificationBannerElevation)
    }

    @Test
    fun `the country flag is the desktop round flag with its hairline ring`() {
        assertEquals(DesignTokens.CountryFlag.Size, dims.countryFlagSize)
        assertEquals(DesignTokens.CountryFlag.RingWidth, dims.surfaceBorderWidth)
    }

    @Test
    fun `the dialog radius is the desktop radius12`() {
        assertEquals(DesignTokens.Radius.Radius12, dims.dialogCornerRadius)
    }

    @Test
    fun `the status eye sits in the desktop well`() {
        assertEquals(DesignTokens.ConnectionStatus.RowGap, dims.connectionStatusGap)
        assertEquals(DesignTokens.ConnectionStatus.TextGap, dims.connectionStatusTextGap)
        assertEquals(DesignTokens.ConnectionStatus.TrailingGap, dims.connectionStatusTrailingGap)
        assertEquals(DesignTokens.ConnectionStatus.WellSize, dims.connectionStatusWellSize)
        assertEquals(DesignTokens.ConnectionStatus.WellRadius, dims.connectionStatusWellRadius)
        assertEquals(DesignTokens.ConnectionStatus.IconSize, dims.connectionStatusIconSize)
    }

    @Test
    fun `the location lines keep the desktop gaps`() {
        assertEquals(DesignTokens.ConnectionLocation.GapAbove, dims.connectionCardBlockGap)
        assertEquals(DesignTokens.ConnectionLocation.HostnameGapAbove, dims.hostnameGapAbove)
    }

    @Test
    fun `the card text is the desktop Open Sans at the desktop sizes`() {
        assertStyle(
            CardTypography.statusTitle,
            DesignTokens.ConnectionStatus.TitleSize.value,
            DesignTokens.ConnectionStatus.TitleLineHeight.value,
            DesignTokens.ConnectionStatus.TitleWeight,
        )
        assertStyle(
            CardTypography.statusSubtitle,
            DesignTokens.ConnectionStatus.SubtitleSize.value,
            DesignTokens.ConnectionStatus.SubtitleLineHeight.value,
            FontWeight.Normal.weight,
        )
        assertStyle(
            CardTypography.location,
            DesignTokens.ConnectionLocation.Size.value,
            DesignTokens.ConnectionLocation.LineHeight.value,
            DesignTokens.ConnectionLocation.Weight,
        )
        assertStyle(
            CardTypography.hostname,
            DesignTokens.ConnectionLocation.HostnameSize.value,
            DesignTokens.ConnectionLocation.HostnameLineHeight.value,
            FontWeight.Normal.weight,
        )
        assertStyle(
            CardTypography.button,
            DesignTokens.CardButton.TextSize.value,
            DesignTokens.CardButton.TextLineHeight.value,
            DesignTokens.CardButton.TextWeight,
        )
        assertEquals(DesignTokens.BetaBanner.PillTextSize, CardTypography.betaPill.fontSize)
        assertEquals(DesignTokens.BetaBanner.PillLetterSpacing, CardTypography.betaPill.letterSpacing)
        assertEquals(
            DesignTokens.BetaBanner.PillTextWeight,
            CardTypography.betaPill.fontWeight?.weight,
        )
        assertEquals(DesignTokens.BetaBanner.TextSize, CardTypography.betaLine.fontSize)
        assertEquals(DesignTokens.BetaBanner.TextWeight, CardTypography.betaLine.fontWeight?.weight)
        assertEquals(WarrenFonts.body, CardTypography.betaPill.fontFamily)
        assertEquals(WarrenFonts.body, CardTypography.betaLine.fontFamily)
    }

    @Test
    fun `the surface palettes are the desktop surfaces, field for field`() {
        assertSurfaces(
            WarrenSurfaces.Dark,
            listOf(
                DesignTokens.Surfaces.Dark.Card,
                DesignTokens.Surfaces.Dark.Line,
                DesignTokens.Surfaces.Dark.ShadowSoft,
                DesignTokens.Surfaces.Dark.ShadowStrong,
                DesignTokens.Surfaces.Dark.Pill,
                DesignTokens.Surfaces.Dark.PillText,
                DesignTokens.Surfaces.Dark.Text,
                DesignTokens.Surfaces.Dark.TextSecondary,
                DesignTokens.Surfaces.Dark.TextMuted,
                DesignTokens.Surfaces.Dark.Button,
                DesignTokens.Surfaces.Dark.ButtonPressed,
                DesignTokens.Surfaces.Dark.ButtonLine,
                DesignTokens.Surfaces.Dark.Exposed,
                DesignTokens.Surfaces.Dark.ExposedWell,
                DesignTokens.Surfaces.Dark.Connecting,
                DesignTokens.Surfaces.Dark.ConnectingWell,
                DesignTokens.Surfaces.Dark.Protected,
                DesignTokens.Surfaces.Dark.ProtectedWell,
                DesignTokens.Surfaces.Dark.Connect,
                DesignTokens.Surfaces.Dark.ConnectPressed,
                DesignTokens.Surfaces.Dark.Disconnect,
                DesignTokens.Surfaces.Dark.DisconnectPressed,
                DesignTokens.Surfaces.Dark.Cancel,
                DesignTokens.Surfaces.Dark.CancelPressed,
                DesignTokens.Surfaces.Dark.ActionText,
            ),
        )
        assertSurfaces(
            WarrenSurfaces.Light,
            listOf(
                DesignTokens.Surfaces.Light.Card,
                DesignTokens.Surfaces.Light.Line,
                DesignTokens.Surfaces.Light.ShadowSoft,
                DesignTokens.Surfaces.Light.ShadowStrong,
                DesignTokens.Surfaces.Light.Pill,
                DesignTokens.Surfaces.Light.PillText,
                DesignTokens.Surfaces.Light.Text,
                DesignTokens.Surfaces.Light.TextSecondary,
                DesignTokens.Surfaces.Light.TextMuted,
                DesignTokens.Surfaces.Light.Button,
                DesignTokens.Surfaces.Light.ButtonPressed,
                DesignTokens.Surfaces.Light.ButtonLine,
                DesignTokens.Surfaces.Light.Exposed,
                DesignTokens.Surfaces.Light.ExposedWell,
                DesignTokens.Surfaces.Light.Connecting,
                DesignTokens.Surfaces.Light.ConnectingWell,
                DesignTokens.Surfaces.Light.Protected,
                DesignTokens.Surfaces.Light.ProtectedWell,
                DesignTokens.Surfaces.Light.Connect,
                DesignTokens.Surfaces.Light.ConnectPressed,
                DesignTokens.Surfaces.Light.Disconnect,
                DesignTokens.Surfaces.Light.DisconnectPressed,
                DesignTokens.Surfaces.Light.Cancel,
                DesignTokens.Surfaces.Light.CancelPressed,
                DesignTokens.Surfaces.Light.ActionText,
            ),
        )
    }

    @Test
    fun `the material roles map onto the desktop primitives`() {
        assertEquals(DesignTokens.Colors.DarkBlue, ColorDarkTokens.Surface)
        assertEquals(DesignTokens.Colors.DarkBlue, ColorDarkTokens.Background)
        assertEquals(DesignTokens.Colors.Blue, ColorDarkTokens.Primary)
        assertEquals(DesignTokens.Colors.Red, ColorDarkTokens.Error)
        assertEquals(DesignTokens.Colors.White, ColorDarkTokens.OnSurface)
        assertEquals(DesignTokens.Colors.WhiteOnDarkBlue60, ColorDarkTokens.OnSurfaceVariant)
        assertEquals(DesignTokens.Colors.Blue10, ColorDarkTokens.SurfaceContainerLowest)
        assertEquals(DesignTokens.Colors.Blue40, ColorDarkTokens.SurfaceContainer)
        // The Material-role trap: `tertiary` is the DEEPEST neutral here, not an
        // accent. A BETA chip painted `tertiary` reads as a charcoal pill, which
        // is exactly how the ocre chip was lost once; the warning accent is
        // `warning` (desktop `yellow`), never a Material role.
        assertEquals(DesignTokens.Colors.DarkerBlue10, ColorDarkTokens.Tertiary)
    }

    private fun assertStyle(style: TextStyle, size: Float, lineHeight: Float, weight: Int) {
        assertEquals(WarrenFonts.body, style.fontFamily)
        assertEquals(size, style.fontSize.value)
        assertEquals(lineHeight, style.lineHeight.value)
        assertEquals(weight, style.fontWeight?.weight)
    }

    // In declaration order: a field swapped for its neighbour fails here.
    private fun assertSurfaces(palette: WarrenSurfaces, expected: List<androidx.compose.ui.graphics.Color>) {
        assertEquals(
            expected,
            listOf(
                palette.card,
                palette.line,
                palette.shadowSoft,
                palette.shadowStrong,
                palette.pill,
                palette.pillText,
                palette.text,
                palette.textSecondary,
                palette.textMuted,
                palette.button,
                palette.buttonPressed,
                palette.buttonLine,
                palette.exposed,
                palette.exposedWell,
                palette.connecting,
                palette.connectingWell,
                palette.protected,
                palette.protectedWell,
                palette.connect,
                palette.connectPressed,
                palette.disconnect,
                palette.disconnectPressed,
                palette.cancel,
                palette.cancelPressed,
                palette.actionText,
            ),
        )
    }
}
