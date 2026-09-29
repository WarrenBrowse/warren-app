package com.warrenbrowse.vpn.lib.ui.theme.color

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens

/**
 * The opaque surfaces the connect screen paints over the scenery (the card, the chips above it,
 * the beta banner), in the palette of the resolved theme: desktop `surface-tokens.ts`. The desktop
 * hover shades are left out, a finger has no hover.
 */
@Immutable
data class WarrenSurfaces(
    val card: Color,
    val line: Color,
    val shadowSoft: Color,
    val shadowStrong: Color,
    val pill: Color,
    val pillText: Color,
    val text: Color,
    val textSecondary: Color,
    val textMuted: Color,
    val button: Color,
    val buttonPressed: Color,
    val buttonLine: Color,
    val exposed: Color,
    val exposedWell: Color,
    val connecting: Color,
    val connectingWell: Color,
    val protected: Color,
    val protectedWell: Color,
    val connect: Color,
    val connectPressed: Color,
    val disconnect: Color,
    val disconnectPressed: Color,
    val cancel: Color,
    val cancelPressed: Color,
    val actionText: Color,
) {
    companion object {
        val Dark =
            with(DesignTokens.Surfaces.Dark) {
                WarrenSurfaces(
                    card = Card,
                    line = Line,
                    shadowSoft = ShadowSoft,
                    shadowStrong = ShadowStrong,
                    pill = Pill,
                    pillText = PillText,
                    text = Text,
                    textSecondary = TextSecondary,
                    textMuted = TextMuted,
                    button = Button,
                    buttonPressed = ButtonPressed,
                    buttonLine = ButtonLine,
                    exposed = Exposed,
                    exposedWell = ExposedWell,
                    connecting = Connecting,
                    connectingWell = ConnectingWell,
                    protected = Protected,
                    protectedWell = ProtectedWell,
                    connect = Connect,
                    connectPressed = ConnectPressed,
                    disconnect = Disconnect,
                    disconnectPressed = DisconnectPressed,
                    cancel = Cancel,
                    cancelPressed = CancelPressed,
                    actionText = ActionText,
                )
            }

        val Light =
            with(DesignTokens.Surfaces.Light) {
                WarrenSurfaces(
                    card = Card,
                    line = Line,
                    shadowSoft = ShadowSoft,
                    shadowStrong = ShadowStrong,
                    pill = Pill,
                    pillText = PillText,
                    text = Text,
                    textSecondary = TextSecondary,
                    textMuted = TextMuted,
                    button = Button,
                    buttonPressed = ButtonPressed,
                    buttonLine = ButtonLine,
                    exposed = Exposed,
                    exposedWell = ExposedWell,
                    connecting = Connecting,
                    connectingWell = ConnectingWell,
                    protected = Protected,
                    protectedWell = ProtectedWell,
                    connect = Connect,
                    connectPressed = ConnectPressed,
                    disconnect = Disconnect,
                    disconnectPressed = DisconnectPressed,
                    cancel = Cancel,
                    cancelPressed = CancelPressed,
                    actionText = ActionText,
                )
            }
    }
}

/** The surfaces of the resolved theme; dark, the house palette, until a theme is provided. */
val LocalWarrenSurfaces = staticCompositionLocalOf { WarrenSurfaces.Dark }
