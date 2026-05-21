package com.warrenbrowse.vpn.lib.repository

import android.content.Context
import arrow.core.Either
import arrow.core.raise.either
import kotlinx.coroutines.flow.combine
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.ConnectError
import com.warrenbrowse.vpn.lib.model.DisconnectReason
import com.warrenbrowse.vpn.lib.model.GeoIpLocation
import com.warrenbrowse.vpn.lib.model.TunnelState

class ConnectionProxy(
    private val context: Context,
    private val managementService: ManagementService,
    translationRepository: RelayLocationTranslationRepository,
) {
    val tunnelState =
        combine(managementService.tunnelState, translationRepository.translations) {
            tunnelState,
            translations ->
            tunnelState.translateLocations(translations)
        }

    private fun TunnelState.translateLocations(translations: Map<String, String>): TunnelState {
        return when (this) {
            is TunnelState.Connecting -> copy(location = location?.translate(translations))
            is TunnelState.Disconnected -> copy(location = location?.translate(translations))
            is TunnelState.Disconnecting -> this
            is TunnelState.Error -> this
            is TunnelState.Connected -> copy(location = location?.translate(translations))
        }
    }

    private fun GeoIpLocation.translate(translations: Map<String, String>): GeoIpLocation =
        copy(city = translations[city] ?: city, country = translations[country] ?: country)

    suspend fun connect(): Either<ConnectError, Boolean> = either {
        context.prepareVpnSafe().mapLeft(ConnectError::NotPrepared).bind()
        managementService.connect().bind()
    }

    suspend fun connectWithoutPermissionCheck(): Either<ConnectError, Boolean> =
        managementService.connect()

    suspend fun disconnect(disconnectReason: DisconnectReason) =
        managementService.disconnect(disconnectReason)

    suspend fun reconnect() = managementService.reconnect()
}
