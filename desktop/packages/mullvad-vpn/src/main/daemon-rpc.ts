import * as grpc from '@grpc/grpc-js';
import { Empty } from 'google-protobuf/google/protobuf/empty_pb.js';
import { BoolValue, StringValue } from 'google-protobuf/google/protobuf/wrappers_pb.js';
import * as grpcTypes from 'management-interface/management-interface/grpc-types';

import {
  AccessMethodExistsError,
  AccessMethodSetting,
  AccountDataError,
  AccountDataResponse,
  CustomListError,
  CustomProxy,
  DaemonAppUpgradeEvent,
  DaemonEvent,
  DeviceState,
  DisconnectSource,
  IAppVersionInfo,
  ICustomList,
  IDevice,
  IDeviceRemoval,
  IDnsOptions,
  IRelayListWithEndpointData,
  ISettings,
  LogoutSource,
  NewAccessMethodSetting,
  NewCustomList,
  ObfuscationSettings,
  ObfuscationType,
  RelaySettings,
  TunnelState,
  VoucherResponse,
  WarrenMultiHopSettings,
  WarrenPubKey,
  WarrenStatus,
} from '../shared/daemon-rpc-types';
import { ConnectionObserver, GrpcClient, noConnectionError } from './grpc-client';
import {
  convertFromApiAccessMethodSetting,
  convertFromAppUpgradeEvent,
  convertFromAppVersionInfo,
  convertFromDaemonEvent,
  convertFromDevice,
  convertFromDeviceState,
  convertFromRelayList,
  convertFromSettings,
  convertFromTunnelState,
  convertFromWarrenStatus,
  convertToApiAccessMethodSetting,
  convertToCustomList,
  convertToCustomProxy,
  convertToNewApiAccessMethodSetting,
  convertToNewCustomList,
  convertToRelayConstraints,
  convertToWarrenMultiHopSettings,
  ensureExists,
} from './grpc-type-convertions';

// Must match `mullvad_paths::rpc_socket::get_default_rpc_socket_path` on the
// Rust side. Without this alignment, Electron hits the legacy upstream Mullvad
// path which can be squatted by a parallel official Mullvad VPN.app install.
const DAEMON_RPC_PATH =
  process.platform === 'win32' ? '//./pipe/Warren VPN' : '/var/run/warren-vpn';

export class SubscriptionListener<T> {
  // Only meant to be used by DaemonRpc
  // @internal
  public subscriptionId?: number;

  constructor(
    private eventHandler: (payload: T) => void,
    private errorHandler: (error: Error) => void,
  ) {}

  // Only meant to be called by DaemonRpc
  // @internal
  public onEvent(payload: T) {
    this.eventHandler(payload);
  }

  // Only meant to be called by DaemonRpc
  // @internal
  public onError(error: Error) {
    this.errorHandler(error);
  }
}

export class DaemonRpc extends GrpcClient {
  private nextSubscriptionId = 0;
  private subscriptions: Map<
    number,
    grpc.ClientReadableStream<
      grpcTypes.DaemonEvent | grpcTypes.AppUpgradeEvent | grpcTypes.WarrenStatus
    >
  > = new Map();

  public constructor(connectionObserver?: ConnectionObserver) {
    super(DAEMON_RPC_PATH, connectionObserver);
  }

  public disconnect() {
    for (const subscriptionId of this.subscriptions.keys()) {
      this.removeSubscription(subscriptionId);
    }

    super.disconnect();
  }

  public subscribeAppUpgradeEventListener(listener: SubscriptionListener<DaemonAppUpgradeEvent>) {
    const call = this.isConnected && this.client.appUpgradeEventsListen(new Empty());
    if (!call) {
      throw noConnectionError;
    }
    const subscriptionId = this.subscriptionId();
    listener.subscriptionId = subscriptionId;
    this.subscriptions.set(subscriptionId, call);

    call.on('data', (data: grpcTypes.AppUpgradeEvent) => {
      try {
        const appUpgradeEvent = convertFromAppUpgradeEvent(data);
        listener.onEvent(appUpgradeEvent);
      } catch (e) {
        const error = e as Error;
        listener.onError(error);
      }
    });

    call.on('error', (error) => {
      listener.onError(error);
      this.removeSubscription(subscriptionId);
    });
  }

