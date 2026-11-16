package com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo

import com.warrenbrowse.vpn.feature.home.impl.connect.ConnectionDetails
import kotlin.test.assertEquals
import org.junit.jupiter.api.Test

/**
 * The rows of the details card, in the mockup's order (desktop `ConnectionDetails`): the protocol,
 * In, Out, a hairline, the reconnection counters and the obfuscation.
 */
class DetailsRowsTest {

    private val singleHop =
        ConnectionDetails(
            multihop = false,
            inAddress = "37.27.217.153:443 UDP",
            outEndpoint = "37.27.217.153:443 UDP",
            outIpv4Address = null,
            outIpv6Address = null,
            outFallback = null,
        )

    @Test
    fun `a single hop with no reconnection reads protocol, in, out, counters and obfuscation`() {
        assertEquals(
            listOf(
                DetailsRow.Protocol(multihop = false),
                DetailsRow.In("37.27.217.153:443 UDP"),
                DetailsRow.Out(listOf("37.27.217.153:443 UDP")),
                DetailsRow.Divider,
                DetailsRow.Reconnects(0),
                DetailsRow.Obfuscation,
            ),
            detailsRows(singleHop, autoRecoveryCount = 0),
        )
    }

    @Test
    fun `a reconnection adds the age of the last one under the counter`() {
        val rows = detailsRows(singleHop, autoRecoveryCount = 2)
        val counter = rows.indexOf(DetailsRow.Reconnects(2))
        assertEquals(DetailsRow.LastReconnect, rows[counter + 1])
    }

    @Test
    fun `the out row stacks the exit socket and every egress address it knows`() {
        val rows =
            detailsRows(
                singleHop.copy(multihop = true, outIpv4Address = "1.2.3.4", outIpv6Address = "::1"),
                autoRecoveryCount = 0,
            )
        assertEquals(DetailsRow.Protocol(multihop = true), rows.first())
        assertEquals(
            DetailsRow.Out(listOf("37.27.217.153:443 UDP", "1.2.3.4", "::1")),
            rows.filterIsInstance<DetailsRow.Out>().single(),
        )
    }

    @Test
    fun `a redacted exit is named by its place`() {
        val rows =
            detailsRows(
                singleHop.copy(outEndpoint = null, outFallback = "Finland, Helsinki"),
                autoRecoveryCount = 0,
            )
        assertEquals(
            DetailsRow.Out(listOf("Finland, Helsinki")),
            rows.filterIsInstance<DetailsRow.Out>().single(),
        )
    }

    @Test
    fun `no tunnel details leaves the addresses empty but keeps the rows`() {
        val rows = detailsRows(null, autoRecoveryCount = 0)
        assertEquals(DetailsRow.In(""), rows[1])
        assertEquals(DetailsRow.Out(emptyList()), rows[2])
    }
}
