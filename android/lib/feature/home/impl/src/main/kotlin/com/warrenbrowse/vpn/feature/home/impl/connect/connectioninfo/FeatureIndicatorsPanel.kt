@file:OptIn(ExperimentalSharedTransitionApi::class)

package com.warrenbrowse.vpn.feature.home.impl.connect.connectioninfo

import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.core.EaseInQuart
import androidx.compose.animation.core.EaseOutQuad
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.warrenbrowse.vpn.common.compose.LocalNavAnimatedVisibilityScope
import com.warrenbrowse.vpn.common.compose.LocalSharedTransitionScope
import com.warrenbrowse.vpn.lib.model.FeatureIndicator
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenNatPmpStatusProvider
import com.warrenbrowse.vpn.lib.ui.designsystem.WarrenFeatureChip
import com.warrenbrowse.vpn.lib.ui.resource.R
import com.warrenbrowse.vpn.lib.ui.theme.Dimens
import org.koin.compose.koinInject

/**
 * The active-feature chips floating above the connection card.
 *
 * Every chip is always shown: the card sits over the scenery with the whole
 * screen width available, so there is nothing to collapse behind a "+N more"
 * affordance. The caller supplies the engine's indicators; the live values that
 * make a chip informative (the MTU in force, the forwarded port, whether the
 * exit actually granted DAITA) live in settings and in the NAT-PMP status, so
 * they are read here rather than threaded through the whole connect state.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AlwaysExpandedFeatureIndicators(
    features: List<FeatureIndicator>,
    onNavigateToFeature: (FeatureIndicator) -> Unit,
) {
    val settings = koinInject<WarrenLocalSettingsRepository>()
    val natPmpProvider = koinInject<WarrenNatPmpStatusProvider>()

    val mtu by settings.tunnelMtu.collectAsStateWithLifecycle()
    val daitaWanted by settings.daitaEnabled.collectAsStateWithLifecycle()
    val natPmpEnabled by settings.natPmpEnabled.collectAsStateWithLifecycle()
    val natPmpStatus by natPmpProvider.natPmpStatus.collectAsStateWithLifecycle()

    val chips =
        featureChips(
            features = features,
            mtu = mtu,
            daitaWanted = daitaWanted,
            natPmpEnabled = natPmpEnabled,
            natPmpStatus = natPmpStatus,
        )

    FlowRow(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(Dimens.smallPadding),
    ) {
        chips.forEach { chip ->
            val sharedTransitionScope = LocalSharedTransitionScope.current
            val animatedVisibilityScope = LocalNavAnimatedVisibilityScope.current

            with(sharedTransitionScope) {
                WarrenFeatureChip(
                    text = chip.label,
                    onClick = { onNavigateToFeature(chip.indicator) },
                    modifier =
                        if (this@with != null && animatedVisibilityScope != null) {
                            Modifier.sharedBounds(
                                rememberSharedContentState(
                                    key =
                                        if (chip.indicator == FeatureIndicator.DAITA_MULTIHOP) {
                                            FeatureIndicator.DAITA
                                        } else {
                                            chip.indicator
                                        }
                                ),
                                animatedVisibilityScope = animatedVisibilityScope,
                                // This flag should be set to `true` (default), which would allow
                                // the element to animate above all other views. However, it makes
                                // the expand/collapse animation janky.
                                renderInOverlayDuringTransition = false,
                                enter = fadeIn(tween(easing = EaseInQuart)),
                                exit = fadeOut(tween(easing = EaseOutQuad)),
                            )
                        } else {
                            Modifier
                        },
                )
            }
        }
    }
}

/** One rendered chip: the feature it navigates to, and the text it carries. */
private data class FeatureChip(val indicator: FeatureIndicator, val label: String)

/**
 * The engine's indicators plus the two the engine cannot report on its own: a
 * reduced MTU (a client-side setting) and DAITA asked for but not granted by
 * this exit, which is exactly the case a user needs to be told about.
 */