  public appUpgrade() {
    void this.callEmpty(this.client.appUpgrade);
  }

  public appUpgradeAbort() {
    void this.callEmpty(this.client.appUpgradeAbort);
  }

  public async getAppUpgradeCacheDir(): Promise<string> {
    const response = await this.callEmpty<StringValue>(this.client.getAppUpgradeCacheDir);
    return response.getValue();
  }

  public unsubscribeAppUpgradeEventListener(listener: SubscriptionListener<DaemonAppUpgradeEvent>) {
    const id = listener.subscriptionId;
    if (id !== undefined) {
      this.removeSubscription(id);
    }
  }

  public subscribeDaemonEventListener(listener: SubscriptionListener<DaemonEvent>) {
    const call = this.isConnected && this.client.eventsListen(new Empty());
    if (!call) {
      throw noConnectionError;
    }
    const subscriptionId = this.subscriptionId();
    listener.subscriptionId = subscriptionId;
    this.subscriptions.set(subscriptionId, call);

    call.on('data', (data: grpcTypes.DaemonEvent) => {
      try {
        const daemonEvent = convertFromDaemonEvent(data);
        listener.onEvent(daemonEvent);
      } catch (e) {
        const error = e as Error;
        listener.onError(error);
      }
    });

    call.on('error', (error) => {
      listener.onError(error);
      this.removeSubscription(subscriptionId);
    });
  }

  public unsubscribeDaemonEventListener(listener: SubscriptionListener<DaemonEvent>) {
    const id = listener.subscriptionId;
    if (id !== undefined) {
      this.removeSubscription(id);
    }
  }

  // Subscribe to the daemon's WarrenStatusUpdates push stream. Each
  // emitted snapshot is converted to the renderer-facing WarrenStatus
  // shape before being forwarded to the listener.
  public subscribeWarrenStatusListener(listener: SubscriptionListener<WarrenStatus>) {
    const call = this.isConnected && this.client.warrenStatusUpdates(new Empty());
    if (!call) {
      throw noConnectionError;
    }
    const subscriptionId = this.subscriptionId();
    listener.subscriptionId = subscriptionId;
    this.subscriptions.set(subscriptionId, call);

    call.on('data', (data: grpcTypes.WarrenStatus) => {
      try {
        listener.onEvent(convertFromWarrenStatus(data));
      } catch (e) {
        const error = e as Error;
        listener.onError(error);
      }
    });

    call.on('error', (error) => {
      listener.onError(error);
      this.removeSubscription(subscriptionId);
    });
  }

  public unsubscribeWarrenStatusListener(listener: SubscriptionListener<WarrenStatus>) {
    const id = listener.subscriptionId;
    if (id !== undefined) {
      this.removeSubscription(id);
    }
  }

  public async getAccountData(pubkey: WarrenPubKey): Promise<AccountDataResponse> {
    try {
      const response = await this.callString<grpcTypes.AccountData>(
        this.client.getAccountData,
        pubkey,
      );
      const expiry = response.getExpiry()!.toDate().toISOString();
      return { type: 'success', expiry };
    } catch (e) {
      const error = e as grpc.ServiceError;
      if (error.code) {
        switch (error.code) {
          case grpc.status.UNAUTHENTICATED:
            return { type: 'error', error: 'invalid-account' };
          default:
            return { type: 'error', error: 'communication' };
        }
      }
      throw error;
    }
  }

  public async getWwwAuthToken(): Promise<string> {
    const response = await this.callEmpty<StringValue>(this.client.getWwwAuthToken);
    return response.getValue();
  }

  /**
   * Returns the BIP39 mnemonic (12 words) so the user can back it up.
   * Empty string if the identity has never been bootstrapped. The
   * renderer caller must display it with a safety warning and explicit
   * user confirmation.
   */
  public async getWarrenMnemonic(): Promise<string> {
    const response = await this.callEmpty<StringValue>(this.client.getWarrenMnemonic);
    return response.getValue();
  }

