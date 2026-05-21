package com.warrenbrowse.vpn.feature.anticensorship.impl

import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.Port

sealed interface ObfuscationSettingItem {

    sealed interface Obfuscation : ObfuscationSettingItem {
        val selected: Boolean

        data class Automatic(override val selected: Boolean) : Obfuscation

        data class Shadowsocks(override val selected: Boolean, val port: Constraint<Port>) :
            Obfuscation

        data class UdpOverTcp(override val selected: Boolean, val port: Constraint<Port>) :
            Obfuscation

        data class Quic(override val selected: Boolean) : Obfuscation

        data class Lwo(override val selected: Boolean, val port: Constraint<Port>) : Obfuscation

        data class WireguardPort(override val selected: Boolean, val port: Constraint<Port>) :
            Obfuscation

        data class Off(override val selected: Boolean) : Obfuscation
    }

    data object Divider : ObfuscationSettingItem
}
