package com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo

import android.os.SystemClock
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.repeatOnLifecycle
import com.warrenbrowse.vpn.feature.home.impl.connect.ConnectionDetails
import com.warrenbrowse.vpn.lib.model.Endpoint
import com.warrenbrowse.vpn.lib.model.TransportProtocol
import com.warrenbrowse.vpn.lib.model.TunnelEndpoint
import com.warrenbrowse.vpn.lib.repository.WarrenAutoRecoveryProvider
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.tag.LOCATION_INFO_CONNECTION_IN_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.LOCATION_INFO_CONNECTION_OUT_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.theme.HomeTypography
import com.warrenbrowse.vpn.lib.ui.theme.color.LocalWarrenSurfaces
import com.warrenbrowse.vpn.lib.ui.theme.tokens.DesignTokens
import kotlinx.coroutines.delay
import org.koin.compose.koinInject

/**
 * The mockup's details card, opened in place under the exit row: a two-column grid of label and
 * value on the details surface with its inset line, the addresses in the mono face, the
 * reconnection counters under a hairline.
 */
@Composable
internal fun HomeDetailsCard(
    connectionDetails: ConnectionDetails?,
    autoRecoveryCount: Int,
    modifier: Modifier = Modifier,
    enableSelectableText: Boolean = true,
) {
    val details = DesignTokens.Home.Details
    val surfaces = LocalWarrenSurfaces.current
    val shape = RoundedCornerShape(details.Radius)
    val rows = detailsRows(connectionDetails, autoRecoveryCount)
    val cells = rows.filter { it != DetailsRow.Divider }
    Box(
        modifier
            .fillMaxWidth()
            .clip(shape)
            .background(surfaces.homeDetails)
            .border(1.dp, surfaces.homeDetailsLine, shape)
            .padding(horizontal = details.PaddingHorizontal, vertical = details.PaddingVertical)
            .testTag(HOME_DETAILS_CARD_TEST_TAG)
    ) {
        DetailsGrid(
            labels = { cells.forEach { DetailsLabel(it.label()) } },
            values = { cells.forEach { DetailsValue(it, enableSelectableText) } },
            divider = {
                Box(
                    Modifier.fillMaxWidth()
                        .padding(vertical = details.DividerMargin)
                        .height(1.dp)
                        .background(surfaces.homeDetailsLine)
                )
            },
            dividerAfter = rows.indexOf(DetailsRow.Divider),
        )
    }
}

@Composable
private fun DetailsRow.label(): String =
    when (this) {
        is DetailsRow.Protocol -> stringResource(R.string.tunnel_protocol_label)
        is DetailsRow.In -> stringResource(R.string.connection_details_in)
        is DetailsRow.Out -> stringResource(R.string.connection_details_out)
        is DetailsRow.Reconnects -> stringResource(R.string.connection_details_reconnects)
        DetailsRow.LastReconnect -> stringResource(R.string.connection_details_last)
        DetailsRow.Obfuscation -> stringResource(R.string.connection_details_obfuscation)
        DetailsRow.Divider -> ""
    }

@Composable
private fun DetailsLabel(text: String) {
    Text(
        text = text,
        style = HomeTypography.detailsLabel,
        color = LocalWarrenSurfaces.current.homeTextSecondary,
        maxLines = 1,
    )
}

@Composable
private fun DetailsValue(row: DetailsRow, selectable: Boolean) {
    when (row) {
        is DetailsRow.Protocol ->
            Column(verticalArrangement = Arrangement.spacedBy(VALUE_LINE_GAP)) {
                ValueText(stringResource(R.string.quic), HomeTypography.detailsValueStrong)
                if (row.multihop) {
                    ValueText(
                        stringResource(R.string.connection_details_multihop),
                        HomeTypography.detailsValueStrong,
                    )
                }
            }
        is DetailsRow.In ->
            Addresses(
                listOf(row.address),
                selectable,
                Modifier.testTag(LOCATION_INFO_CONNECTION_IN_TEST_TAG),
            )
        is DetailsRow.Out ->
            Addresses(
                row.lines,
                selectable,
                Modifier.testTag(LOCATION_INFO_CONNECTION_OUT_TEST_TAG),
            )
        is DetailsRow.Reconnects -> ValueText(row.count.toString(), HomeTypography.detailsValue)
        DetailsRow.LastReconnect -> LastReconnectValue()
        DetailsRow.Obfuscation -> ObfuscationValue()
        DetailsRow.Divider -> Unit
    }
}

@Composable
private fun ValueText(text: String, style: TextStyle, modifier: Modifier = Modifier) {
    Text(
        text = text,
        style = style,
        color = LocalWarrenSurfaces.current.homeText,
        modifier = modifier,
    )
}

