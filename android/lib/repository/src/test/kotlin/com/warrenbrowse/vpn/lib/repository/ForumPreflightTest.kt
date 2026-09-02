package com.warrenbrowse.vpn.lib.repository

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Test

class ForumPreflightTest {

    @Test
    fun a_settled_tunnel_lets_a_forum_request_leave() {
        assertEquals(ForumPreflight.Proceed, ForumPreflight.of(WarrenConnectedInfo.Disconnected))
        assertEquals(
            ForumPreflight.Proceed,
            ForumPreflight.of(
                WarrenConnectedInfo.Connected(
                    exitEndpointHost = "203.0.113.7:443",
                    entryEndpointHost = null,
                    multiHop = false,
                    daita = false,
                    assignedNatPmpPort = null,
                )
            ),
        )
    }

    @Test
    fun a_tunnel_between_states_defers_with_its_class() {
        // While the TUN comes up the resolver points at the tunnel DNS and
        // times out; while the kill switch holds there is no DNS server at
        // all. Both leave the forum POST hanging or failing on the host name.
        assertEquals(ForumPreflight.Defer("connecting"), ForumPreflight.of(WarrenConnectedInfo.Connecting()))
        assertEquals(
            ForumPreflight.Defer("reconnecting"),
            ForumPreflight.of(WarrenConnectedInfo.Reconnecting()),
        )
        assertEquals(
            ForumPreflight.Defer("disconnecting"),
            ForumPreflight.of(WarrenConnectedInfo.Disconnecting(reconnecting = false)),
        )
        assertEquals(
            ForumPreflight.Defer("reconnecting"),
            ForumPreflight.of(WarrenConnectedInfo.Disconnecting(reconnecting = true)),
        )
        assertEquals(ForumPreflight.Defer("failed"), ForumPreflight.of(WarrenConnectedInfo.Failed("x")))
        assertEquals(ForumPreflight.Defer("blocking"), ForumPreflight.of(WarrenConnectedInfo.Blocking("x")))
    }

    @Test
    fun the_deferral_class_never_carries_the_endpoint_or_the_reason() {
        val verdict =
            ForumPreflight.of(
                WarrenConnectedInfo.Connecting(exitEndpointHost = "203.0.113.7:443", multiHop = true)
            ) as ForumPreflight.Defer
        assertFalse(verdict.tunnelClass.contains("203"))
        val blocked =
            ForumPreflight.of(WarrenConnectedInfo.Blocking("exit 203.0.113.7 refused")) as ForumPreflight.Defer
        assertEquals("blocking", blocked.tunnelClass)
    }
}
