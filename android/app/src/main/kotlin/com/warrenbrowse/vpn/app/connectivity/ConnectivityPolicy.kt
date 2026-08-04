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
 * Whether a Warren relay dial can succeed on this connectivity. Relays are
 * dialed over IPv4 (mirrors the desktop `reconnects_on_connectivity` family
 * gating): an online edge carrying only IPv6, e.g. a router advertisement
 * on a link with no v4 lease, must not start a connect cycle that can only
 * fail. [Connectivity.PresumeOnline] is treated as dialable: it means the
 * platform could not resolve the real state, and refusing to dial on it
 * would strand the retry loop forever.
 */
fun Connectivity.canDialRelay(): Boolean =
    when (this) {
        is Connectivity.Online ->
            ipAvailability == IpAvailability.Ipv4 ||
                ipAvailability == IpAvailability.Ipv4AndIpv6
        Connectivity.PresumeOnline -> true
        Connectivity.Offline -> false
    }

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
