package com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo

import android.os.SystemClock
import androidx.compose.animation.AnimatedContent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.constraintlayout.compose.ConstraintLayout
import androidx.constraintlayout.compose.Dimension
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.feature.home.impl.connect.ConnectionDetails
import com.warrenbrowse.vpn.feature.home.impl.connect.marqueeLine
import com.warrenbrowse.vpn.lib.model.Endpoint
import com.warrenbrowse.vpn.lib.model.TransportProtocol
import com.warrenbrowse.vpn.lib.model.TunnelEndpoint
import com.warrenbrowse.vpn.lib.repository.WarrenAutoRecoveryProvider
import com.warrenbrowse.vpn.lib.ui.component.SPACE_CHAR
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.LOCATION_INFO_CONNECTION_IN_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.LOCATION_INFO_CONNECTION_OUT_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import kotlinx.coroutines.delay
import org.koin.compose.koinInject

@Composable
fun ConnectionDetailPanel(
    connectionDetails: ConnectionDetails?,
    enableSelectableText: Boolean = true,
    autoRecoveryCount: Int = 0,
) {

    Column {
        ConnectionInfoHeader(
            stringResource(R.string.connect_panel_connection_details),
            Modifier.fillMaxWidth().padding(bottom = Dimens.smallPadding),
        )

        // Tunnel protocol: Warren tunnels are always QUIC (bare label, desktop
        // parity). Rendered above the In/Out grid.
        LabelValueRow(
            label = null,
            value = stringResource(R.string.quic),
            modifier = Modifier.fillMaxWidth().padding(bottom = Dimens.smallPadding),
        )

        if (connectionDetails?.multihop == true) {
            LabelValueRow(
                label = null,
                value = stringResource(R.string.connection_details_multihop),
                modifier = Modifier.fillMaxWidth().padding(bottom = Dimens.smallPadding),
            )
        }

        AnimatedContent(connectionDetails, label = "ConnectionDetails") {
            ConnectionDetails(
                it?.inAddress.orEmpty(),
                it?.outEndpoint,
                it?.outIpv4Address,
                it?.outIpv6Address,
                it?.outFallback,
                modifier = Modifier.padding(bottom = Dimens.smallPadding),
                enableSelectableText = enableSelectableText,
            )
        }

        // Desktop warren-status-view rows: Reconnects is shown in every session
        // (0 included), "Last" only once one happened, and the obfuscation row
        // states the always-on HTTP/3 mimicry.
        LabelValueRow(
            label = stringResource(R.string.connection_details_reconnects),
            value = autoRecoveryCount.toString(),
            modifier = Modifier.fillMaxWidth().padding(bottom = Dimens.smallPadding),
        )
        if (autoRecoveryCount > 0) {
            LabelValueRow(
                label = stringResource(R.string.connection_details_last),
                value = lastReconnectAgeLabel(),
                modifier = Modifier.fillMaxWidth().padding(bottom = Dimens.smallPadding),
            )
        }
        LabelValueRow(
            label = stringResource(R.string.connection_details_obfuscation),
            value = stringResource(R.string.connection_details_obfuscation_active),
            modifier = Modifier.fillMaxWidth().padding(bottom = Dimens.smallPadding),
        )
    }
}

/**
 * The age of the last recovery, re-read once a second while on screen so the
 * row does not freeze on the value it had when the panel opened. The adapter
 * stamps the moment on the elapsed-realtime clock, so a suspended device does
 * not shrink the age.
 */
@Composable
private fun lastReconnectAgeLabel(): String {
    val recoveryProvider = koinInject<WarrenAutoRecoveryProvider>()
    val lastAt by recoveryProvider.lastAutoRecoveryAtMs.collectAsStateWithLifecycle()
    var now by remember { mutableLongStateOf(SystemClock.elapsedRealtime()) }
    LaunchedEffect(lastAt) {
        while (true) {
            now = SystemClock.elapsedRealtime()
            delay(AGE_TICK_MS)
        }
    }
    val at = lastAt ?: return stringResource(R.string.connection_details_never)
    return stringResource(R.string.connection_details_age, formatReconnectAge(now - at))
}

private const val AGE_TICK_MS = 1_000L

@Composable
private fun LabelValueRow(label: String?, value: String, modifier: Modifier = Modifier) {
    Row(modifier = modifier) {
        if (label != null) {
            Text(
                text = label,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.padding(end = Dimens.smallPadding),
            )
        }
        // A long IPv6 Out address does not fit the row: it scrolls its overflow
        // rather than hiding half of itself behind an ellipsis.
        Text(
            text = value,
            color = MaterialTheme.colorScheme.onPrimary,
            style = MaterialTheme.typography.bodyMedium,
            maxLines = 1,
            modifier = Modifier.marqueeLine(),
        )
    }
}

