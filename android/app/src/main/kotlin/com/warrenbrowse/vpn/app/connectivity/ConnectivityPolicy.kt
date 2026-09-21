package com.warrenbrowse.vpn.app.connectivity

import com.warrenbrowse.talpid.model.Connectivity
import com.warrenbrowse.talpid.model.IpAvailability
import kotlin.time.Duration
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf

/**
 * The address families the fleet's entry hops publish, as reported by
 * `WarrenJni.directoryDialableFamilies` over the verified directory.
 *
 * It exists because "this network cannot dial a relay" is a comparison between
 * two sets, and the app used to hardcode one of them: relays were IPv4, so an
 * IPv6-only network was declared hopeless. That was true of every deployment
 * until a node bound a v6 listener, and it cost an IPv6-only mobile network
 * every connection it attempted
 * (`incidents/2026-09-20-an-ipv6-only-mobile-network-*`).
 */
@JvmInline
value class RelayFamilies(val mask: Int) {
    val hasIpv4: Boolean
        get() = mask and IPV4 != 0

    val hasIpv6: Boolean
        get() = mask and IPV6 != 0

    /** Nothing dialable was published (or the directory did not verify). */
    val isEmpty: Boolean
        get() = mask == 0

    companion object {
        const val IPV4 = 1
        const val IPV6 = 2

        /**
         * What every fleet published until 2026: IPv4 only. The default the app
         * assumes before it has read a directory, which keeps a cold start
         * behaving exactly as it did.
         */
        val V4_ONLY = RelayFamilies(IPV4)
    }
}

/**
 * Whether a Warren relay dial can succeed on this connectivity, given the
 * families [relays] actually publishes.
 *
 * An online edge that shares no family with the fleet must not start a connect
 * cycle that can only fail: on such a network `sendmsg` answers `ENETUNREACH`
 * at the first packet and every retry repeats it. An edge that shares one is
 * dialed, and which of the two addresses gets used is then the engine's
 * decision (`warrenguard_multihop::dial`), never this gate's.
 *
 * [Connectivity.PresumeOnline] is treated as dialable: it means the platform
 * could not resolve the real state, and refusing to dial on it would strand the
 * retry loop forever. A fleet that publishes nothing dialable is treated the
 * same way: the fault is not the network's, so parking on it would wait for an
 * event that cannot come.
 */
fun Connectivity.canDialRelay(relays: RelayFamilies = RelayFamilies.V4_ONLY): Boolean =
    when (this) {
        is Connectivity.Online -> {
            if (relays.isEmpty) {
                true
            } else {
                when (ipAvailability) {
                    IpAvailability.Ipv4 -> relays.hasIpv4
                    IpAvailability.Ipv6 -> relays.hasIpv6
                    IpAvailability.Ipv4AndIpv6 -> relays.hasIpv4 || relays.hasIpv6
                }
            }
        }
        Connectivity.PresumeOnline -> true
        Connectivity.Offline -> false
    }

/**
 * The device has a working network and none of its address families can carry a
 * relay dial. Distinct from [Connectivity.Offline] on purpose: the phone browses
 * normally, so telling its owner they are offline would be false, and the retry
 * loop parks with no prospect of resuming until they reach another network.
 */
fun Connectivity.isOnlineWithNoDialableFamily(
    relays: RelayFamilies = RelayFamilies.V4_ONLY
): Boolean = this is Connectivity.Online && !canDialRelay(relays)

/**
 * Hold a rising edge (false -> true) for [holdFor] before letting it
 * through; a falling edge applies immediately. Mirrors the desktop
 * `useHostOffline` debounce: routine network handovers synthesize an
 * offline blip of under a second, and rendering it would flash the
 * offline UI on every wifi to cellular switch.
 */
@OptIn(ExperimentalCoroutinesApi::class)
fun Flow<Boolean>.holdRisingEdge(holdFor: Duration): Flow<Boolean> =
    distinctUntilChanged()
        .flatMapLatest { raw ->
            if (!raw) {
                flowOf(false)
            } else {
                flow {
                    delay(holdFor)
                    emit(true)
                }
            }
        }
        .distinctUntilChanged()
