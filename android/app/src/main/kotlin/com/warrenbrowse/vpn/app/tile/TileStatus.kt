package com.warrenbrowse.vpn.app.tile

import com.warrenbrowse.vpn.lib.repository.WarrenConnectedInfo

/**
 * What the quick settings tile shows. [active] lights the tile in every state in which the tunnel
 * is wanted, a kill switch holding traffic included, so a click on a lit tile always means "stop".
 */
enum class TileStatus(val active: Boolean) {
    Connected(active = true),
    Connecting(active = true),
    Blocking(active = true),
    Disconnected(active = false),
}

fun tileStatusFor(info: WarrenConnectedInfo): TileStatus =
    when (info) {
        is WarrenConnectedInfo.Connected -> TileStatus.Connected
        is WarrenConnectedInfo.Dialling -> TileStatus.Connecting
        is WarrenConnectedInfo.Disconnecting ->
            if (info.reconnecting) TileStatus.Connecting else TileStatus.Disconnected
        is WarrenConnectedInfo.Blocking -> TileStatus.Blocking
        is WarrenConnectedInfo.Failed,
        WarrenConnectedInfo.Disconnected -> TileStatus.Disconnected
    }