/** Addresses wrap rather than hide behind an ellipsis: a long IPv6 egress must read whole. */
@Composable
private fun Addresses(lines: List<String>, selectable: Boolean, modifier: Modifier = Modifier) {
    val content =
        @Composable {
            Column(modifier, verticalArrangement = Arrangement.spacedBy(VALUE_LINE_GAP)) {
                lines.forEach { ValueText(it, HomeTypography.detailsMono) }
            }
        }
    if (selectable) SelectionContainer(content = content) else content()
}

/** The obfuscation's name and the small "active" chip after it. */
@Composable
private fun ObfuscationValue() {
    val details = DesignTokens.Home.Details
    val surfaces = LocalWarrenSurfaces.current
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(CHIP_GAP),
    ) {
        ValueText(
            stringResource(R.string.connection_details_obfuscation_name),
            HomeTypography.detailsValue,
            Modifier.weight(1f, fill = false),
        )
        Text(
            text = stringResource(R.string.connection_details_obfuscation_chip),
            style = HomeTypography.detailsChip,
            color = surfaces.homeActive,
            maxLines = 1,
            modifier =
                Modifier.clip(CircleShape)
                    .background(surfaces.homeActiveWell)
                    .padding(
                        horizontal = details.ChipPaddingHorizontal,
                        vertical = details.ChipPaddingVertical,
                    ),
        )
    }
}

/**
 * The age of the last recovery, re-read once a second while the screen is started so it does not
 * freeze on the value it had when the card opened, and not at all while it is stopped. The tick is
 * read here and nowhere above, so each second recomposes this value and not the card around it.
 * The adapter stamps the moment on the elapsed-realtime clock, so a suspended device does not
 * shrink the age.
 */
@Composable
private fun LastReconnectValue() {
    val recoveryProvider = koinInject<WarrenAutoRecoveryProvider>()
    val lastAt by recoveryProvider.lastAutoRecoveryAtMs.collectAsStateWithLifecycle()
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val now by
        produceState(SystemClock.elapsedRealtime(), lifecycle) {
            lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
                while (true) {
                    value = SystemClock.elapsedRealtime()
                    delay(AGE_TICK_MS)
                }
            }
        }
    val value =
        lastAt?.let { stringResource(R.string.connection_details_age, formatReconnectAge(now - it)) }
            ?: stringResource(R.string.connection_details_never)
    ValueText(value, HomeTypography.detailsValue)
}

/**
 * The desktop's `auto 1fr` grid: the label column as wide as its widest label, the values in the
 * rest, [DesignTokens.Home.Details.RowGap] between rows and the hairline as a full-width row after
 * row [dividerAfter].
 */
@Composable
private fun DetailsGrid(
    labels: @Composable () -> Unit,
    values: @Composable () -> Unit,
    divider: @Composable () -> Unit,
    dividerAfter: Int,
) {
    val details = DesignTokens.Home.Details
    Layout(contents = listOf(labels, values, divider)) { (labelSlot, valueSlot, dividerSlot), c ->
        val width = c.maxWidth
        val columnGap = details.ColumnGap.roundToPx()
        val rowGap = details.RowGap.roundToPx()
        val labelPlaceables = labelSlot.map { it.measure(Constraints(maxWidth = width / 2)) }
        val labelWidth = labelPlaceables.maxOfOrNull { it.width } ?: 0
        val valueWidth = (width - labelWidth - columnGap).coerceAtLeast(0)
        val valuePlaceables = valueSlot.map { it.measure(Constraints(maxWidth = valueWidth)) }
        val dividerPlaceable =
            dividerSlot.firstOrNull()?.measure(Constraints(minWidth = width, maxWidth = width))
        val rowHeights = labelPlaceables.zip(valuePlaceables) { l, v -> maxOf(l.height, v.height) }
        val tops = IntArray(rowHeights.size)
        var y = 0
        var dividerTop = -1
        rowHeights.forEachIndexed { index, height ->
            if (index > 0) y += rowGap
            if (index == dividerAfter && dividerPlaceable != null) {
                dividerTop = y
                y += dividerPlaceable.height + rowGap
            }
            tops[index] = y
            y += height
        }
        layout(width, y) {
            labelPlaceables.forEachIndexed { i, p -> p.place(0, tops[i]) }
            valuePlaceables.forEachIndexed { i, p -> p.place(labelWidth + columnGap, tops[i]) }
            if (dividerTop >= 0) dividerPlaceable?.place(0, dividerTop)
        }
    }
}

private val VALUE_LINE_GAP = 2.dp
private val CHIP_GAP = 6.dp
private const val AGE_TICK_MS = 1_000L

internal const val HOME_DETAILS_CARD_TEST_TAG = "home_details_card_test_tag"

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
