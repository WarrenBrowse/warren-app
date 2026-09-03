import * as grpc from '@grpc/grpc-js';
import { Empty } from 'google-protobuf/google/protobuf/empty_pb.js';
import { BoolValue, StringValue } from 'google-protobuf/google/protobuf/wrappers_pb.js';
import * as grpcTypes from 'management-interface/management-interface/grpc-types';

import {
  AccessMethodExistsError,
  AccessMethodSetting,
  AccountDataResponse,
  CustomListError,
  CustomProxy,
  DaemonAppUpgradeEvent,
  DaemonEvent,
  DeviceState,
  DisconnectSource,
  IAppVersionInfo,
  ICustomList,
  IDnsOptions,
  IRelayListWithEndpointData,
  ISettings,
  LogoutSource,
  NatPmpSettings,
  NatPmpStatus,
  NewAccessMethodSetting,
  NewCustomList,
  ObfuscationSettings,
  ObfuscationType,
  RelaySettings,
  TrustNewExitKeyOutcome,
  TunnelState,
  VoucherResponse,
  WarrenCustomExitSettings,
  WarrenMultiHopSettings,
  WarrenPubKey,
  WarrenPubkeyMismatch,
  WarrenStatus,
} from '../shared/daemon-rpc-types';
import { daemonRpcPath } from './daemon-rpc-path';
import { ConnectionObserver, GrpcClient, noConnectionError } from './grpc-client';
import {
  convertFromApiAccessMethodSetting,
  convertFromAppUpgradeEvent,
  convertFromAppVersionInfo,
  convertFromDaemonEvent,
  convertFromDeviceState,
  convertFromNatPmpSettings,
  convertFromNatPmpStatus,
  convertFromRelayList,
  convertFromSettings,
  convertFromTunnelState,
  convertFromWarrenStatus,
  convertToApiAccessMethodSetting,
  convertToCustomList,
  convertToCustomProxy,
  convertToNatPmpSettings,
  convertToNewApiAccessMethodSetting,
  convertToNewCustomList,
  convertToRelayConstraints,
  convertToWarrenCustomExitSettings,
  convertToWarrenMultiHopSettings,
  ensureExists,
} from './grpc-type-convertions';

const DAEMON_RPC_PATH = daemonRpcPath(
  process.platform,
  process.env,
  process.env.NODE_ENV === 'development',
);

/**
 * The signed community-forum login material returned by the daemon: the
 * four X-Warren-* header values plus the JSON body to POST to the forum
 * connect host (doc 55). None of these is a long-lived secret.
 */