  /**
   * Replaces the identity with the provided BIP39 mnemonic. The daemon
   * validates BIP39 and writes atomically. Throws `grpc.ServiceError`
   * (status INVALID_ARGUMENT) if the BIP39 input is invalid. Daemon
   * restart is required to activate.
   */
  public async setWarrenMnemonic(mnemonic: string): Promise<void> {
    await this.callString<Empty>(this.client.setWarrenMnemonic, mnemonic);
  }

  public async submitVoucher(voucherCode: string): Promise<VoucherResponse> {
    try {
      const response = await this.callString<grpcTypes.VoucherSubmission>(
        this.client.submitVoucher,
        voucherCode,
      );

      const secondsAdded = ensureExists(
        response.getSecondsAdded(),
        "no 'secondsAdded' field in voucher response",
      );
      const newExpiry = ensureExists(
        response.getNewExpiry(),
        "no 'newExpiry' field in voucher response",
      )
        .toDate()
        .toISOString();
      return {
        type: 'success',
        secondsAdded,
        newExpiry,
      };
    } catch (e) {
      const error = e as grpc.ServiceError;
      if (error.code) {
        switch (error.code) {
          case grpc.status.NOT_FOUND:
            return { type: 'invalid' };
          case grpc.status.RESOURCE_EXHAUSTED:
            return { type: 'already_used' };
        }
      }
      return { type: 'error' };
    }
  }

  public async getRelayLocations(): Promise<IRelayListWithEndpointData> {
    if (this.isConnected) {
      const response = await this.callEmpty<grpcTypes.RelayList>(this.client.getRelayLocations);
      return convertFromRelayList(response);
    } else {
      throw noConnectionError;
    }
  }

  public async createNewAccount(): Promise<string> {
    const response = await this.callEmpty<StringValue>(this.client.createNewAccount);
    return response.getValue();
  }

  public async loginAccount(pubkey: WarrenPubKey): Promise<AccountDataError | void> {
    try {
      await this.callString(this.client.loginAccount, pubkey);
    } catch (e) {
      const error = e as grpc.ServiceError;
      switch (error.code) {
        case grpc.status.RESOURCE_EXHAUSTED:
          return { type: 'error', error: 'too-many-devices' };
        case grpc.status.UNAUTHENTICATED:
          return { type: 'error', error: 'invalid-account' };
        default:
          return { type: 'error', error: 'communication' };
      }
    }
  }

  public async logoutAccount(source: LogoutSource): Promise<void> {
    const prefixedSource = `desktop ${source}`;
    await this.callString(this.client.logoutAccount, prefixedSource);
  }

  // TODO: Custom tunnel configurations are not supported by the GUI.
  public async setRelaySettings(relaySettings: RelaySettings): Promise<void> {
    if ('normal' in relaySettings) {
      const normalSettings = relaySettings.normal;
      const grpcRelaySettings = new grpcTypes.RelaySettings();
      grpcRelaySettings.setNormal(convertToRelayConstraints(normalSettings));

      await this.call<grpcTypes.RelaySettings, Empty>(
        this.client.setRelaySettings,
        grpcRelaySettings,
      );
    }
  }

  public async setAllowLan(allowLan: boolean): Promise<void> {
    await this.callBool(this.client.setAllowLan, allowLan);
  }

  // Persistent toggle for the Iroh tunnel mode. Daemon restart is
  // required to apply (see resolve() on the Rust side).
  public async setWarrenMode(enabled: boolean): Promise<void> {
    await this.callBool(this.client.setWarrenMode, enabled);
  }

  // Persistent toggle for local account mode (= no api.mullvad.net).
  // Daemon restart required.
  public async setWarrenLocalAccount(enabled: boolean): Promise<void> {
    await this.callBool(this.client.setWarrenLocalAccount, enabled);
  }

  // Persistent warren-api URL. Empty string → unset on the daemon
  // side (= fallback to upstream Mullvad backend). Daemon restart is
  // required to apply (see `resolve_warren_api_config` on the Rust
  // side, which reads Settings at boot only).
  public async setWarrenApiUrl(url: string): Promise<void> {
    await this.callString(this.client.setWarrenApiUrl, url);
  }

