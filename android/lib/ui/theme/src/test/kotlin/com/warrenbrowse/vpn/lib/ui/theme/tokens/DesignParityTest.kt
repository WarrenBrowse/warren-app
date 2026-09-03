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
 * The connect-screen primitives Android draws with, enumerated against the
 * generated desktop tokens: a "let me just bump this to 16" edit fails here on
 * the platform that moved. Where Android deliberately deviates, the deviation
 * is pinned next to the desktop value it deviates from.
 */
class DesignParityTest {

    private val dims = defaultDimensions

    @Test
    fun `the connection card keeps the desktop geometry`() {
        assertEquals(DesignTokens.ConnectionCard.PaddingVertical, dims.connectionCardVerticalPadding)
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
    fun `the badge stack gap is the documented touch-row deviation`() {
        // Desktop stacks 22 px pills 5 px apart for a pointer. Every Android
        // chip sits in its own 48 dp touch row, so the pills already fall 26 dp
        // apart at a zero row gap; the desktop value is unreachable without
        // shrinking the touch boxes under the platform floor.
        assertEquals(5.dp, DesignTokens.ConnectionCard.BadgeGap)
        assertEquals(0.dp, dims.chipStackGap)
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
        assertEquals(DesignTokens.NotificationBanner.PaddingVertical, dims.notificationBannerVerticalPadding)
        assertEquals(DesignTokens.NotificationBanner.PaddingStart, dims.notificationBannerStartPadding)
        assertEquals(DesignTokens.NotificationBanner.PaddingEnd, dims.notificationBannerEndPadding)
        assertEquals(DesignTokens.NotificationBanner.Elevation, dims.notificationBannerElevation)
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