@Composable
private fun featureChips(
    features: List<FeatureIndicator>,
    mtu: Int,
    daitaWanted: Boolean,
    natPmpEnabled: Boolean,
    natPmpStatus: String,
): List<FeatureChip> {
    val chips = mutableListOf<FeatureChip>()

    features.forEach { indicator ->
        chips.add(FeatureChip(indicator, indicator.label(natPmpStatus)))
    }

    if (mtu < WarrenLocalSettingsRepository.MTU_MAX) {
        chips.add(
            FeatureChip(
                FeatureIndicator.CUSTOM_MTU,
                stringResource(R.string.feature_reduced_mtu_value, mtu.toString()),
            )
        )
    }

    // The setting is on and the engine reports no DAITA on this session, so the
    // protection the user enabled is not running.
    if (daitaWanted &&
        FeatureIndicator.DAITA !in features &&
        FeatureIndicator.DAITA_MULTIHOP !in features
    ) {
        chips.add(
            FeatureChip(
                FeatureIndicator.DAITA,
                stringResource(
                    R.string.feature_daita_not_active_on_server,
                    stringResource(R.string.daita),
                ),
            )
        )
    }

    // The engine only raises the port-forwarding chip once a mapping exists, so
    // a request the exit refused would otherwise be invisible on this screen.
    if (natPmpEnabled && FeatureIndicator.PORT_FORWARDING !in features) {
        natPmpFailureLabel(natPmpStatus)?.let { label ->
            chips.add(FeatureChip(FeatureIndicator.PORT_FORWARDING, label))
        }
    }

    return chips.sortedBy { it.indicator.ordinal }
}

@Suppress("CyclomaticComplexMethod")
@Composable
private fun FeatureIndicator.label(natPmpStatus: String): String {
    val resource =
        when (this) {
            FeatureIndicator.QUANTUM_RESISTANCE -> R.string.feature_quantum_resistant
            FeatureIndicator.SPLIT_TUNNELING -> R.string.split_tunneling
            FeatureIndicator.SHADOWSOCKS -> R.string.shadowsocks
            FeatureIndicator.UDP_2_TCP -> R.string.udp_over_tcp
            FeatureIndicator.QUIC -> R.string.quic
            FeatureIndicator.LWO -> R.string.lwo
            FeatureIndicator.LAN_SHARING -> R.string.local_network_sharing
            FeatureIndicator.DNS_CONTENT_BLOCKERS -> R.string.dns_content_blockers
            FeatureIndicator.CUSTOM_DNS -> R.string.feature_custom_dns
            FeatureIndicator.SERVER_IP_OVERRIDE -> R.string.server_ip_override
            FeatureIndicator.CUSTOM_MTU -> R.string.feature_reduced_mtu
            FeatureIndicator.PORT_FORWARDING ->
                return portForwardingLabel(natPmpStatus)
            FeatureIndicator.DAITA -> R.string.daita
            FeatureIndicator.DAITA_MULTIHOP ->
                return stringResource(R.string.daita_multihop, stringResource(R.string.daita))
            FeatureIndicator.MULTIHOP -> R.string.multihop
        }
    return stringResource(resource)
}

/** The chip names the forwarded port, which is the thing the user needs. */
@Composable
private fun portForwardingLabel(natPmpStatus: String): String {
    val port = natPmpJsonField(natPmpStatus, "external_port")
    return if (port != null) {
        stringResource(R.string.feature_port_forwarding_value, port)
    } else {
        natPmpFailureLabel(natPmpStatus) ?: stringResource(R.string.feature_port_forwarding)
    }
}

/**
 * The label for a port-forwarding request that did not produce a mapping, or
 * null while the request is merely in flight (nothing to report yet).
 */
@Composable
private fun natPmpFailureLabel(natPmpStatus: String): String? =
    when {
        natPmpJsonField(natPmpStatus, "state") != "failed" -> null
        natPmpJsonField(natPmpStatus, "reason") == "SuggestedPortInUse" ->
            stringResource(R.string.feature_port_forwarding_in_use)
        else -> stringResource(R.string.feature_port_forwarding_blocked)
    }

/**
 * Flat JSON string/number value by key, unquoted, or null. The NAT-PMP status
 * crosses the JNI bridge as a flat JSON object, and this module carries no JSON
 * parser for one field.
 */
private fun natPmpJsonField(json: String, key: String): String? {
    val marker = "\"$key\""
    val keyAt = json.indexOf(marker)
    if (keyAt < 0) return null
    val colon = json.indexOf(':', keyAt + marker.length)
    if (colon < 0) return null
    var i = colon + 1
    while (i < json.length && json[i].isWhitespace()) i++
    if (i >= json.length) return null
    return if (json[i] == '"') {
        val end = json.indexOf('"', i + 1)
        if (end < 0) null else json.substring(i + 1, end)
    } else {
        val end = json.indexOfFirst(i) { it == ',' || it == '}' }
        val raw = json.substring(i, if (end < 0) json.length else end).trim()
        raw.ifEmpty { null }
    }
}

private inline fun String.indexOfFirst(from: Int, predicate: (Char) -> Boolean): Int {
    for (i in from until length) {
        if (predicate(this[i])) return i
    }
    return -1
}
