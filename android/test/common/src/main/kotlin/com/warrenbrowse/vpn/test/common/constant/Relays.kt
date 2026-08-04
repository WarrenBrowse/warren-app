package com.warrenbrowse.vpn.test.common.constant

import com.warrenbrowse.vpn.test.common.misc.TestRelay

// The single Warren flavor uses Production fixtures only.
object Production {
    val DEFAULT_RELAY = Relays.gotWg001
    val DAITA_RELAY = Relays.saoWg201
    val NON_DAITA_RELAY = Relays.tiaWg004
    val QUIC_RELAY = Relays.stoWg204
    val LWO_RELAY = Relays.stoWg204
    val OVERRIDE_RELAY = Relays.tiaWg004
}

private object Relays {
    val tiaWg004 = TestRelay(relay = "al-tia-wg-004", country = "Albania", city = "Tirana")

    val saoWg201 = TestRelay(relay = "br-sao-wg-201", country = "Brazil", city = "Sao Paulo")

    val gotWg001 = TestRelay(relay = "se-got-wg-001", country = "Sweden", city = "Gothenburg")

    val stoWg204 = TestRelay(relay = "se-sto-wg-204", country = "Sweden", city = "Stockholm")
}