  // Warren multi-hop settings (M4.E.D). Restart daemon required to
  // apply (the supervisor is wired once at boot from the env-var +
  // settings-file path).
  public async getWarrenMultiHopSettings(): Promise<WarrenMultiHopSettings> {
    const response = await this.callEmpty<grpcTypes.WarrenMultiHopSettings>(
      this.client.getWarrenMultiHopSettings,
    );
    const rotation = response.getHpkeEpochRotation();
    return {
      enabled: response.getEnabled(),
      entryCountry: response.getEntryCountry(),
      exitCountry: response.getExitCountry(),
      hpkeEpochRotationMs: rotation
        ? rotation.getSeconds() * 1000 + Math.floor(rotation.getNanos() / 1e6)
        : 4 * 60 * 60 * 1000,
    };
  }

  public async setWarrenMultiHopSettings(settings: WarrenMultiHopSettings): Promise<void> {
    const proto = convertToWarrenMultiHopSettings(settings);
    await this.call<grpcTypes.WarrenMultiHopSettings, Empty>(
      this.client.setWarrenMultiHopSettings,
      proto,
    );
  }

  public async getWarrenStatus(): Promise<WarrenStatus> {
    const response = await this.callEmpty<grpcTypes.WarrenStatus>(this.client.getWarrenStatus);
    return convertFromWarrenStatus(response);
  }

  public async setShowBetaReleases(showBetaReleases: boolean): Promise<void> {
    await this.callBool(this.client.setShowBetaReleases, showBetaReleases);
  }

  public async setEnableIpv6(enableIpv6: boolean): Promise<void> {
    await this.callBool(this.client.setEnableIpv6, enableIpv6);
  }

  public async setLockdownMode(lockdownMode: boolean): Promise<void> {
    await this.callBool(this.client.setLockdownMode, lockdownMode);
  }

  public async setObfuscationSettings(obfuscationSettings: ObfuscationSettings): Promise<void> {
    const grpcObfuscationSettings = new grpcTypes.ObfuscationSettings();
    switch (obfuscationSettings.selectedObfuscation) {
      case ObfuscationType.auto:
        grpcObfuscationSettings.setSelectedObfuscation(
          grpcTypes.ObfuscationSettings.SelectedObfuscation.AUTO,
        );
        break;
      case ObfuscationType.off:
        grpcObfuscationSettings.setSelectedObfuscation(
          grpcTypes.ObfuscationSettings.SelectedObfuscation.OFF,
        );
        break;
      case ObfuscationType.shadowsocks:
        grpcObfuscationSettings.setSelectedObfuscation(
          grpcTypes.ObfuscationSettings.SelectedObfuscation.SHADOWSOCKS,
        );
        break;
      case ObfuscationType.udp2tcp:
        grpcObfuscationSettings.setSelectedObfuscation(
          grpcTypes.ObfuscationSettings.SelectedObfuscation.UDP2TCP,
        );
        break;
      case ObfuscationType.quic:
        grpcObfuscationSettings.setSelectedObfuscation(
          grpcTypes.ObfuscationSettings.SelectedObfuscation.QUIC,
        );
        break;
      case ObfuscationType.lwo:
        grpcObfuscationSettings.setSelectedObfuscation(
          grpcTypes.ObfuscationSettings.SelectedObfuscation.LWO,
        );
        break;
      case ObfuscationType.wireGuardPort:
        grpcObfuscationSettings.setSelectedObfuscation(
          grpcTypes.ObfuscationSettings.SelectedObfuscation.WIREGUARD_PORT,
        );
        break;
    }

    if (obfuscationSettings.udp2tcpSettings) {
      const grpcUdp2tcpSettings = new grpcTypes.ObfuscationSettings.Udp2TcpObfuscation();
      if (obfuscationSettings.udp2tcpSettings.port !== 'any') {
        grpcUdp2tcpSettings.setPort(obfuscationSettings.udp2tcpSettings.port.only);
      }
      grpcObfuscationSettings.setUdp2tcp(grpcUdp2tcpSettings);
    }

    if (obfuscationSettings.shadowsocksSettings) {
      const shadowsocksSettings = new grpcTypes.ObfuscationSettings.Shadowsocks();
      if (obfuscationSettings.shadowsocksSettings.port !== 'any') {
        shadowsocksSettings.setPort(obfuscationSettings.shadowsocksSettings.port.only);
      }
      grpcObfuscationSettings.setShadowsocks(shadowsocksSettings);
    }

    if (obfuscationSettings.wireGuardPortSettings) {
      const wireGuardPortSettings = new grpcTypes.ObfuscationSettings.WireguardPort();
      if (obfuscationSettings.wireGuardPortSettings.port !== 'any') {
        wireGuardPortSettings.setPort(obfuscationSettings.wireGuardPortSettings.port.only);
      }
      grpcObfuscationSettings.setWireguardPort(wireGuardPortSettings);
    }

    if (obfuscationSettings.lwoSettings) {
      const lwoSettings = new grpcTypes.ObfuscationSettings.Lwo();
      if (obfuscationSettings.lwoSettings.port !== 'any') {
        lwoSettings.setPort(obfuscationSettings.lwoSettings.port.only);
      }
      grpcObfuscationSettings.setLwo(lwoSettings);
    }

    await this.call<grpcTypes.ObfuscationSettings, Empty>(
      this.client.setObfuscationSettings,
      grpcObfuscationSettings,
    );
  }

