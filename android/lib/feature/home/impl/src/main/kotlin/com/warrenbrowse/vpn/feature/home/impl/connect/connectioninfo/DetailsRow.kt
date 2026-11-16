package com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo

import com.warrenbrowse.vpn.feature.home.impl.connect.ConnectionDetails

/** A row of the connection details card. */
internal sealed interface DetailsRow {
    /** QUIC, which every Warren tunnel is, and the circuit when it has two hops. */
    data class Protocol(val multihop: Boolean) : DetailsRow

    data class In(val address: String) : DetailsRow

    /** The exit socket, then every egress address known, or the exit's place when redacted. */
    data class Out(val lines: List<String>) : DetailsRow

    data object Divider : DetailsRow

    data class Reconnects(val count: Int) : DetailsRow

    /** The age of the last automatic reconnect, ticking while shown. */
    data object LastReconnect : DetailsRow

    /** The HTTP/3 mimicry, always on in a Warren tunnel. */
    data object Obfuscation : DetailsRow
}

/** The card's rows, in the mockup's order (desktop `ConnectionDetails`). */
internal fun detailsRows(details: ConnectionDetails?, autoRecoveryCount: Int): List<DetailsRow> =
    buildList {
        add(DetailsRow.Protocol(multihop = details?.multihop == true))
        add(DetailsRow.In(details?.inAddress.orEmpty()))
        add(
            DetailsRow.Out(
                listOfNotNull(
                    details?.outEndpoint,
                    details?.outIpv4Address,
                    details?.outIpv6Address,
                    details?.outFallback,
                )
            )
        )
        add(DetailsRow.Divider)
        add(DetailsRow.Reconnects(autoRecoveryCount))
        if (autoRecoveryCount > 0) add(DetailsRow.LastReconnect)
        add(DetailsRow.Obfuscation)
    }
