package com.warrenbrowse.vpn.test.common.misc

import com.warrenbrowse.vpn.test.common.constant.Production

// D.4 step 64: RelayProvider simplified — Mullvad's OSS-vs-PLAY-flavor relay
// selection (Production vs Stagemole) collapsed since Warren has a single
// flavor. The `currentFlavor` parameter is kept for back-compat with the
// existing e2e test call sites (now passes empty string).
@Suppress("UNUSED_PARAMETER")
class RelayProvider(val currentFlavor: String = "") {

    fun getDefaultRelay(): TestRelay = Production.DEFAULT_RELAY

    fun getDaitaRelay(): TestRelay = Production.DAITA_RELAY

    fun getNonDaitaRelay(): TestRelay = Production.NON_DAITA_RELAY

    fun getQuicRelay(): TestRelay = Production.QUIC_RELAY

    fun getLwoRelay(): TestRelay = Production.LWO_RELAY

    fun getOverrideRelay(): TestRelay = Production.OVERRIDE_RELAY
}

data class TestRelay(val country: String, val city: String, val relay: String)
