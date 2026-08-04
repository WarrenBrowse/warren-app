package com.warrenbrowse.vpn.lib.model

import arrow.optics.optics

@optics
data class ShadowsocksObfuscationSettings(val port: Constraint<Port>) {
    companion object
}
