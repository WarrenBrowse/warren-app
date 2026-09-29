package com.warrenbrowse.vpn.feature.home.impl.connect.button

import androidx.compose.ui.graphics.Color
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces
import io.mockk.mockk
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The action button says what the click does (desktop `ConnectButton` and `DisconnectButton`), and
 * each tone paints from the card palette (desktop `CardButton`).
 */
class CardButtonToneTest {

    @Test
    fun `the action tone names what the click does`() {
        assertEquals(CardButtonTone.Connect, TunnelState.Disconnected().actionTone())
        assertEquals(
            CardButtonTone.Connect,
            TunnelState.Disconnecting(ActionAfterDisconnect.Nothing).actionTone(),
        )
        assertEquals(
            CardButtonTone.Cancel,
            TunnelState.Connecting(null, null, emptyList()).actionTone(),
        )
        assertEquals(
            CardButtonTone.Disconnect,
            TunnelState.Connected(mockk(relaxed = true), null, emptyList()).actionTone(),
        )
        // Turning the switch off is not an alarm.
        assertEquals(CardButtonTone.Neutral, TunnelState.Error(mockk(relaxed = true)).actionTone())
    }

    @Test
    fun `the neutral tone is a raised fill of the card, written in the card text`() {
        val s = WarrenSurfaces.Light
        assertEquals(
            CardButtonColors(
                fill = s.button,
                pressed = s.buttonPressed,
                border = s.buttonLine,
                text = s.text,
            ),
            CardButtonTone.Neutral.colors(s),
        )
    }

    @Test
    fun `the action tones are white on the fill of their action, with no border`() {
        val s = WarrenSurfaces.Dark
        assertEquals(
            CardButtonColors(s.connect, s.connectPressed, Color.Transparent, s.actionText),
            CardButtonTone.Connect.colors(s),
        )
        assertEquals(
            CardButtonColors(s.disconnect, s.disconnectPressed, Color.Transparent, s.actionText),
            CardButtonTone.Disconnect.colors(s),
        )
        assertEquals(
            CardButtonColors(s.cancel, s.cancelPressed, Color.Transparent, s.actionText),
            CardButtonTone.Cancel.colors(s),
        )
    }
}
