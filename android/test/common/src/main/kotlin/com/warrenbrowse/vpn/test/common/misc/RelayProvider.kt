package com.warrenbrowse.vpn.test.common.misc

import com.warrenbrowse.vpn.test.common.constant.Production

// Warren has a single flavor, so all relays come from `Production`. The
// `currentFlavor` parameter is retained for the existing e2e test call sites.
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
