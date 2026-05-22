package com.warrenbrowse.vpn.lib.grpc

import android.net.LocalSocketAddress
import arrow.core.Either
import arrow.core.raise.either
import arrow.core.raise.ensure
import arrow.optics.copy
import arrow.optics.dsl.index
import arrow.optics.typeclasses.Index
import co.touchlab.kermit.Logger
import com.google.protobuf.BoolValue
import com.google.protobuf.Empty
import com.google.protobuf.StringValue
import com.google.protobuf.UInt32Value
import io.grpc.ConnectivityState
import io.grpc.Status
import io.grpc.StatusException
import io.grpc.android.UdsChannelBuilder
import java.io.File
import java.net.InetAddress
import java.util.logging.Level
import java.util.logging.Logger as JavaLogger
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.asExecutor
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.mapNotNull
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import mullvad_daemon.management_interface.ManagementInterface
import mullvad_daemon.management_interface.ManagementServiceGrpcKt
import com.warrenbrowse.vpn.lib.grpc.mapper.fromDomain
import com.warrenbrowse.vpn.lib.grpc.mapper.toDomain
import com.warrenbrowse.vpn.lib.grpc.util.AndroidLoggingHandler
import com.warrenbrowse.vpn.lib.grpc.util.LogInterceptor
import com.warrenbrowse.vpn.lib.grpc.util.connectivityFlow
import com.warrenbrowse.vpn.lib.model.AccountData
import com.warrenbrowse.vpn.lib.model.AccountNumber
import com.warrenbrowse.vpn.lib.model.AddSplitTunnelingAppError
import com.warrenbrowse.vpn.lib.model.AppVersionInfo as ModelAppVersionInfo
import com.warrenbrowse.vpn.lib.model.ConnectError
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.CustomList as ModelCustomList
import com.warrenbrowse.vpn.lib.model.CustomListId
import com.warrenbrowse.vpn.lib.model.CustomListName
import com.warrenbrowse.vpn.lib.model.DefaultDnsOptions
import com.warrenbrowse.vpn.lib.model.DeleteDeviceError
import com.warrenbrowse.vpn.lib.model.Device
import com.warrenbrowse.vpn.lib.model.DeviceId
import com.warrenbrowse.vpn.lib.model.DeviceState as ModelDeviceState
import com.warrenbrowse.vpn.lib.model.DeviceUpdateError
import com.warrenbrowse.vpn.lib.model.DisconnectReason
import com.warrenbrowse.vpn.lib.model.DnsOptions as ModelDnsOptions
import com.warrenbrowse.vpn.lib.model.DnsOptions
import com.warrenbrowse.vpn.lib.model.DnsState as ModelDnsState
import com.warrenbrowse.vpn.lib.model.DnsState
import com.warrenbrowse.vpn.lib.model.GeoLocationId
import com.warrenbrowse.vpn.lib.model.GetAccountDataError
import com.warrenbrowse.vpn.lib.model.GetDeviceListError
import com.warrenbrowse.vpn.lib.model.GetDeviceStateError
import com.warrenbrowse.vpn.lib.model.GetVersionInfoError
import com.warrenbrowse.vpn.lib.model.IpVersion
import com.warrenbrowse.vpn.lib.model.LogoutAccountError
import com.warrenbrowse.vpn.lib.model.NameAlreadyExists
import com.warrenbrowse.vpn.lib.model.ObfuscationMode
import com.warrenbrowse.vpn.lib.model.ObfuscationSettings
import com.warrenbrowse.vpn.lib.model.Ownership as ModelOwnership
import com.warrenbrowse.vpn.lib.model.PackageName
import com.warrenbrowse.vpn.lib.model.Port
import com.warrenbrowse.vpn.lib.model.Providers
import com.warrenbrowse.vpn.lib.model.QuantumResistantState as ModelQuantumResistantState
import com.warrenbrowse.vpn.lib.model.RelayConstraints
import com.warrenbrowse.vpn.lib.model.RelayItem
import com.warrenbrowse.vpn.lib.model.RelayItemId as ModelRelayItemId
import com.warrenbrowse.vpn.lib.model.RelayItemId
import com.warrenbrowse.vpn.lib.model.RelayList as ModelRelayList
import com.warrenbrowse.vpn.lib.model.RelayList
import com.warrenbrowse.vpn.lib.model.RelaySettings
import com.warrenbrowse.vpn.lib.model.RemoveSplitTunnelingAppError
import com.warrenbrowse.vpn.lib.model.SetAllowLanError
import com.warrenbrowse.vpn.lib.model.SetDaitaSettingsError
import com.warrenbrowse.vpn.lib.model.SetDnsOptionsError
import com.warrenbrowse.vpn.lib.model.SetObfuscationOptionsError
import com.warrenbrowse.vpn.lib.model.SetRelayLocationError
import com.warrenbrowse.vpn.lib.model.SetWireguardConstraintsError
import com.warrenbrowse.vpn.lib.model.SetWireguardMtuError
import com.warrenbrowse.vpn.lib.model.SetWireguardQuantumResistantError
import com.warrenbrowse.vpn.lib.model.Settings as ModelSettings
import com.warrenbrowse.vpn.lib.model.TunnelState as ModelTunnelState
import com.warrenbrowse.vpn.lib.model.UpdateCustomListError
import com.warrenbrowse.vpn.lib.model.UpdateRelayLocationsError
import com.warrenbrowse.vpn.lib.model.WireguardConstraints
import com.warrenbrowse.vpn.lib.model.WireguardEndpointData as ModelWireguardEndpointData
import com.warrenbrowse.vpn.lib.model.addresses
import com.warrenbrowse.vpn.lib.model.customOptions
import com.warrenbrowse.vpn.lib.model.defaultOptions
import com.warrenbrowse.vpn.lib.model.entryLocation
import com.warrenbrowse.vpn.lib.model.ipVersion
import com.warrenbrowse.vpn.lib.model.isMultihopEnabled
import com.warrenbrowse.vpn.lib.model.location
import com.warrenbrowse.vpn.lib.model.ownership
import com.warrenbrowse.vpn.lib.model.providers
import com.warrenbrowse.vpn.lib.model.relayConstraints
import com.warrenbrowse.vpn.lib.model.selectedObfuscationMode
import com.warrenbrowse.vpn.lib.model.shadowsocks
import com.warrenbrowse.vpn.lib.model.state
import com.warrenbrowse.vpn.lib.model.udp2tcp
import com.warrenbrowse.vpn.lib.model.wireguardConstraints
import com.warrenbrowse.vpn.lib.model.wireguardPort