  public async setWireguardMtu(mtu?: number): Promise<void> {
    await this.callNumber(this.client.setWireguardMtu, mtu);
  }

  public async setWireguardQuantumResistant(quantumResistant: boolean): Promise<void> {
    const quantumResistantState = new grpcTypes.QuantumResistantState();
    switch (quantumResistant) {
      case true:
        quantumResistantState.setState(grpcTypes.QuantumResistantState.State.ON);
        break;
      case false:
        quantumResistantState.setState(grpcTypes.QuantumResistantState.State.OFF);
        break;
    }
    await this.call<grpcTypes.QuantumResistantState, Empty>(
      this.client.setQuantumResistantTunnel,
      quantumResistantState,
    );
  }

  public async setAutoConnect(autoConnect: boolean): Promise<void> {
    await this.callBool(this.client.setAutoConnect, autoConnect);
  }

  public async connectTunnel(): Promise<void> {
    await this.callEmpty(this.client.connectTunnel);
  }

  public async disconnectTunnel(source: DisconnectSource): Promise<void> {
    const prefixedSource = `desktop ${source}`;
    await this.callString(this.client.disconnectTunnel, prefixedSource);
  }

  public async reconnectTunnel(): Promise<void> {
    await this.callEmpty(this.client.reconnectTunnel);
  }

  public async getState(): Promise<TunnelState> {
    const response = await this.callEmpty<grpcTypes.TunnelState>(this.client.getTunnelState);
    return convertFromTunnelState(response)!;
  }

  public async getSettings(): Promise<ISettings> {
    const response = await this.callEmpty<grpcTypes.Settings>(this.client.getSettings);
    return convertFromSettings(response)!;
  }

  public async getAccountHistory(): Promise<WarrenPubKey | undefined> {
    const response = await this.callEmpty<grpcTypes.AccountHistory>(this.client.getAccountHistory);
    return response.getNumber()?.getValue();
  }

  public async clearAccountHistory(): Promise<void> {
    await this.callEmpty(this.client.clearAccountHistory);
  }

  public async getCurrentVersion(): Promise<string> {
    const response = await this.callEmpty<StringValue>(this.client.getCurrentVersion);
    return response.getValue();
  }

