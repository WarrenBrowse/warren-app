package com.warrenbrowse.vpn.feature.home.impl.connect.button

import androidx.compose.ui.graphics.Color
import com.warrenbrowse.vpn.lib.model.ActionAfterDisconnect
import com.warrenbrowse.vpn.lib.model.TunnelState
import com.warrenbrowse.vpn.lib.ui.theme.color.WarrenSurfaces
import io.mockk.mockk
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The action button says what the click does (desktop `HomeControls` ActionButton): a solid go, an
 * outlined stop, and a neutral field to turn the kill switch off.
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
    fun `the neutral tone is the home field, written in the home text`() {
        val s = WarrenSurfaces.Light
        assertEquals(
            CardButtonColors(
                fill = s.homeField,
                pressed = s.homeFieldPressed,
                border = Color.Transparent,
                text = s.homeText,
            ),
            CardButtonTone.Neutral.colors(s),
        )
    }

    @Test
    fun `connect is a solid fill`() {
        val s = WarrenSurfaces.Dark
        assertEquals(
            CardButtonColors(s.homeConnect, s.homeConnectPressed, Color.Transparent, s.homeConnectText),
            CardButtonTone.Connect.colors(s),
        )
    }

    @Test
    fun `disconnect and cancel are outlined, pressed with their hover wash`() {
        val s = WarrenSurfaces.Dark
        assertEquals(
            CardButtonColors(
                Color.Transparent,
                s.homeDisconnectHover,
                s.homeDisconnectLine,
                s.homeDisconnectText,
            ),
            CardButtonTone.Disconnect.colors(s),
        )
        assertEquals(
            CardButtonColors(Color.Transparent, s.homeCancelHover, s.homeCancelLine, s.homeCancelText),
            CardButtonTone.Cancel.colors(s),
        )
    }
}
