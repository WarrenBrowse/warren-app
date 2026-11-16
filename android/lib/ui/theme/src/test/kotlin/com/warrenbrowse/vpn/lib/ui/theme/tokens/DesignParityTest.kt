package com.warrenbrowse.vpn.lib.ui.theme.tokens

import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.TextUnit
import com.warrenbrowse.vpn.lib.ui.theme.HomeTypography
import com.warrenbrowse.vpn.lib.ui.theme.WarrenFonts
import com.warrenbrowse.vpn.lib.ui.theme.color.ColorDarkTokens
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.dimensions.defaultDimensions
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The primitives Android draws with, enumerated against the generated desktop tokens: a "let me
 * just bump this to 16" edit fails here on the platform that moved.
 */
class DesignParityTest {

    private val dims = defaultDimensions

    @Test
    fun `the dialog radius is the desktop radius12`() {
        assertEquals(DesignTokens.Radius.Radius12, dims.dialogCornerRadius)
    }

    @Test
    fun `the home text is Instrument Sans at the desktop home sizes`() {
        val home = HomeTypography
        assertHome(home.pill, DesignTokens.Home.Pill.TextSize, DesignTokens.Home.Pill.TextWeight)
        assertHome(home.title, DesignTokens.Home.Title.TextSize, DesignTokens.Home.Title.TextWeight)
        assertEquals(DesignTokens.Home.Title.LetterSpacingEm, home.title.letterSpacing.value)
        assertHome(home.subtitle, DesignTokens.Home.Subtitle.TextSize, FontWeight.Normal.weight)
        assertHome(
            home.location,
            DesignTokens.Home.Controls.LabelSize,
            DesignTokens.Home.Controls.LabelWeight,
        )
        assertHome(home.change, DesignTokens.Home.Controls.ChangeSize, FontWeight.Normal.weight)
        assertHome(
            home.button,
            DesignTokens.Home.Controls.ButtonTextSize,
            DesignTokens.Home.Controls.ButtonTextWeight,
        )
        assertHome(home.chip, DesignTokens.Home.Chips.TextSize, FontWeight.Normal.weight)
        assertHome(home.footer, DesignTokens.Home.Footer.TextSize, FontWeight.Normal.weight)
        // The host name and the account key are set in the desktop mono face.
        assertEquals(WarrenFonts.mono, home.subtitleMono.fontFamily)
        assertEquals(DesignTokens.Home.Subtitle.MonoSize, home.subtitleMono.fontSize)
        assertEquals(WarrenFonts.mono, home.footerMono.fontFamily)
        assertEquals(DesignTokens.Home.Footer.TextSize, home.footerMono.fontSize)
    }

    @Test
    fun `the dark home palette is the desktop dark home surfaces, field for field`() {
        assertHomeSurfaces(
            WarrenSurfaces.Dark,
            listOf(
                DesignTokens.Surfaces.Dark.HomePage,
                DesignTokens.Surfaces.Dark.HomeText,
                DesignTokens.Surfaces.Dark.HomeTextSecondary,
                DesignTokens.Surfaces.Dark.HomeTextMono,
                DesignTokens.Surfaces.Dark.HomeField,
                DesignTokens.Surfaces.Dark.HomeFieldHover,
                DesignTokens.Surfaces.Dark.HomeFieldPressed,
                DesignTokens.Surfaces.Dark.HomeFieldIcon,
                DesignTokens.Surfaces.Dark.HomeFooter,
                DesignTokens.Surfaces.Dark.HomeChipText,
                DesignTokens.Surfaces.Dark.HomeChipLine,
                DesignTokens.Surfaces.Dark.HomeToolbar,
                DesignTokens.Surfaces.Dark.HomeToolbarLine,
                DesignTokens.Surfaces.Dark.HomeToolbarShadow,
                DesignTokens.Surfaces.Dark.HomeBadge,
                DesignTokens.Surfaces.Dark.HomeBadgeRing,
                DesignTokens.Surfaces.Dark.HomeScrim,
                DesignTokens.Surfaces.Dark.HomeLogoGlow,
                DesignTokens.Surfaces.Dark.HomePillExposed,
                DesignTokens.Surfaces.Dark.HomePillExposedText,
                DesignTokens.Surfaces.Dark.HomePillProtected,
                DesignTokens.Surfaces.Dark.HomePillProtectedText,
                DesignTokens.Surfaces.Dark.HomePillPending,
                DesignTokens.Surfaces.Dark.HomePillPendingText,
                DesignTokens.Surfaces.Dark.HomePillBlocked,
                DesignTokens.Surfaces.Dark.HomePillBlockedText,
                DesignTokens.Surfaces.Dark.HomeConnect,
                DesignTokens.Surfaces.Dark.HomeConnectHover,
                DesignTokens.Surfaces.Dark.HomeConnectPressed,
                DesignTokens.Surfaces.Dark.HomeConnectText,
                DesignTokens.Surfaces.Dark.HomeDisconnectLine,
                DesignTokens.Surfaces.Dark.HomeDisconnectText,
                DesignTokens.Surfaces.Dark.HomeDisconnectHover,
                DesignTokens.Surfaces.Dark.HomeCancelLine,
                DesignTokens.Surfaces.Dark.HomeCancelText,
                DesignTokens.Surfaces.Dark.HomeCancelHover,
            ),
        )
    }