  public async setDnsOptions(dns: IDnsOptions): Promise<void> {
    const dnsOptions = new grpcTypes.DnsOptions();

    const defaultOptions = new grpcTypes.DefaultDnsOptions();
    defaultOptions.setBlockAds(dns.defaultOptions.blockAds);
    defaultOptions.setBlockTrackers(dns.defaultOptions.blockTrackers);
    defaultOptions.setBlockMalware(dns.defaultOptions.blockMalware);
    defaultOptions.setBlockAdultContent(dns.defaultOptions.blockAdultContent);
    defaultOptions.setBlockGambling(dns.defaultOptions.blockGambling);
    defaultOptions.setBlockSocialMedia(dns.defaultOptions.blockSocialMedia);
    dnsOptions.setDefaultOptions(defaultOptions);

    const customOptions = new grpcTypes.CustomDnsOptions();
    customOptions.setAddressesList(dns.customOptions.addresses);
    dnsOptions.setCustomOptions(customOptions);

    if (dns.state === 'custom') {
      dnsOptions.setState(grpcTypes.DnsOptions.DnsState.CUSTOM);
    } else {
      dnsOptions.setState(grpcTypes.DnsOptions.DnsState.DEFAULT);
    }

    await this.call<grpcTypes.DnsOptions, Empty>(this.client.setDnsOptions, dnsOptions);
  }

  public async getVersionInfo(): Promise<IAppVersionInfo> {
    const response = await this.callEmpty<grpcTypes.AppVersionInfo>(this.client.getVersionInfo);
    const versionInfo = convertFromAppVersionInfo(response);

    return versionInfo;
  }

  public async addSplitTunnelingApplication(path: string): Promise<void> {
    await this.callString(this.client.addSplitTunnelApp, path);
  }

  public async removeSplitTunnelingApplication(path: string): Promise<void> {
    await this.callString(this.client.removeSplitTunnelApp, path);
  }

  public async setSplitTunnelingState(enabled: boolean): Promise<void> {
    await this.callBool(this.client.setSplitTunnelState, enabled);
  }

  public async splitTunnelIsSupported(): Promise<boolean> {
    try {
      const isSupported = await this.callEmpty<BoolValue>(this.client.splitTunnelIsSupported);
      return isSupported.getValue();
    } catch {
      return false;
    }
  }

  public async needFullDiskPermissions(): Promise<boolean> {
    const needFullDiskPermissions = await this.callEmpty<BoolValue>(
      this.client.needFullDiskPermissions,
    );
    return needFullDiskPermissions.getValue();
  }

  public async checkVolumes(): Promise<void> {
    await this.callEmpty(this.client.checkVolumes);
  }

  public async isPerformingPostUpgrade(): Promise<boolean> {
    const response = await this.callEmpty<BoolValue>(this.client.isPerformingPostUpgrade);
    return response.getValue();
  }

  public async getDevice(): Promise<DeviceState> {
    const response = await this.callEmpty<grpcTypes.DeviceState>(this.client.getDevice);
    return convertFromDeviceState(response);
  }

  public async updateDevice(): Promise<void> {
    await this.callEmpty(this.client.updateDevice);
  }

  public async prepareRestart(quit: boolean) {
    await this.callBool(this.client.prepareRestartV2, quit);
  }

  public async setEnableDaita(value: boolean): Promise<void> {
    await this.callBool(this.client.setEnableDaita, value);
  }

  public async setDaitaDirectOnly(value: boolean): Promise<void> {
    await this.callBool(this.client.setDaitaDirectOnly, value);
  }

  public async listDevices(pubkey: WarrenPubKey): Promise<Array<IDevice>> {
    try {
      const response = await this.callString<grpcTypes.DeviceList>(this.client.listDevices, pubkey);

      return response.getDevicesList().map(convertFromDevice);
    } catch {
      throw new Error('Failed to list devices');
    }
  }

  public async removeDevice(deviceRemoval: IDeviceRemoval): Promise<void> {
    const grpcDeviceRemoval = new grpcTypes.DeviceRemoval();
    // The gRPC field is named `account_number` for wire-format
    // compat but its content is the 64-char hex Warren pubkey.
    grpcDeviceRemoval.setAccountNumber(deviceRemoval.pubkey);
    grpcDeviceRemoval.setDeviceId(deviceRemoval.deviceId);

    await this.call<grpcTypes.DeviceRemoval, Empty>(this.client.removeDevice, grpcDeviceRemoval);
  }