@Suppress("LongMethod", "LongParameterList", "CyclomaticComplexMethod")
@Composable
fun ConnectionDetails(
    inIPV4: String,
    outEndpoint: String?,
    outIPV4: String?,
    outIPV6: String?,
    outFallback: String?,
    modifier: Modifier = Modifier,
    enableSelectableText: Boolean = true,
) {
    // The primary "Out" line is the egress IPv4 when the daemon reports it; when
    // the exit IP is redacted (multi-hop) the exit country/city stands in so the
    // Out row is never blank (desktop `formatExitLocation` fallback).
    val outPrimaryLabel: String?
    val outPrimaryValue: String?
    when {
        outIPV4 != null -> {
            outPrimaryLabel =
                buildString {
                    append(stringResource(R.string.connection_details_out))
                    append(SPACE_CHAR)
                    append(stringResource(R.string.ipv4))
                }
            outPrimaryValue = outIPV4
        }
        // The country/city fallback is the last resort, exactly as on desktop:
        // it only speaks when no address of any kind is known.
        outFallback != null && outEndpoint == null -> {
            outPrimaryLabel = stringResource(R.string.connection_details_out)
            outPrimaryValue = outFallback
        }
        else -> {
            outPrimaryLabel = null
            outPrimaryValue = null
        }
    }

    ConstraintLayout(modifier = modifier.fillMaxWidth()) {
        val (
            inAddrHeader,
            inAddr,
            outEndpointHeader,
            outEndpointAddr,
            outAddrV4Header,
            outAddrV4,
            outAddrV6Header,
            outAddrV6,
        ) = createRefs()
        val headerBarrier =
            createEndBarrier(inAddrHeader, outEndpointHeader, outAddrV4Header, outAddrV6Header)

        val inAddrBarrier = createBottomBarrier(inAddrHeader, inAddr)
        val outEndpointBarrier =
            createBottomBarrier(inAddrHeader, inAddr, outEndpointHeader, outEndpointAddr)
        val outAddrV4Barrier =
            createBottomBarrier(
                inAddrHeader,
                inAddr,
                outEndpointHeader,
                outEndpointAddr,
                outAddrV4Header,
                outAddrV4,
            )

        val outAddrV6Barrier =
            createBottomBarrier(
                inAddrHeader,
                inAddr,
                outEndpointHeader,
                outEndpointAddr,
                outAddrV4Header,
                outAddrV4,
                outAddrV6Header,
                outAddrV6,
            )

        Text(
            text = stringResource(R.string.connection_details_in),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            style = MaterialTheme.typography.bodyMedium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier =
                Modifier.padding(end = Dimens.smallPadding).constrainAs(inAddrHeader) {
                    start.linkTo(parent.start)
                    top.linkTo(parent.top)
                    bottom.linkTo(inAddrBarrier)
                    height = Dimension.wrapContent
                    width = Dimension.wrapContent
                },
        )
        SelectionContainer(
            modifier =
                Modifier.constrainAs(inAddr) {
                    start.linkTo(headerBarrier)
                    end.linkTo(parent.end)
                    top.linkTo(parent.top)
                    bottom.linkTo(inAddrBarrier)
                    height = Dimension.wrapContent
                    width = Dimension.fillToConstraints
                }
        ) {
            Text(
                text = inIPV4,
                color = MaterialTheme.colorScheme.onPrimary,
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.testTag(LOCATION_INFO_CONNECTION_IN_TEST_TAG),
            )
        }

        // The exit socket the tunnel egresses from, ahead of every geoip line:
        // Warren skips the conncheck, so this is usually the only address the
        // Out side can honestly state.
        if (outEndpoint != null) {
            Text(
                text = stringResource(R.string.connection_details_out),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier =
                    Modifier.padding(end = Dimens.smallPadding).constrainAs(outEndpointHeader) {
                        start.linkTo(parent.start)
                        top.linkTo(inAddrBarrier)
                        bottom.linkTo(outEndpointBarrier)
                        height = Dimension.wrapContent
                        width = Dimension.wrapContent
                    },
            )
            Box(
                modifier =
                    Modifier.constrainAs(outEndpointAddr) {
                        start.linkTo(headerBarrier)
                        end.linkTo(parent.end)
                        top.linkTo(inAddrBarrier)
                        bottom.linkTo(outEndpointBarrier)
                        height = Dimension.wrapContent
                        width = Dimension.fillToConstraints
                    }
            ) {
                val outEndpointText =
                    @Composable {
                        Text(
                            modifier = Modifier.testTag(LOCATION_INFO_CONNECTION_OUT_TEST_TAG),
                            text = outEndpoint,
                            color = MaterialTheme.colorScheme.onPrimary,
                            style = MaterialTheme.typography.bodyMedium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                if (enableSelectableText) {
                    SelectionContainer(content = outEndpointText)
                } else {
                    outEndpointText()
                }
            }
        }

        if (outPrimaryLabel != null && outPrimaryValue != null) {
            Text(
                text = outPrimaryLabel,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier =
                    Modifier.padding(end = Dimens.smallPadding).constrainAs(outAddrV4Header) {
                        start.linkTo(parent.start)
                        top.linkTo(outEndpointBarrier)
                        bottom.linkTo(outAddrV4Barrier)
                        height = Dimension.wrapContent
                        width = Dimension.wrapContent
                    },
            )
            Box(
                modifier =
                    Modifier.constrainAs(outAddrV4) {
                        start.linkTo(headerBarrier)
                        end.linkTo(parent.end)
                        top.linkTo(outEndpointBarrier)
                        bottom.linkTo(outAddrV4Barrier)
                        height = Dimension.wrapContent
                        width = Dimension.fillToConstraints
                    }
            ) {
                val outPrimaryText =
                    @Composable {
                        Text(
                            // The tag marks the first Out line, which the exit
                            // endpoint takes over when it is present.
                            modifier =
                                if (outEndpoint == null) {
                                    Modifier.testTag(LOCATION_INFO_CONNECTION_OUT_TEST_TAG)
                                } else {
                                    Modifier
                                },
                            text = outPrimaryValue,
                            color = MaterialTheme.colorScheme.onPrimary,
                            style = MaterialTheme.typography.bodyMedium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                if (enableSelectableText) {
                    SelectionContainer(content = outPrimaryText)
                } else {
                    outPrimaryText()
                }
            }
        }

        if (outIPV6 != null) {
            Text(
                text =
                    buildString {
                        append(stringResource(R.string.connection_details_out))
                        append(SPACE_CHAR)
                        append(stringResource(R.string.ipv6))
                    },
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier =
                    Modifier.padding(end = Dimens.smallPadding).constrainAs(outAddrV6Header) {
                        start.linkTo(parent.start)
                        top.linkTo(outAddrV4Barrier)
                        bottom.linkTo(outAddrV6Barrier)
                        height = Dimension.wrapContent
                        width = Dimension.wrapContent
                    },
            )
            Box(
                modifier =
                    Modifier.constrainAs(outAddrV6) {
                        start.linkTo(headerBarrier)
                        end.linkTo(parent.end)
                        top.linkTo(outAddrV4Barrier)
                        bottom.linkTo(outAddrV6Barrier)
                        height = Dimension.wrapContent
                        width = Dimension.fillToConstraints
                    }
            ) {
                val outIpV6Text =
                    @Composable {
                        Text(
                            text = outIPV6,
                            color = MaterialTheme.colorScheme.onPrimary,
                            style = MaterialTheme.typography.bodyMedium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                if (enableSelectableText) {
                    SelectionContainer(content = outIpV6Text)
                } else {
                    outIpV6Text()
                }
            }
        }
    }
}

@Composable
fun TunnelEndpoint.toInAddress(): String {
    // Order is important
    // First we check for obfuscation (Shadowsocks, UDP-Over-UDP)
    // Then we check for entry if we have multihop
    // Finally we check for exit endpoint
    val relayEndpoint = obfuscation?.endpoint ?: entryEndpoint ?: endpoint
    return relayEndpoint.toOutAddress()
}

/** An endpoint as "host:port PROTO", the shape both the In and Out rows use. */
@Composable
fun Endpoint.toOutAddress(): String {
    val host = address.address?.hostAddress ?: address.hostString ?: ""
    return buildString {
        append(host)
        append(":")
        append(address.port)
        append(" ")
        append(
            when (protocol) {
                TransportProtocol.Tcp -> stringResource(id = R.string.tcp)
                TransportProtocol.Udp -> stringResource(id = R.string.udp)
            }
        )
    }
}

/**
 * The engine's "no address to give" sentinel, which must never be printed at
 * the user (desktop `isUnspecifiedSocketAddress`).
 */
fun Endpoint.isUnspecified(): Boolean =
    address.address?.isAnyLocalAddress == true || address.port == 0
