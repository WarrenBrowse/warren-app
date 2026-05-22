@file:Suppress("TooManyFunctions")

package com.warrenbrowse.vpn.lib.grpc.mapper

import mullvad_daemon.management_interface.ManagementInterface
import com.warrenbrowse.vpn.lib.model.ApiAccessMethod
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodId
import com.warrenbrowse.vpn.lib.model.ApiAccessMethodSetting
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.CustomDnsOptions
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.DaitaSettings
import com.warrenbrowse.vpn.lib.model.DefaultDnsOptions
import com.warrenbrowse.vpn.lib.model.DnsOptions
import com.warrenbrowse.vpn.lib.model.DnsState
import com.warrenbrowse.vpn.lib.model.GeoLocationId
import com.warrenbrowse.vpn.lib.model.IpVersion
import com.warrenbrowse.vpn.lib.model.LwoObfuscationSettings
import com.warrenbrowse.vpn.lib.model.ObfuscationMode
import com.warrenbrowse.vpn.lib.model.ObfuscationSettings
import com.warrenbrowse.vpn.lib.model.Ownership
import com.warrenbrowse.vpn.lib.model.Port
import com.warrenbrowse.vpn.lib.model.Providers
import com.warrenbrowse.vpn.lib.model.RelayItemId
import com.warrenbrowse.vpn.lib.model.RelaySettings
import com.warrenbrowse.vpn.lib.model.ShadowsocksObfuscationSettings
import com.warrenbrowse.vpn.lib.model.SocksAuth
import com.warrenbrowse.vpn.lib.model.Udp2TcpObfuscationSettings
import com.warrenbrowse.vpn.lib.model.WireguardConstraints

internal fun Constraint<RelayItemId>.fromDomain(): ManagementInterface.LocationConstraint =
    ManagementInterface.LocationConstraint.newBuilder()
        .apply {
            when (this@fromDomain) {
                Constraint.Any ->
                    setLocation(
                        ManagementInterface.GeographicLocationConstraint.getDefaultInstance()
                    )
                is Constraint.Only ->
                    when (val relayItemId = this@fromDomain.value) {
                        is CustomListId -> setCustomList(relayItemId.value)
                        is GeoLocationId -> setLocation(relayItemId.fromDomain())
                    }
            }
        }
        .build()

internal fun Constraint<Providers>.fromDomain(): List<String> =
    when (this) {
        is Constraint.Any -> emptyList()
        is Constraint.Only -> value.map { it.value }
    }

internal fun DnsOptions.fromDomain(): ManagementInterface.DnsOptions =
    ManagementInterface.DnsOptions.newBuilder()
        .setState(state.fromDomain())
        .setCustomOptions(customOptions.fromDomain())
        .setDefaultOptions(defaultOptions.fromDomain())
        .build()

internal fun DnsState.fromDomain(): ManagementInterface.DnsOptions.DnsState =
    when (this) {
        DnsState.Default -> ManagementInterface.DnsOptions.DnsState.DEFAULT
        DnsState.Custom -> ManagementInterface.DnsOptions.DnsState.CUSTOM
    }

internal fun CustomDnsOptions.fromDomain(): ManagementInterface.CustomDnsOptions =
    ManagementInterface.CustomDnsOptions.newBuilder()
        .addAllAddresses(addresses.map { it.hostAddress })
        .build()

internal fun DefaultDnsOptions.fromDomain(): ManagementInterface.DefaultDnsOptions =
    ManagementInterface.DefaultDnsOptions.newBuilder()
        .setBlockAds(blockAds)
        .setBlockGambling(blockGambling)
        .setBlockMalware(blockMalware)
        .setBlockTrackers(blockTrackers)
        .setBlockAdultContent(blockAdultContent)
        .setBlockSocialMedia(blockSocialMedia)
        .build()

internal fun ObfuscationSettings.fromDomain(): ManagementInterface.ObfuscationSettings =
    ManagementInterface.ObfuscationSettings.newBuilder()
        .setSelectedObfuscation(selectedObfuscationMode.fromDomain())
        .setUdp2Tcp(udp2tcp.fromDomain())
        .setShadowsocks(shadowsocks.fromDomain())
        .setWireguardPort(wireguardPort.fromDomain())
        .setLwo(lwo.fromDomain())
        .build()