  public async createCustomList(newCustomList: NewCustomList): Promise<void | CustomListError> {
    try {
      await this.call<grpcTypes.NewCustomList, StringValue>(
        this.client.createCustomList,
        convertToNewCustomList(newCustomList),
      );
    } catch (e) {
      const error = e as grpc.ServiceError;
      if (error.code === 6) {
        return { type: 'name already exists' };
      } else {
        throw error;
      }
    }
  }

  public async deleteCustomList(id: string): Promise<void> {
    await this.callString<Empty>(this.client.deleteCustomList, id);
  }

  public async updateCustomList(customList: ICustomList): Promise<void | CustomListError> {
    try {
      await this.call<grpcTypes.CustomList, Empty>(
        this.client.updateCustomList,
        convertToCustomList(customList),
      );
    } catch (e) {
      const error = e as grpc.ServiceError;
      if (error.code === 6) {
        return { type: 'name already exists' };
      } else {
        throw error;
      }
    }
  }

  public async addApiAccessMethod(
    method: NewAccessMethodSetting,
  ): Promise<string | AccessMethodExistsError> {
    try {
      const result = await this.call<grpcTypes.NewAccessMethodSetting, grpcTypes.UUID>(
        this.client.addApiAccessMethod,
        convertToNewApiAccessMethodSetting(method),
      );
      return result.getValue();
    } catch (e) {
      const error = e as grpc.ServiceError;
      if (error.code === 6) {
        return { type: 'name already exists' };
      } else {
        throw error;
      }
    }
  }

  public async updateApiAccessMethod(
    method: AccessMethodSetting,
  ): Promise<void | AccessMethodExistsError> {
    try {
      await this.call(this.client.updateApiAccessMethod, convertToApiAccessMethodSetting(method));
    } catch (e) {
      const error = e as grpc.ServiceError;
      if (error.code === 6) {
        return { type: 'name already exists' };
      } else {
        throw error;
      }
    }
  }

  public async getCurrentApiAccessMethod() {
    const response = await this.callEmpty<grpcTypes.AccessMethodSetting>(
      this.client.getCurrentApiAccessMethod,
    );
    return convertFromApiAccessMethodSetting(response);
  }

  public async removeApiAccessMethod(id: string) {
    const uuid = new grpcTypes.UUID();
    uuid.setValue(id);
    await this.call(this.client.removeApiAccessMethod, uuid);
  }

  public async setApiAccessMethod(id: string) {
    const uuid = new grpcTypes.UUID();
    uuid.setValue(id);
    await this.call(this.client.setApiAccessMethod, uuid);
  }

  public async testApiAccessMethodById(id: string): Promise<boolean> {
    const uuid = new grpcTypes.UUID();
    uuid.setValue(id);
    const result = await this.call<grpcTypes.UUID, BoolValue>(
      this.client.testApiAccessMethodById,
      uuid,
    );
    return result.getValue();
  }

  public async testCustomApiAccessMethod(method: CustomProxy): Promise<boolean> {
    const result = await this.call<grpcTypes.CustomProxy, BoolValue>(
      this.client.testCustomApiAccessMethod,
      convertToCustomProxy(method),
    );
    return result.getValue();
  }

  public async applyJsonSettings(settings: string): Promise<void> {
    await this.callString(this.client.applyJsonSettings, settings);
  }

  public async clearAllRelayOverrides(): Promise<void> {
    await this.callEmpty(this.client.clearAllRelayOverrides);
  }

  public async setEnableRecents(enabled: boolean): Promise<void> {
    const boolValue = new BoolValue();
    boolValue.setValue(enabled);
    await this.call(this.client.setEnableRecents, boolValue);
  }

  private subscriptionId(): number {
    const current = this.nextSubscriptionId;
    this.nextSubscriptionId += 1;
    return current;
  }

  private removeSubscription(id: number) {
    const subscription = this.subscriptions.get(id);
    if (subscription !== undefined) {
      this.subscriptions.delete(id);
      subscription.removeAllListeners('data');
      subscription.removeAllListeners('error');

      subscription.on('error', (e) => {
        const error = e as grpc.ServiceError;
        if (error.code !== grpc.status.CANCELLED) {
          throw error;
        }
      });
      subscription.cancel();
    }
  }
}
