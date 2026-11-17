package com.warrenbrowse.vpn.app.tile

import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class TileStatusTest {
    @Test
    fun `a live tunnel reads as connected`() {
        val info =
            WarrenConnectedInfo.Connected(
                exitEndpointHost = "exit:443",
                entryEndpointHost = null,
                multiHop = false,
                daita = false,
                assignedNatPmpPort = null,
            )
        assertEquals(TileStatus.Connected, tileStatusFor(info))
    }

    @Test
    fun `a dial in flight reads as connecting, never as connected`() {
        assertEquals(TileStatus.Connecting, tileStatusFor(WarrenConnectedInfo.Connecting()))
        assertEquals(TileStatus.Connecting, tileStatusFor(WarrenConnectedInfo.Reconnecting()))
    }

    @Test
    fun `a teardown on its way back up reads as connecting`() {
        assertEquals(
            TileStatus.Connecting,
            tileStatusFor(WarrenConnectedInfo.Disconnecting(reconnecting = true)),
        )
    }

    @Test
    fun `a kill switch holding traffic reads as blocking, never as disconnected`() {
        assertEquals(
            TileStatus.Blocking,
            tileStatusFor(WarrenConnectedInfo.Blocking(reason = "dropped")),
        )
    }

    @Test
    fun `no tunnel reads as disconnected`() {
        assertEquals(TileStatus.Disconnected, tileStatusFor(WarrenConnectedInfo.Disconnected))
        assertEquals(
            TileStatus.Disconnected,
            tileStatusFor(WarrenConnectedInfo.Disconnecting(reconnecting = false)),
        )
        assertEquals(
            TileStatus.Disconnected,
            tileStatusFor(WarrenConnectedInfo.Failed(reason = "refused")),
        )
    }

    @Test
    fun `every state that wants the tunnel lights the tile, so a click there stops it`() {
        assertEquals(
            listOf(true, true, true, false),
            listOf(
                    TileStatus.Connected,
                    TileStatus.Connecting,
                    TileStatus.Blocking,
                    TileStatus.Disconnected,
                )
                .map { it.active },
        )
    }
}