internal fun ObfuscationMode.fromDomain():
    ManagementInterface.ObfuscationSettings.SelectedObfuscation =
    when (this) {
        ObfuscationMode.Udp2Tcp ->
            ManagementInterface.ObfuscationSettings.SelectedObfuscation.UDP2TCP
        ObfuscationMode.Shadowsocks ->
            ManagementInterface.ObfuscationSettings.SelectedObfuscation.SHADOWSOCKS
        ObfuscationMode.WireguardPort ->
            ManagementInterface.ObfuscationSettings.SelectedObfuscation.WIREGUARD_PORT
        ObfuscationMode.Quic -> ManagementInterface.ObfuscationSettings.SelectedObfuscation.QUIC
        ObfuscationMode.Lwo -> ManagementInterface.ObfuscationSettings.SelectedObfuscation.LWO
        ObfuscationMode.Auto -> ManagementInterface.ObfuscationSettings.SelectedObfuscation.AUTO
        ObfuscationMode.Off -> ManagementInterface.ObfuscationSettings.SelectedObfuscation.OFF
    }

internal fun Udp2TcpObfuscationSettings.fromDomain():
    ManagementInterface.ObfuscationSettings.Udp2TcpObfuscation =
    ManagementInterface.ObfuscationSettings.Udp2TcpObfuscation.newBuilder()
        .let {
            when (val port = port) {
                is Constraint.Any -> it.clearPort()
                is Constraint.Only -> it.setPort(port.value.value)
            }
        }
        .build()

internal fun Constraint<Port>.fromDomain(): ManagementInterface.ObfuscationSettings.WireguardPort =
    ManagementInterface.ObfuscationSettings.WireguardPort.newBuilder()
        .let {
            when (this) {
                is Constraint.Any -> it.clearPort()
                is Constraint.Only -> it.setPort(value.value)
            }
        }
        .build()

internal fun GeoLocationId.fromDomain(): ManagementInterface.GeographicLocationConstraint =
    ManagementInterface.GeographicLocationConstraint.newBuilder()
        .let {
            when (this) {
                is GeoLocationId.Country -> it.setCountry(code)
                is GeoLocationId.City -> it.setCountry(country.code).setCity(code)
                is GeoLocationId.Hostname ->
                    it.setCountry(country.code).setCity(city.code).setHostname(code)
            }
        }
        .build()

// D.4 step 52: CustomList.fromDomain dropped (updateCustomList accessor gone).

internal fun WireguardConstraints.fromDomain(): ManagementInterface.WireguardConstraints =
    ManagementInterface.WireguardConstraints.newBuilder()
        .setUseMultihop(isMultihopEnabled)
        .setEntryLocation(entryLocation.fromDomain())
        .let {
            when (val ipVersion = ipVersion) {
                is Constraint.Any -> it.clearIpVersion()
                is Constraint.Only -> it.setIpVersion(ipVersion.value.fromDomain())
            }
        }
        .build()

internal fun Ownership.fromDomain(): ManagementInterface.Ownership =
    when (this) {
        Ownership.MullvadOwned -> ManagementInterface.Ownership.MULLVAD_OWNED
        Ownership.Rented -> ManagementInterface.Ownership.RENTED
    }

internal fun RelaySettings.fromDomain(): ManagementInterface.RelaySettings =
    ManagementInterface.RelaySettings.newBuilder()
        .setNormal(
            ManagementInterface.NormalRelaySettings.newBuilder()
                .setWireguardConstraints(relayConstraints.wireguardConstraints.fromDomain())
                .setLocation(relayConstraints.location.fromDomain())
                .setOwnership(relayConstraints.ownership.fromDomain())
                .addAllProviders(relayConstraints.providers.fromDomain())
                .build()
        )
        .build()

internal fun Constraint<Ownership>.fromDomain(): ManagementInterface.Ownership =
    when (this) {
        Constraint.Any -> ManagementInterface.Ownership.ANY
        is Constraint.Only -> value.fromDomain()
    }

// D.4 step 50: PlayPurchasePaymentToken.fromDomain + PlayPurchase.fromDomain
// dropped (Play Store billing dead).

// D.4 step 48: NewAccessMethodSetting.fromDomain dropped (apiaccess dead).