export interface ForumLoginSignature {
  pubkeySs58: string;
  signatureHex: string;
  timestamp: number;
  nonceHex: string;
  body: string;
}

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
      | grpcTypes.DaemonEvent
      | grpcTypes.AppUpgradeEvent
      | grpcTypes.WarrenStatus
      | grpcTypes.NatPmpStatus
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
          // The daemon maps a 404 from warren-api (no active
          // subscription for the current pubkey) to gRPC NOT_FOUND.
          // The cache translates this into an expired-account
          // Redux state so the UI redirects to the "buy plan"
          // screen instead of letting the user click Connect.
          case grpc.status.NOT_FOUND:
            return { type: 'error', error: 'no-subscription' };
          default:
            return { type: 'error', error: 'communication' };
        }
      }
      throw error;
    }
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
   * validates BIP39, writes atomically, then hot-swaps the in-memory
   * `WarrenAuthSigner` and triggers an `account_manager.login(new_pubkey)`
   * so the new identity is active without restarting the daemon. The
   * GUI observes the resulting `deviceState: 'logged in'` change and
   * proceeds normally. Throws `grpc.ServiceError`
   * (status INVALID_ARGUMENT) if the BIP39 input is invalid.
   */
  public async setWarrenMnemonic(mnemonic: string): Promise<void> {
    await this.callString<Empty>(this.client.setWarrenMnemonic, mnemonic);
  }

  /**
   * Signs a community-forum login challenge (doc 55, DiscourseConnect
   * wallet SSO). `sid` comes from a `warren://forum-login?sid=..` deep
   * link. The daemon signs the fixed `POST /v1/forum/login` request with
   * the Warren identity key and returns the four X-Warren-* header values
   * plus the JSON body to POST. The signing key never leaves the daemon.
   */
  public async signForumLogin(sid: string): Promise<ForumLoginSignature> {
    const request = new grpcTypes.ForumLoginRequest();
    request.setSid(sid);
    const response = await this.call<grpcTypes.ForumLoginRequest, grpcTypes.ForumLoginSignature>(
      this.client.signForumLogin,
      request,
    );
    return {
      pubkeySs58: response.getPubkeySs58(),
      signatureHex: response.getSignatureHex(),
      timestamp: response.getTimestamp(),
      nonceHex: response.getNonceHex(),
      body: response.getBody(),
    };
  }

  /**
   * Signs a community-forum notification read (doc 55). Takes no argument:
   * the account read is derived from the signature, so there is nothing to
   * point at somebody else. Returns the four X-Warren-* header values plus
   * the exact body to POST verbatim. The signing key never leaves the
   * daemon.
   */
  public async signForumNotifications(): Promise<ForumLoginSignature> {
    const response = await this.call<Empty, grpcTypes.ForumLoginSignature>(
      this.client.signForumNotifications,
      new Empty(),
    );
    return {
      pubkeySs58: response.getPubkeySs58(),
      signatureHex: response.getSignatureHex(),
      timestamp: response.getTimestamp(),
      nonceHex: response.getNonceHex(),
      body: response.getBody(),
    };
  }

  /**
   * Signs marking the caller's own forum notification list as seen, which is
   * what the forum bell does by itself. Its own signature rather than a reuse
   * of the read above: a signature is bound to one method and one path, so a
   * read can never be replayed as this write.
   */
  public async signForumNotificationsSeen(): Promise<ForumLoginSignature> {
    const response = await this.call<Empty, grpcTypes.ForumLoginSignature>(
      this.client.signForumNotificationsSeen,
      new Empty(),
    );
    return {
      pubkeySs58: response.getPubkeySs58(),
      signatureHex: response.getSignatureHex(),
      timestamp: response.getTimestamp(),
      nonceHex: response.getNonceHex(),
      body: response.getBody(),
    };
  }

  /**
   * Signs a community-forum attach-logs request (doc 55). `sid` and
   * `topicId` come from a `warren://attach-logs?sid=..&topic=..` deep link
   * and `logGz` is the gzipped redacted problem report. The daemon builds
   * the canonical `POST /v1/forum/attach-logs` JSON body itself, signs it
   * with the Warren identity key, and returns the four X-Warren-* header
   * values plus that exact body, which must be POSTed verbatim so the
   * signed bytes and the sent bytes are identical. The signing key never
   * leaves the daemon.
   */
  public async signForumAttachLogs(
    sid: string,
    topicId: number,
    logGz: Uint8Array,
  ): Promise<ForumLoginSignature> {
    const request = new grpcTypes.ForumAttachLogsRequest();
    request.setSid(sid);
    request.setTopicId(topicId);
    request.setLogGz(logGz);
    const response = await this.call<
      grpcTypes.ForumAttachLogsRequest,
      grpcTypes.ForumLoginSignature
    >(this.client.signForumAttachLogs, request);
    return {
      pubkeySs58: response.getPubkeySs58(),
      signatureHex: response.getSignatureHex(),
      timestamp: response.getTimestamp(),
      nonceHex: response.getNonceHex(),
      body: response.getBody(),
    };
  }

  /**
   * Signs a community-forum in-app report (doc 55). `reportJson` is the
   * form's fields as one JSON object in the connect contract's names and
   * `logGz` the gzipped redacted problem report (empty for a report filed
   * without logs). The daemon builds the canonical `POST /v1/forum/report`
   * body through the crate the mobile clients sign with, signs it with the
   * Warren identity key, and returns the four X-Warren-* header values plus
   * that exact body, which must be POSTed verbatim so the signed bytes and
   * the sent bytes are identical. The signing key never leaves the daemon.
   */
  public async signForumReport(
    reportJson: string,
    logGz: Uint8Array,
  ): Promise<ForumLoginSignature> {
    const request = new grpcTypes.ForumReportRequest();
    request.setReportJson(reportJson);
    request.setLogGz(logGz);
    const response = await this.call<grpcTypes.ForumReportRequest, grpcTypes.ForumLoginSignature>(
      this.client.signForumReport,
      request,
    );
    return {
      pubkeySs58: response.getPubkeySs58(),
      signatureHex: response.getSignatureHex(),
      timestamp: response.getTimestamp(),
      nonceHex: response.getNonceHex(),
      body: response.getBody(),
    };
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
          case grpc.status.FAILED_PRECONDITION:
            return { type: 'expired' };
          // Also emitted on daemon-transport failures: both mean
          // "nothing definitive happened, retry later".
          case grpc.status.UNAVAILABLE:
            return { type: 'not_ready' };
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

  // Persistent warren-api URL. Empty string → unset on the daemon
  // side (= fallback to upstream Mullvad backend). Daemon restart is
  // required to apply (see `resolve_warren_api_config` on the Rust
  // side, which reads Settings at boot only).
  public async setWarrenApiUrl(url: string): Promise<void> {
    await this.callString(this.client.setWarrenApiUrl, url);
  }

  // Client-side bandwidth ceiling in bits per second. `undefined`
  // maps to 0 on the wire = unset (unlimited). Applies to a live
  // tunnel without a reconnect.
  public async setWarrenMaxRateBps(bps?: number): Promise<void> {
    await this.callNumber64(this.client.setWarrenMaxRateBps, bps ?? 0);
  }

  // Warren multi-hop settings. Restart daemon required to
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

  public async setWarrenCustomExit(settings: WarrenCustomExitSettings): Promise<void> {
    const proto = convertToWarrenCustomExitSettings(settings);
    await this.call<grpcTypes.WarrenCustomExitSettings, Empty>(
      this.client.setWarrenCustomExit,
      proto,
    );
  }

  public async getWarrenStatus(): Promise<WarrenStatus> {
    const response = await this.callEmpty<grpcTypes.WarrenStatus>(this.client.getWarrenStatus);
    return convertFromWarrenStatus(response);
  }

  // Warren NAT-PMP port-forwarding. The setter pushes the value live
  // to the daemon, which both persists it AND updates the running
  // parameters generator so the next tunnel reconnect spawns (or
  // stops) the refresh loop.
  public async getNatPmpSettings(): Promise<NatPmpSettings> {
    const response = await this.callEmpty<grpcTypes.NatPmpSettings>(this.client.getNatPmpSettings);
    return convertFromNatPmpSettings(response);
  }

  public async setNatPmpSettings(settings: NatPmpSettings): Promise<void> {
    const proto = convertToNatPmpSettings(settings);
    await this.call<grpcTypes.NatPmpSettings, Empty>(this.client.setNatPmpSettings, proto);
  }

  // TOFU pubkey-pinning user actions. The daemon-side
  // verify hook keeps the in-memory pin table; these RPCs let the
  // user resolve a pending mismatch from the modal.
  public async trustNewExitKey(input: {
    exitIdHex: string;
    newPubkeyHex: string;
  }): Promise<TrustNewExitKeyOutcome> {
    const req = new grpcTypes.TrustNewExitKeyRequest();
    req.setExitIdHex(input.exitIdHex);
    req.setNewPubkeyHex(input.newPubkeyHex);
    const response = await this.call<
      grpcTypes.TrustNewExitKeyRequest,
      grpcTypes.TrustNewExitKeyResponse
    >(this.client.trustNewExitKey, req);
    switch (response.getResult()) {
      case grpcTypes.TrustNewExitKeyResponse.Result.OK:
        return { result: 'ok' };
      case grpcTypes.TrustNewExitKeyResponse.Result.EXIT_NOT_FOUND:
        return { result: 'exit-not-found' };
      case grpcTypes.TrustNewExitKeyResponse.Result.PUBKEY_MISMATCH:
        return { result: 'pubkey-mismatch' };
      default:
        return { result: 'io-error', errorMessage: response.getErrorMessage() };
    }
  }

  public async resetPinnedExitKeys(): Promise<number> {
    const response = await this.callEmpty<grpcTypes.ResetPinnedExitKeysResponse>(
      this.client.resetPinnedExitKeys,
    );
    return response.getResetCount();
  }

  public async dismissPubkeyMismatch(): Promise<void> {
    await this.callEmpty<Empty>(this.client.dismissPubkeyMismatch);
  }

  public async reportPubkeyMismatch(mismatch: WarrenPubkeyMismatch): Promise<void> {
    const req = new grpcTypes.ReportPubkeyMismatchRequest();
    req.setExitIdHex(mismatch.exitIdHex);
    req.setOldPubkeyHex(mismatch.pinnedPubkeyHex);
    req.setNewPubkeyHex(mismatch.observedPubkeyHex);
    req.setCountryCode(mismatch.countryCode);
    req.setCity(mismatch.city);
    await this.call<grpcTypes.ReportPubkeyMismatchRequest, Empty>(
      this.client.reportPubkeyMismatch,
      req,
    );
  }

  // Push stream subscription mirroring `subscribeWarrenStatusListener`.
  // Forwards every refresh-loop event (Mapped / Renewed / Failed /
  // Cancelled) as a renderer-facing `NatPmpStatus` so the
  // port-forwarding view updates without polling.
  public subscribeNatPmpStatusListener(listener: SubscriptionListener<NatPmpStatus>) {
    const call = this.isConnected && this.client.natPmpStatusUpdates(new Empty());
    if (!call) {
      throw noConnectionError;
    }
    const subscriptionId = this.subscriptionId();
    listener.subscriptionId = subscriptionId;
    this.subscriptions.set(subscriptionId, call);

    call.on('data', (data: grpcTypes.NatPmpStatus) => {
      try {
        listener.onEvent(convertFromNatPmpStatus(data));
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

  public unsubscribeNatPmpStatusListener(listener: SubscriptionListener<NatPmpStatus>) {
    const id = listener.subscriptionId;
    if (id !== undefined) {
      this.removeSubscription(id);
    }
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

    dnsOptions.setAllowExternalDns(dns.allowExternalDns);

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

  public async prepareRestart(quit: boolean) {
    await this.callBool(this.client.prepareRestartV2, quit);
  }

  public async setEnableDaita(value: boolean): Promise<void> {
    await this.callBool(this.client.setEnableDaita, value);
  }

  public async setDaitaDirectOnly(value: boolean): Promise<void> {
    await this.callBool(this.client.setDaitaDirectOnly, value);
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
