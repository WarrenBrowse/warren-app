package com.warrenbrowse.vpn.lib.ui.theme.tokens

import androidx.compose.ui.unit.dp
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha20
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha40
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha60
import com.warrenbrowse.vpn.lib.ui.theme.color.Alpha80
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaStatusWellBorder
import com.warrenbrowse.vpn.lib.ui.theme.color.AlphaStatusWellFill
import com.warrenbrowse.vpn.lib.ui.theme.color.ColorDarkTokens
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
        assertEquals(
            DesignTokens.ConnectionCard.PaddingVertical,
            dims.connectionCardVerticalPadding,
        )
        assertEquals(DesignTokens.ConnectionCard.PaddingHorizontal, dims.mediumPadding)
        assertEquals(DesignTokens.ConnectionCard.Radius, dims.connectionCardRadius)
        assertEquals(DesignTokens.Radius.Radius16, dims.connectionCardRadius)
        assertEquals(DesignTokens.ConnectionCard.BorderWidth, dims.thinBorderWidth)
        assertEquals(DesignTokens.ConnectionCard.BorderAlpha, Alpha20)
    }

    @Test
    fun `the feature chip keeps the desktop padding and radius`() {
        assertEquals(DesignTokens.FeatureChip.PaddingVertical, dims.chipVerticalPadding)
        assertEquals(DesignTokens.FeatureChip.PaddingHorizontal, dims.chipHorizontalPadding)
        assertEquals(DesignTokens.FeatureChip.Radius, dims.chipCornerRadius)
        assertEquals(DesignTokens.Radius.Radius8, dims.chipCornerRadius)
        assertEquals(DesignTokens.FeatureChip.BorderWidth, dims.thinBorderWidth)
        assertEquals(DesignTokens.FeatureChip.ErrorFillAlpha, Alpha40)
    }

    @Test
    fun `the badge stack gap is the desktop gap, reachable because chips carry no touch inflation`() {
        // Measured on a 1080x2400 emulator before this was fixed: the pills fell
        // exactly 48.00 dp centre to centre and 27.81 dp edge to edge, because
        // each chip sat in its own minimum-interactive row. Desktop stacks its
        // pills 5 px apart, so the stack read as six times too loose.
        //
        // The row inflation is off for the chip stack (chipInteractiveMinSize),
        // which makes the desktop gap reachable exactly. The pills stay the
        // desktop size, so a chip's own target is about 20 dp tall; WCAG 2.2 AA
        // 2.5.8 is met through its spacing clause, since a 24 dp circle centred
        // on one pill reaches no other pill's circle once the gap is 5 dp (the
        // centres fall 25.2 dp apart). The chips are 100 dp wide shortcuts and
        // every one of them is also reachable from Settings.
        assertEquals(5.dp, DesignTokens.ConnectionCard.BadgeGap)
        assertEquals(DesignTokens.ConnectionCard.BadgeGap, dims.chipStackGap)
        assertEquals(0.dp, dims.chipInteractiveMinSize)
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
    fun `the country flag is the desktop round flag`() {
        assertEquals(DesignTokens.CountryFlag.Size, dims.countryFlagSize)
        assertEquals(DesignTokens.CountryFlag.BorderWidth, dims.thinBorderWidth)
        assertEquals(DesignTokens.CountryFlag.BorderAlpha, Alpha20)
    }

    @Test
    fun `the dialog radius is the desktop radius12`() {
        assertEquals(DesignTokens.Radius.Radius12, dims.dialogCornerRadius)
    }

    @Test
    fun `the status eye sits in the desktop well`() {
        assertEquals(DesignTokens.ConnectionStatus.RowGap, dims.connectionStatusGap)
        assertEquals(DesignTokens.ConnectionStatus.WellSize, dims.connectionStatusWellSize)
        assertEquals(DesignTokens.ConnectionStatus.WellRadius, dims.connectionStatusWellRadius)
        assertEquals(DesignTokens.ConnectionStatus.IconSize, dims.connectionStatusIconSize)
        assertEquals(DesignTokens.ConnectionStatus.WellFillAlpha, AlphaStatusWellFill)
        assertEquals(DesignTokens.ConnectionStatus.WellBorderAlpha, AlphaStatusWellBorder)
        assertEquals(DesignTokens.ConnectionStatus.SubtitleAlpha, Alpha80)
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
}