    @Test
    fun `the light home palette is the desktop light home surfaces, field for field`() {
        assertHomeSurfaces(
            WarrenSurfaces.Light,
            listOf(
                DesignTokens.Surfaces.Light.HomePage,
                DesignTokens.Surfaces.Light.HomeText,
                DesignTokens.Surfaces.Light.HomeTextSecondary,
                DesignTokens.Surfaces.Light.HomeTextMono,
                DesignTokens.Surfaces.Light.HomeField,
                DesignTokens.Surfaces.Light.HomeFieldHover,
                DesignTokens.Surfaces.Light.HomeFieldPressed,
                DesignTokens.Surfaces.Light.HomeFieldIcon,
                DesignTokens.Surfaces.Light.HomeFooter,
                DesignTokens.Surfaces.Light.HomeChipText,
                DesignTokens.Surfaces.Light.HomeChipLine,
                DesignTokens.Surfaces.Light.HomeToolbar,
                DesignTokens.Surfaces.Light.HomeToolbarLine,
                DesignTokens.Surfaces.Light.HomeToolbarShadow,
                DesignTokens.Surfaces.Light.HomeBadge,
                DesignTokens.Surfaces.Light.HomeBadgeRing,
                DesignTokens.Surfaces.Light.HomeScrim,
                DesignTokens.Surfaces.Light.HomeLogoGlow,
                DesignTokens.Surfaces.Light.HomePillExposed,
                DesignTokens.Surfaces.Light.HomePillExposedText,
                DesignTokens.Surfaces.Light.HomePillProtected,
                DesignTokens.Surfaces.Light.HomePillProtectedText,
                DesignTokens.Surfaces.Light.HomePillPending,
                DesignTokens.Surfaces.Light.HomePillPendingText,
                DesignTokens.Surfaces.Light.HomePillBlocked,
                DesignTokens.Surfaces.Light.HomePillBlockedText,
                DesignTokens.Surfaces.Light.HomeConnect,
                DesignTokens.Surfaces.Light.HomeConnectHover,
                DesignTokens.Surfaces.Light.HomeConnectPressed,
                DesignTokens.Surfaces.Light.HomeConnectText,
                DesignTokens.Surfaces.Light.HomeDisconnectLine,
                DesignTokens.Surfaces.Light.HomeDisconnectText,
                DesignTokens.Surfaces.Light.HomeDisconnectHover,
                DesignTokens.Surfaces.Light.HomeCancelLine,
                DesignTokens.Surfaces.Light.HomeCancelText,
                DesignTokens.Surfaces.Light.HomeCancelHover,
            ),
        )
    }

    @Test
    fun `each palette knows which theme it is`() {
        assertEquals(true, WarrenSurfaces.Dark.isDark)
        assertEquals(false, WarrenSurfaces.Light.isDark)
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

    private fun assertHome(style: TextStyle, size: TextUnit, weight: Int) {
        assertEquals(WarrenFonts.sans, style.fontFamily)
        assertEquals(size, style.fontSize)
        assertEquals(weight, style.fontWeight?.weight)
    }

    private fun assertHomeSurfaces(palette: WarrenSurfaces, expected: List<Color>) {
        assertEquals(
            expected,
            listOf(
                palette.homePage,
                palette.homeText,
                palette.homeTextSecondary,
                palette.homeTextMono,
                palette.homeField,
                palette.homeFieldHover,
                palette.homeFieldPressed,
                palette.homeFieldIcon,
                palette.homeFooter,
                palette.homeChipText,
                palette.homeChipLine,
                palette.homeToolbar,
                palette.homeToolbarLine,
                palette.homeToolbarShadow,
                palette.homeBadge,
                palette.homeBadgeRing,
                palette.homeScrim,
                palette.homeLogoGlow,
                palette.homePillExposed,
                palette.homePillExposedText,
                palette.homePillProtected,
                palette.homePillProtectedText,
                palette.homePillPending,
                palette.homePillPendingText,
                palette.homePillBlocked,
                palette.homePillBlockedText,
                palette.homeConnect,
                palette.homeConnectHover,
                palette.homeConnectPressed,
                palette.homeConnectText,
                palette.homeDisconnectLine,
                palette.homeDisconnectText,
                palette.homeDisconnectHover,
                palette.homeCancelLine,
                palette.homeCancelText,
                palette.homeCancelHover,
            ),
        )
    }

    // In declaration order: a field swapped for its neighbour fails here.
    private fun assertSurfaces(palette: WarrenSurfaces, expected: List<Color>) {
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