@Suppress("TooManyFunctions", "LargeClass")
class ManagementService(
    rpcSocketFile: File,
    private val extensiveLogging: Boolean,
    private val scope: CoroutineScope,
) {
    private var job: Job? = null

    // We expect daemon to create the rpc socket file on the path provided on initialisation
    @Suppress("DEPRECATION")
    private val channel =
        UdsChannelBuilder.forPath(
                rpcSocketFile.absolutePath,
                LocalSocketAddress.Namespace.FILESYSTEM,
            )
            // Workaround for handling WiFi with proxy
            // https://github.com/grpc/grpc-java/issues/11922
            .proxyDetector { null }
            .build()

    val connectionState: StateFlow<GrpcConnectivityState> =
        channel
            .connectivityFlow()
            .map(ConnectivityState::toDomain)
            .onEach { Logger.i("ManagementService connection state: $it") }
            .stateIn(scope, SharingStarted.Eagerly, channel.getState(false).toDomain())

    // D.4 step 47: partitionRelays + RelaySelectorService stub dropped — only
    // consumer was the dead Mullvad relay selector tooling.

    private val grpc by lazy {
        ManagementServiceGrpcKt.ManagementServiceCoroutineStub(channel)
            .withExecutor(Dispatchers.IO.asExecutor())
            .let {
                if (extensiveLogging) {
                    it.withInterceptors(LogInterceptor())
                } else it
            }
            .withWaitForReady()
    }

    private val _mutableDeviceState = MutableStateFlow<ModelDeviceState?>(null)
    val deviceState: Flow<ModelDeviceState> = _mutableDeviceState.filterNotNull()

    private val _mutableTunnelState = MutableStateFlow<ModelTunnelState?>(null)
    val tunnelState: Flow<ModelTunnelState> = _mutableTunnelState.filterNotNull()

    private val _mutableSettings = MutableStateFlow<ModelSettings?>(null)
    val settings: Flow<ModelSettings> = _mutableSettings.filterNotNull()

    private val _mutableVersionInfo = MutableStateFlow<ModelAppVersionInfo?>(null)
    val versionInfo: Flow<ModelAppVersionInfo> = _mutableVersionInfo.filterNotNull()

    private val _mutableRelayList = MutableStateFlow<RelayList?>(null)
    val relayList: Flow<RelayList> = _mutableRelayList.filterNotNull()

    val relayCountries: Flow<List<RelayItem.Location.Country>> = relayList.mapNotNull {
        it.countries
    }

    val wireguardEndpointData: Flow<ModelWireguardEndpointData> = relayList.mapNotNull {
        it.wireguardEndpointData
    }

    // D.4 step 47: currentAccessMethod Flow dropped (apiaccess feature dead).

    init {
        if (extensiveLogging && ENABLE_TRACE_LOGGING) {
            AndroidLoggingHandler.reset(AndroidLoggingHandler())
            JavaLogger.getLogger("io.grpc").level = Level.FINEST
        }
    }

    fun start() {
        // Just to ensure that connection is set up since the connection won't be setup without a
        // call to the daemon
        if (job != null) {
            error("ManagementService already started")
        }

        job = scope.launch { subscribeEvents() }
    }

    fun stop() {
        job?.cancel(message = "ManagementService stopped")
            ?: error("ManagementService already stopped")
        job = null
    }

    fun enterIdle() = channel.enterIdle()

    private suspend fun subscribeEvents() =
        withContext(Dispatchers.IO) {
            launch {
                grpc.eventsListen(Empty.getDefaultInstance()).collect { event ->
                    if (extensiveLogging) {
                        Logger.v("Event: $event")
                    }
                    @Suppress("WHEN_ENUM_CAN_BE_NULL_IN_JAVA")
                    when (event.eventCase) {
                        ManagementInterface.DaemonEvent.EventCase.TUNNEL_STATE ->
                            _mutableTunnelState.update { event.tunnelState.toDomain() }
                        ManagementInterface.DaemonEvent.EventCase.SETTINGS ->
                            _mutableSettings.update { event.settings.toDomain() }
                        ManagementInterface.DaemonEvent.EventCase.RELAY_LIST ->
                            _mutableRelayList.update { event.relayList.toDomain() }
                        ManagementInterface.DaemonEvent.EventCase.VERSION_INFO ->
                            _mutableVersionInfo.update { event.versionInfo.toDomain() }
                        ManagementInterface.DaemonEvent.EventCase.DEVICE ->
                            _mutableDeviceState.update { event.device.newState.toDomain() }
                        ManagementInterface.DaemonEvent.EventCase.NEW_ACCESS_METHOD -> {}
                        ManagementInterface.DaemonEvent.EventCase.REMOVE_DEVICE -> {}
                        ManagementInterface.DaemonEvent.EventCase.LEAK_INFO -> {}
                        ManagementInterface.DaemonEvent.EventCase.EVENT_NOT_SET -> {}
                    }
                }
            }
            getInitialServiceState()
        }

    suspend fun getDevice(): Either<GetDeviceStateError, ModelDeviceState> =
        Either.catch { grpc.getDevice(Empty.getDefaultInstance()) }
            .map { it.toDomain() }
            .onLeft { Logger.e("Get device error") }
            .mapLeft { GetDeviceStateError.Unknown(it) }

    suspend fun updateDevice(): Either<DeviceUpdateError, Unit> =
        Either.catch { grpc.updateDevice(Empty.getDefaultInstance()) }
            .mapEmpty()
            .onLeft { Logger.e("Update device error") }
            .mapLeft { DeviceUpdateError(it) }

    suspend fun getDeviceList(token: AccountNumber): Either<GetDeviceListError, List<Device>> =
        Either.catch { grpc.listDevices(StringValue.of(token.value)) }
            .map { it.devicesList.map(ManagementInterface.Device::toDomain) }
            .onLeft { Logger.e("Get device list error") }
            .mapLeft { GetDeviceListError.Unknown(it) }

    suspend fun removeDevice(
        token: AccountNumber,
        deviceId: DeviceId,
    ): Either<DeleteDeviceError, Unit> =
        Either.catch {
                grpc.removeDevice(
                    ManagementInterface.DeviceRemoval.newBuilder()
                        .setAccountNumber(token.value)
                        .setDeviceId(deviceId.value.toString())
                        .build()
                )
            }
            .mapEmpty()
            .onLeft { Logger.e("Remove device error") }
            .mapLeft { DeleteDeviceError.Unknown(it) }

    suspend fun connect(): Either<ConnectError, Boolean> =
        Either.catch { grpc.connectTunnel(Empty.getDefaultInstance()).value }
            .onLeft { Logger.e("Connect error") }
            .mapLeft(ConnectError::Unknown)

    suspend fun disconnect(disconnectReason: DisconnectReason): Either<ConnectError, Boolean> =
        Either.catch { grpc.disconnectTunnel(StringValue.of(disconnectReason.logString)).value }
            .onLeft { Logger.e("Disconnect error") }
            .mapLeft(ConnectError::Unknown)

    suspend fun reconnect(): Either<ConnectError, Boolean> =
        Either.catch { grpc.reconnectTunnel(Empty.getDefaultInstance()).value }
            .onLeft { Logger.e("Reconnect error") }
            .mapLeft(ConnectError::Unknown)

    private suspend fun getTunnelState(): ModelTunnelState =
        grpc.getTunnelState(Empty.getDefaultInstance()).toDomain()

    private suspend fun getSettings(): ModelSettings =
        grpc.getSettings(Empty.getDefaultInstance()).toDomain()

    private suspend fun getDeviceState(): ModelDeviceState =
        grpc.getDevice(Empty.getDefaultInstance()).toDomain()

    private suspend fun getRelayList(): ModelRelayList =
        grpc.getRelayLocations(Empty.getDefaultInstance()).toDomain()

    // On release build this will return error until services have published the new beta, daemon
    // will get 404 until the api have been published, thus we need to ignore error downstream.
    private suspend fun getVersionInfo(): Either<GetVersionInfoError, ModelAppVersionInfo> =
        Either.catch { grpc.getVersionInfo(Empty.getDefaultInstance()).toDomain() }
            .onLeft { Logger.e("Get version info error") }
            .mapLeft { GetVersionInfoError.Unknown(it) }

    // D.4 step 47: getCurrentApiAccessMethod dropped (apiaccess feature dead).

    suspend fun logoutAccount(): Either<LogoutAccountError, Unit> =
        Either.catch { grpc.logoutAccount(StringValue.of("android-ui")) }
            .onLeft { Logger.e("Logout account error") }
            .mapLeft(LogoutAccountError::Unknown)
            .mapEmpty()

    // D.4 step 49: loginAccount + deleteAccount + clearAccountHistory +
    // getAccountHistory dropped (Mullvad account-number login + DeleteAccount
    // + AccountHistory screens deleted in steps 18/28).

    private suspend fun getInitialServiceState() {
        withContext(Dispatchers.IO) {
            awaitAll(
                async { _mutableTunnelState.update { getTunnelState() } },
                async { _mutableDeviceState.update { getDeviceState() } },
                async { _mutableSettings.update { getSettings() } },
                async { _mutableVersionInfo.update { getVersionInfo().getOrNull() } },
                async { _mutableRelayList.update { getRelayList() } },
            )
        }
    }

    suspend fun getAccountData(
        accountNumber: AccountNumber
    ): Either<GetAccountDataError, AccountData> =
        Either.catch {
                grpc.getAccountData(StringValue.of(accountNumber.value)).toDomain(accountNumber)
            }
            .onLeft { Logger.e("Get account data error") }
            .mapLeft(GetAccountDataError::Unknown)

    // D.4 step 49: createAccount dropped (Mullvad account creation flow dead).

    suspend fun updateDnsContentBlockers(
        update: (DefaultDnsOptions) -> DefaultDnsOptions
    ): Either<SetDnsOptionsError, Unit> =
        Either.catch {
                val currentDnsOptions = getSettings().tunnelOptions.dnsOptions
                val newDefaultDnsOptions = update(currentDnsOptions.defaultOptions)
                val updated = DnsOptions.defaultOptions.set(currentDnsOptions, newDefaultDnsOptions)
                grpc.setDnsOptions(updated.fromDomain())
            }
            .onLeft { Logger.e("Set dns state error") }
            .mapLeft(SetDnsOptionsError::Unknown)
            .mapEmpty()

    suspend fun setDnsOptions(dnsOptions: ModelDnsOptions): Either<SetDnsOptionsError, Unit> =
        Either.catch { grpc.setDnsOptions(dnsOptions.fromDomain()) }
            .onLeft { Logger.e("Set dns options error") }
            .mapLeft(SetDnsOptionsError::Unknown)
            .mapEmpty()

    suspend fun setDnsState(dnsState: ModelDnsState): Either<SetDnsOptionsError, Unit> =
        Either.catch {
                val currentDnsOptions = getSettings().tunnelOptions.dnsOptions
                val updated = DnsOptions.state.set(currentDnsOptions, dnsState)
                grpc.setDnsOptions(updated.fromDomain())
            }
            .onLeft { Logger.e("Set dns state error") }
            .mapLeft(SetDnsOptionsError::Unknown)
            .mapEmpty()

    suspend fun setCustomDns(index: Int, address: InetAddress): Either<SetDnsOptionsError, Unit> =
        Either.catch {
                val currentDnsOptions = getSettings().tunnelOptions.dnsOptions
                val updatedDnsOptions =
                    DnsOptions.customOptions.addresses
                        .index(Index.list(), index)
                        .set(currentDnsOptions, address)

                grpc.setDnsOptions(updatedDnsOptions.fromDomain())
            }
            .onLeft { Logger.e("Set custom dns error") }
            .mapLeft(SetDnsOptionsError::Unknown)
            .mapEmpty()

    suspend fun addCustomDns(address: InetAddress): Either<SetDnsOptionsError, Int> =
        Either.catch {
                val currentDnsOptions = getSettings().tunnelOptions.dnsOptions
                val updatedDnsOptions = currentDnsOptions.copy {
                    DnsOptions.customOptions.addresses set
                        currentDnsOptions.customOptions.addresses + address
                    // If it is the first address, then turn on Custom Dns
                    DnsOptions.state set
                        if (currentDnsOptions.customOptions.addresses.isEmpty()) DnsState.Custom
                        else currentDnsOptions.state
                }
                grpc.setDnsOptions(updatedDnsOptions.fromDomain())
                updatedDnsOptions.customOptions.addresses.lastIndex
            }
            .onLeft { Logger.e("Add custom dns error") }
            .mapLeft(SetDnsOptionsError::Unknown)

    suspend fun deleteCustomDns(index: Int): Either<SetDnsOptionsError, Unit> =
        Either.catch {
                val currentDnsOptions = getSettings().tunnelOptions.dnsOptions
                val mutableAddresses = currentDnsOptions.customOptions.addresses.toMutableList()
                mutableAddresses.removeAt(index)

                val updatedDnsOptions = currentDnsOptions.copy {
                    DnsOptions.customOptions.addresses set mutableAddresses.toList()
                    // If it is the last address, then turn off Custom Dns
                    DnsOptions.state set
                        if (mutableAddresses.isEmpty()) DnsState.Default
                        else currentDnsOptions.state
                }
                grpc.setDnsOptions(updatedDnsOptions.fromDomain())
            }
            .onLeft { Logger.e("Delete custom dns error") }
            .mapLeft(SetDnsOptionsError::Unknown)
            .mapEmpty()

    suspend fun setWireguardMtu(value: Int): Either<SetWireguardMtuError, Unit> =
        Either.catch { grpc.setWireguardMtu(UInt32Value.of(value)) }
            .onLeft { Logger.e("Set wireguard mtu error") }
            .mapLeft(SetWireguardMtuError::Unknown)
            .mapEmpty()

    suspend fun resetWireguardMtu(): Either<SetWireguardMtuError, Unit> =
        Either.catch { grpc.setWireguardMtu(UInt32Value.newBuilder().clearValue().build()) }
            .onLeft { Logger.e("Reset wireguard mtu error") }
            .mapLeft(SetWireguardMtuError::Unknown)
            .mapEmpty()

    suspend fun setWireguardQuantumResistant(
        value: ModelQuantumResistantState
    ): Either<SetWireguardQuantumResistantError, Unit> =
        Either.catch { grpc.setQuantumResistantTunnel(value.toDomain()) }
            .onLeft { Logger.e("Set wireguard quantum resistant error") }
            .mapLeft(SetWireguardQuantumResistantError::Unknown)
            .mapEmpty()

    suspend fun setObfuscation(value: ObfuscationMode): Either<SetObfuscationOptionsError, Unit> =
        Either.catch {
                val updatedObfuscationSettings =
                    ObfuscationSettings.selectedObfuscationMode.modify(
                        getSettings().obfuscationSettings
                    ) {
                        value
                    }
                grpc.setObfuscationSettings(updatedObfuscationSettings.fromDomain())
            }
            .onLeft { Logger.e("Set obfuscation error") }
            .mapLeft(SetObfuscationOptionsError::Unknown)
            .mapEmpty()

    suspend fun setWireguardObfuscationPort(
        portConstraint: Constraint<Port>
    ): Either<SetObfuscationOptionsError, Unit> =
        Either.catch {
                val updatedSettings =
                    ObfuscationSettings.wireguardPort.modify(getSettings().obfuscationSettings) {
                        portConstraint
                    }
                grpc.setObfuscationSettings(updatedSettings.fromDomain())
            }
            .onLeft { Logger.e("Set wireguard port error") }
            .mapLeft(SetObfuscationOptionsError::Unknown)
            .mapEmpty()

    suspend fun setUdp2TcpObfuscationPort(
        portConstraint: Constraint<Port>
    ): Either<SetObfuscationOptionsError, Unit> =
        Either.catch {
                val updatedSettings =
                    ObfuscationSettings.udp2tcp.modify(getSettings().obfuscationSettings) {
                        it.copy(port = portConstraint)
                    }
                grpc.setObfuscationSettings(updatedSettings.fromDomain())
            }
            .onLeft { Logger.e("Set obfuscation port error") }
            .mapLeft(SetObfuscationOptionsError::Unknown)
            .mapEmpty()

    suspend fun setShadowsocksObfuscationPort(
        portConstraint: Constraint<Port>
    ): Either<SetObfuscationOptionsError, Unit> =
        Either.catch {
                val updatedSettings =
                    ObfuscationSettings.shadowsocks.modify(getSettings().obfuscationSettings) {
                        it.copy(port = portConstraint)
                    }
                grpc.setObfuscationSettings(updatedSettings.fromDomain())
            }
            .mapLeft(SetObfuscationOptionsError::Unknown)
            .mapEmpty()

    suspend fun setAllowLan(allow: Boolean): Either<SetAllowLanError, Unit> =
        Either.catch { grpc.setAllowLan(BoolValue.of(allow)) }
            .onLeft { Logger.e("Set allow lan error") }
            .mapLeft(SetAllowLanError::Unknown)
            .mapEmpty()

    suspend fun setDaitaEnabled(enabled: Boolean): Either<SetDaitaSettingsError, Unit> =
        Either.catch { grpc.setEnableDaita(BoolValue.of(enabled)) }
            .mapLeft(SetDaitaSettingsError::Unknown)
            .mapEmpty()

    suspend fun setDaitaDirectOnly(enabled: Boolean): Either<SetDaitaSettingsError, Unit> =
        Either.catch { grpc.setDaitaDirectOnly(BoolValue.of(enabled)) }
            .mapLeft(SetDaitaSettingsError::Unknown)
            .mapEmpty()

    suspend fun setRelayLocation(location: ModelRelayItemId): Either<SetRelayLocationError, Unit> =
        Either.catch {
                val currentRelaySettings = getSettings().relaySettings
                val updatedRelaySettings =
                    RelaySettings.relayConstraints.location.set(
                        currentRelaySettings,
                        Constraint.Only(location),
                    )
                grpc.setRelaySettings(updatedRelaySettings.fromDomain())
            }
            .onLeft { Logger.e("Set relay location error") }
            .mapLeft(SetRelayLocationError::Unknown)
            .mapEmpty()

    suspend fun setRelayLocationMultihop(
        isMultihopEnabled: Boolean,
        entry: RelayItemId?,
        exit: RelayItemId,
    ): Either<SetRelayLocationError, Unit> =
        Either.catch {
                val currentRelaySettings = getSettings().relaySettings

                val updatedRelaySettings = currentRelaySettings.copy {
                    inside(RelaySettings.relayConstraints) {
                        RelayConstraints.location set Constraint.Only(exit)
                        if (entry != null) {
                            RelayConstraints.wireguardConstraints.entryLocation set
                                Constraint.Only(entry)
                        }
                        RelayConstraints.wireguardConstraints.isMultihopEnabled set
                            isMultihopEnabled
                    }
                }
                grpc.setRelaySettings(updatedRelaySettings.fromDomain())
            }
            .onLeft { Logger.e("Set relay multihop error") }
            .mapLeft(SetRelayLocationError::Unknown)
            .mapEmpty()

    // D.4 step 47: createCustomList dropped (CustomList feature dead).

    suspend fun updateCustomList(customList: ModelCustomList): Either<UpdateCustomListError, Unit> =
        Either.catch { grpc.updateCustomList(customList.fromDomain()) }
            .mapLeftStatus {
                when (it.status.code) {
                    Status.Code.ALREADY_EXISTS -> NameAlreadyExists(customList.name)
                    else -> {
                        Logger.e("Unknown update custom list error")
                        UnknownCustomListError(it)
                    }
                }
            }
            .mapEmpty()

    // D.4 step 47: deleteCustomList + clearAllRelayOverrides + applySettingsPatch
    // dropped (CustomList / ServerIpOverride / SettingsPatch features dead).

    suspend fun setOwnershipAndProviders(
        ownershipConstraint: Constraint<ModelOwnership>,
        providersConstraint: Constraint<Providers>,
    ): Either<SetWireguardConstraintsError, Unit> =
        Either.catch {
                val relaySettings = getSettings().relaySettings
                val updated = relaySettings.copy {
                    inside(RelaySettings.relayConstraints) {
                        RelayConstraints.providers set providersConstraint
                        RelayConstraints.ownership set ownershipConstraint
                    }
                }
                grpc.setRelaySettings(updated.fromDomain())
            }
            .onLeft { Logger.e("Set ownership and providers error") }
            .mapLeft(SetWireguardConstraintsError::Unknown)
            .mapEmpty()

    suspend fun setOwnership(
        ownership: Constraint<ModelOwnership>
    ): Either<SetWireguardConstraintsError, Unit> =
        Either.catch {
                val relaySettings = getSettings().relaySettings
                val updated = RelaySettings.relayConstraints.ownership.set(relaySettings, ownership)
                grpc.setRelaySettings(updated.fromDomain())
            }
            .onLeft { Logger.e("Set ownership error") }
            .mapLeft(SetWireguardConstraintsError::Unknown)
            .mapEmpty()

    suspend fun setProviders(
        providersConstraint: Constraint<Providers>
    ): Either<SetWireguardConstraintsError, Unit> =
        Either.catch {
                val relaySettings = getSettings().relaySettings
                val updated =
                    RelaySettings.relayConstraints.providers.set(relaySettings, providersConstraint)
                grpc.setRelaySettings(updated.fromDomain())
            }
            .onLeft { Logger.e("Set providers error") }
            .mapLeft(SetWireguardConstraintsError::Unknown)
            .mapEmpty()

    // D.4 step 50: submitVoucher + verifyPlayPurchase + initializePlayPurchase
    // accessors all dropped (Mullvad voucher redeem + Play Store VPN billing
    // both dead on Warren).

    suspend fun addSplitTunnelingApp(app: PackageName): Either<AddSplitTunnelingAppError, Unit> =
        Either.catch { grpc.addSplitTunnelApp(StringValue.of(app.value)) }
            .onLeft { Logger.e("Add split tunneling app error") }
            .mapLeft(AddSplitTunnelingAppError::Unknown)
            .mapEmpty()

    suspend fun removeSplitTunnelingApp(
        app: PackageName
    ): Either<RemoveSplitTunnelingAppError, Unit> =
        Either.catch { grpc.removeSplitTunnelApp(StringValue.of(app.value)) }
            .onLeft { Logger.e("Remove split tunneling app error") }
            .mapLeft(RemoveSplitTunnelingAppError::Unknown)
            .mapEmpty()

    suspend fun setSplitTunnelingState(
        enabled: Boolean
    ): Either<RemoveSplitTunnelingAppError, Unit> =
        Either.catch { grpc.setSplitTunnelState(BoolValue.of(enabled)) }
            .onLeft { Logger.e("Set split tunneling state error") }
            .mapLeft(RemoveSplitTunnelingAppError::Unknown)
            .mapEmpty()

    // D.4 step 46: getWebsiteAuthToken removed (mullvad.net web-account flow dead).

    // D.4 step 47: full apiaccess gRPC accessor block dropped — addApiAccessMethod,
    // removeApiAccessMethod, setApiAccessMethod, updateApiAccessMethod,
    // testCustomApiAccessMethod, testApiAccessMethodById (apiaccess feature
    // dead in step 33 ; Warren API endpoint is fixed at build time).

    suspend fun setMultihop(enabled: Boolean): Either<SetWireguardConstraintsError, Unit> =
        Either.catch {
                val relaySettings = getSettings().relaySettings
                val updated =
                    RelaySettings.relayConstraints.wireguardConstraints.isMultihopEnabled.set(
                        relaySettings,
                        enabled,
                    )
                grpc.setRelaySettings(updated.fromDomain())
            }
            .onLeft { Logger.e("Set multihop error") }
            .mapLeft(SetWireguardConstraintsError::Unknown)
            .mapEmpty()

    suspend fun setEntryLocation(
        entryLocation: RelayItemId
    ): Either<SetWireguardConstraintsError, Unit> =
        Either.catch {
                val relaySettings = getSettings().relaySettings
                val updated =
                    RelaySettings.relayConstraints.wireguardConstraints.entryLocation.set(
                        relaySettings,
                        Constraint.Only(entryLocation),
                    )
                grpc.setRelaySettings(updated.fromDomain())
            }
            .onLeft { Logger.e("Set multihop error") }
            .mapLeft(SetWireguardConstraintsError::Unknown)
            .mapEmpty()

    suspend fun setDeviceIpVersion(
        ipVersion: Constraint<IpVersion>
    ): Either<SetWireguardConstraintsError, Unit> =
        Either.catch {
                val relaySettings = getSettings().relaySettings
                val updated =
                    RelaySettings.relayConstraints.wireguardConstraints.ipVersion.set(
                        relaySettings,
                        ipVersion,
                    )
                grpc.setRelaySettings(updated.fromDomain())
            }
            .onLeft { Logger.e("Set multihop error") }
            .mapLeft(SetWireguardConstraintsError::Unknown)
            .mapEmpty()

    suspend fun setIpv6Enabled(enabled: Boolean): Either<SetDaitaSettingsError, Unit> =
        Either.catch { grpc.setEnableIpv6(BoolValue.of(enabled)) }
            .mapLeft(SetDaitaSettingsError::Unknown)
            .mapEmpty()

    suspend fun setRecentsEnabled(enabled: Boolean): Either<SetWireguardConstraintsError, Unit> =
        Either.catch { grpc.setEnableRecents(BoolValue.of(enabled)) }
            .mapLeft(SetWireguardConstraintsError::Unknown)
            .mapEmpty()

    suspend fun updateRelayLocations(): Either<UpdateRelayLocationsError, Unit> =
        Either.catch { grpc.updateRelayLocations(Empty.getDefaultInstance()) }
            .mapLeft(UpdateRelayLocationsError::Unknown)
            .mapEmpty()

    suspend fun setMultihopAndEntryLocation(
        isMultihopEnabled: Boolean,
        entryLocation: RelayItemId,
    ): Either<SetWireguardConstraintsError, Unit> =
        Either.catch {
                val currentRelaySettings = getSettings().relaySettings
                val updatedRelaySettings = currentRelaySettings.copy {
                    inside(RelaySettings.relayConstraints.wireguardConstraints) {
                        WireguardConstraints.entryLocation set Constraint.Only(entryLocation)
                        WireguardConstraints.isMultihopEnabled set isMultihopEnabled
                    }
                }
                grpc.setRelaySettings(updatedRelaySettings.fromDomain())
            }
            .onLeft { Logger.e("Set multihop error") }
            .mapLeft(SetWireguardConstraintsError::Unknown)
            .mapEmpty()

    private fun <A> Either<A, Empty>.mapEmpty() = map {}

    private inline fun <B, C> Either<Throwable, B>.mapLeftStatus(
        f: (StatusException) -> C
    ): Either<C, B> = mapLeft {
        if (it is StatusException) {
            f(it)
        } else {
            throw it
        }
    }

    private fun Status.isTooManyRequests() = description == TOO_MANY_REQUESTS

    companion object {
        const val ENABLE_TRACE_LOGGING = false

        const val TOO_MANY_REQUESTS = "429 Too Many Requests"
    }
}

// D.4 step 47: PartitionRelaysError dropped (partitionRelays method removed).

sealed interface GrpcConnectivityState {
    data object Connecting : GrpcConnectivityState

    data object Ready : GrpcConnectivityState

    data object Idle : GrpcConnectivityState

    data object TransientFailure : GrpcConnectivityState

    data object Shutdown : GrpcConnectivityState
}
