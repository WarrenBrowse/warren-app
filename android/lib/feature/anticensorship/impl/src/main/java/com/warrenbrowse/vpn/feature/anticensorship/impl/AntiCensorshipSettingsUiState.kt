package com.warrenbrowse.vpn.feature.anticensorship.impl

import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.ObfuscationMode
import com.warrenbrowse.vpn.lib.model.Port

data class AntiCensorshipSettingsUiState(
    val isModal: Boolean,
    val items: List<ObfuscationSettingItem>,
) {
    companion object {
        fun from(
            isModal: Boolean,
            obfuscationMode: ObfuscationMode,
            selectedUdp2TcpObfuscationPort: Constraint<Port>,
            selectedShadowsocksObfuscationPort: Constraint<Port>,
            selectedWireguardPort: Constraint<Port>,
        ): AntiCensorshipSettingsUiState =
            AntiCensorshipSettingsUiState(
                isModal = isModal,
                items =
                    buildList {
                        add(
                            ObfuscationSettingItem.Obfuscation.Automatic(
                                obfuscationMode == ObfuscationMode.Auto
                            )
                        )
                        add(ObfuscationSettingItem.Divider)
                        add(
                            ObfuscationSettingItem.Obfuscation.WireguardPort(
                                obfuscationMode == ObfuscationMode.WireguardPort,
                                selectedWireguardPort,
                            )
                        )
                        add(ObfuscationSettingItem.Divider)
                        add(
                            ObfuscationSettingItem.Obfuscation.Lwo(
                                obfuscationMode == ObfuscationMode.Lwo,
                                selectedWireguardPort,
                            )
                        )
                        add(ObfuscationSettingItem.Divider)
                        add(
                            ObfuscationSettingItem.Obfuscation.Quic(
                                obfuscationMode == ObfuscationMode.Quic
                            )
                        )
                        add(ObfuscationSettingItem.Divider)
                        add(
                            ObfuscationSettingItem.Obfuscation.Shadowsocks(
                                obfuscationMode == ObfuscationMode.Shadowsocks,
                                selectedShadowsocksObfuscationPort,
                            )
                        )
                        add(ObfuscationSettingItem.Divider)
                        add(
                            ObfuscationSettingItem.Obfuscation.UdpOverTcp(
                                obfuscationMode == ObfuscationMode.Udp2Tcp,
                                selectedUdp2TcpObfuscationPort,
                            )
                        )
                        add(ObfuscationSettingItem.Divider)
                        add(
                            ObfuscationSettingItem.Obfuscation.Off(
                                obfuscationMode == ObfuscationMode.Off
                            )
                        )
                    },
            )
    }
}
