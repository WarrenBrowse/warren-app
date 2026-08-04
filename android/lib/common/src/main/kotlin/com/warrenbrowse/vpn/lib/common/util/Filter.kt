package com.warrenbrowse.vpn.lib.common.util

import com.warrenbrowse.vpn.lib.model.MultihopRelayListType
import com.warrenbrowse.vpn.lib.model.RelayListType

fun shouldFilterByDaita(daitaDirectOnly: Boolean, relayListType: RelayListType) =
    when (relayListType) {
        RelayListType.Single -> daitaDirectOnly
        is RelayListType.Multihop ->
            daitaDirectOnly && relayListType.multihopRelayListType == MultihopRelayListType.ENTRY
    }

fun shouldFilterByQuic(isQuicEnabled: Boolean, relayListType: RelayListType) =
    when (relayListType) {
        RelayListType.Single -> isQuicEnabled
        is RelayListType.Multihop ->
            isQuicEnabled && relayListType.multihopRelayListType == MultihopRelayListType.ENTRY
    }

fun shouldFilterByLwo(isLwoEnable: Boolean, relayListType: RelayListType) =
    when (relayListType) {
        RelayListType.Single -> isLwoEnable
        is RelayListType.Multihop ->
            isLwoEnable && relayListType.multihopRelayListType == MultihopRelayListType.ENTRY
    }