internal fun ApiAccessMethod.fromDomain(): ManagementInterface.AccessMethod =
    ManagementInterface.AccessMethod.newBuilder()
        .let {
            when (this) {
                ApiAccessMethod.Direct ->
                    it.setDirect(ManagementInterface.AccessMethod.Direct.getDefaultInstance())
                ApiAccessMethod.Bridges ->
                    it.setBridges(ManagementInterface.AccessMethod.Bridges.getDefaultInstance())
                is ApiAccessMethod.CustomProxy -> it.setCustom(fromDomain())
                is ApiAccessMethod.EncryptedDns ->
                    it.setEncryptedDnsProxy(
                        ManagementInterface.AccessMethod.EncryptedDnsProxy.getDefaultInstance()
                    )
            }
        }
        .build()

internal fun ApiAccessMethod.CustomProxy.fromDomain(): ManagementInterface.CustomProxy =
    ManagementInterface.CustomProxy.newBuilder()
        .let {
            when (this) {
                is ApiAccessMethod.CustomProxy.Shadowsocks -> it.setShadowsocks(fromDomain())
                is ApiAccessMethod.CustomProxy.Socks5Remote -> it.setSocks5Remote(fromDomain())
            }
        }
        .build()

internal fun ApiAccessMethod.CustomProxy.Socks5Remote.fromDomain():
    ManagementInterface.Socks5Remote =
    ManagementInterface.Socks5Remote.newBuilder().setIp(ip).setPort(port.value).let {
        auth?.let { auth -> it.setAuth(auth.fromDomain()) }
        it.build()
    }

internal fun SocksAuth.fromDomain(): ManagementInterface.SocksAuth =
    ManagementInterface.SocksAuth.newBuilder().setUsername(username).setPassword(password).build()

internal fun ApiAccessMethod.CustomProxy.Shadowsocks.fromDomain(): ManagementInterface.Shadowsocks =
    ManagementInterface.Shadowsocks.newBuilder()
        .setIp(ip)
        .setCipher(cipher.label)
        .setPort(port.value)
        .let {
            if (password != null) {
                it.setPassword(password)
            }
            it.build()
        }

internal fun ApiAccessMethodId.fromDomain(): ManagementInterface.UUID =
    ManagementInterface.UUID.newBuilder().setValue(value.toString()).build()

internal fun ApiAccessMethodSetting.fromDomain(): ManagementInterface.AccessMethodSetting =
    ManagementInterface.AccessMethodSetting.newBuilder()
        .setName(name.value)
        .setId(id.fromDomain())
        .setEnabled(enabled)
        .setAccessMethod(apiAccessMethod.fromDomain())
        .build()

internal fun ShadowsocksObfuscationSettings.fromDomain():
    ManagementInterface.ObfuscationSettings.Shadowsocks =
    when (val port = port) {
        is Constraint.Any ->
            ManagementInterface.ObfuscationSettings.Shadowsocks.newBuilder().clearPort().build()
        is Constraint.Only ->
            ManagementInterface.ObfuscationSettings.Shadowsocks.newBuilder()
                .setPort(port.value.value)
                .build()
    }

internal fun LwoObfuscationSettings.fromDomain(): ManagementInterface.ObfuscationSettings.Lwo =
    when (val port = port) {
        is Constraint.Any ->
            ManagementInterface.ObfuscationSettings.Lwo.newBuilder().clearPort().build()
        is Constraint.Only ->
            ManagementInterface.ObfuscationSettings.Lwo.newBuilder()
                .setPort(port.value.value)
                .build()
    }

internal fun IpVersion.fromDomain(): ManagementInterface.IpVersion =
    when (this) {
        IpVersion.IPV4 -> ManagementInterface.IpVersion.V4
        IpVersion.IPV6 -> ManagementInterface.IpVersion.V6
    }

// D.4 step 48: RelaySelectorPredicate + MultihopConstraints + EntryConstraints
// + ExitConstraints + applyIfOnly + fromDomain1 mappers dropped (relay selector
// tooling dead). DaitaSettings.fromDomain kept inline below.

internal fun DaitaSettings.fromDomain(): ManagementInterface.DaitaSettings =
    ManagementInterface.DaitaSettings.newBuilder()
        .setEnabled(enabled)
        .setDirectOnly(directOnly)
        .build()

internal fun RelayItemId.fromDomain(): ManagementInterface.LocationConstraint =
    ManagementInterface.LocationConstraint.newBuilder()
        .let {
            when (this) {
                is CustomListId -> it.setCustomList(value)
                is GeoLocationId -> it.setLocation(fromDomain())
            }
        }
        .build()
