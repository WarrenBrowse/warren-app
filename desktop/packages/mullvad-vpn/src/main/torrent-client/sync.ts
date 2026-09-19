import {
  effectiveNatPmpRules,
  NatPmpProto,
  NatPmpRule,
  NatPmpSettings,
  NatPmpStatus,
} from '../../shared/daemon-rpc-types';
import { PortChange } from '../../shared/port-forwarding-changes';
import {
  ProbeResult,
  TorrentClientConfig,
  TorrentClientError,
  TorrentClientRuleRef,
  TorrentClientStatus,
} from '../../shared/torrent-client';
import { TorrentClientAdapter, toTorrentClientError } from './adapters';

/**
 * How long the controller waits between two attempts at the same port.
 *
 * One attempt plus these three retries, spread over 17 s. A torrent client is
 * usually restarting when it refuses the first write (the user restarted it,
 * or the machine just woke), and the delays grow so a client that needs a
 * moment gets one without the app hammering a socket that is not there.
 */
const PUSH_RETRY_DELAYS_MS = [2_000, 5_000, 10_000];

export interface TorrentClientSyncDeps {
  config: () => TorrentClientConfig | undefined;
  password: () => string | undefined;
  natPmpSettings: () => NatPmpSettings;
  setNatPmpSettings: (settings: NatPmpSettings) => Promise<void>;
  natPmpStatus: () => NatPmpStatus | undefined;
  setRule: (rule: TorrentClientRuleRef) => void;
  adapterFor: (config: TorrentClientConfig, password: string | undefined) => TorrentClientAdapter;
  onStatus: (status: TorrentClientStatus) => void;
  sleep: (ms: number) => Promise<void>;
  now: () => number;
}

/**
 * What to do about one port change on the linked rule.
 *
 * `repoint` is the case where the exit granted a public port other than the
 * one the rule names, which is what happens to a rule that let the exit pick.
 * The app's own model is one number ("the port you picked is the port that
 * opens"), and a torrent client has one incoming-port field, so the rule is
 * moved onto the granted port rather than the client being told to listen on
 * one number and announce another.
 *
 * Pure: no clock, no I/O.
 */
export function planForChange(change: PortChange, rule: NatPmpRule): 'push' | 'repoint' | 'ignore' {
  if (change.state !== 'mapped' || change.port === undefined) {
    return 'ignore';
  }
  return change.port === rule.internalPort ? 'push' : 'repoint';
}

/**
 * Keeps a torrent client's incoming port equal to the public port the exit
 * currently forwards.
 *
 * Fed the same `PortChange[]` the desktop notification consumes, so the app
 * can never tell the user one port and the client another. Everything it
 * touches is injected, which is what lets the whole decision table be tested
 * without a daemon, a clock or a socket.
 */
export class TorrentClientSync {
  /** The port the controller is currently trying to write. A newer one
   * replaces it while a push is in flight, and the loop picks it up rather
   * than finishing on a number that is already stale. */
  private target?: number;
  private pushing = false;
  /** Granted ports already re-pointed onto. A grant that flaps replays the
   * same change, and re-pointing on each one would rewrite the daemon
   * settings for as long as the flapping lasts. */
  private readonly repointed = new Set<number>();
  private last?: TorrentClientStatus;

  public constructor(private readonly deps: TorrentClientSyncDeps) {}

  /** Acts on the one change that concerns the linked rule. Returns once the
   * work it started is done, which is what the specs await; the main process
   * fires it and forgets it. */
  public async onChanges(changes: PortChange[]): Promise<void> {
    const config = this.deps.config();
    if (config === undefined || config.kind === 'none') {
      return;
    }
    const linked = this.linkedRule();
    if (linked === undefined) {
      return;
    }
    const change = changes.find(
      (candidate) =>
        candidate.internalPort === linked.internalPort && candidate.protocol === linked.protocol,
    );
    if (change === undefined) {
      return;
    }

    switch (planForChange(change, linked)) {
      case 'push':
        await this.push(change.port as number);
        return;
      case 'repoint':
        await this.repoint(linked, change.port as number);
        return;
      default:
        this.emit({ state: 'waiting', at: this.deps.now() });
    }
  }

  /** Writes the port the linked rule holds right now, on the user's command.
   * Answers the status it ended on so the settings view can show it without
   * waiting for the push notification. */
  public async applyNow(): Promise<TorrentClientStatus> {
    const config = this.deps.config();
    if (config === undefined || config.kind === 'none') {
      return this.emit({ state: 'off', at: this.deps.now() });
    }
    const port = this.grantedPort();
    if (port === undefined) {
      return this.emit({ state: 'waiting', at: this.deps.now() });
    }
    await this.push(port);
    return this.last ?? this.emit({ state: 'waiting', at: this.deps.now() });
  }

  /** Asks the client who it is and what it listens on, without writing
   * anything. The one call the settings form makes while the user is still
   * typing the address. */
  public async testConnection(): Promise<ProbeResult | TorrentClientError> {
    const config = this.deps.config();
    if (config === undefined || config.kind === 'none') {
      return { kind: 'unreachable' };
    }
    try {
      return await this.deps.adapterFor(config, this.deps.password()).probe();
    } catch (error) {
      return toTorrentClientError(error);
    }
  }

  /** Re-states where things stand, after a configuration change. */
  public refresh(): TorrentClientStatus {
    const config = this.deps.config();
    const off = config === undefined || config.kind === 'none';
    return this.emit({ state: off ? 'off' : 'waiting', at: this.deps.now() });
  }

  /** Drops the in-flight state, the way a daemon disconnect drops the
   * watcher's baseline: the mappings that come back after a reconnect are a
   * fresh start, and a re-point guard held over from the previous connection
   * would silently refuse the first one of the new. */
  public reset() {
    this.target = undefined;
    this.repointed.clear();
  }

  /** The rule the client is bound to: the recorded one while it is still
   * configured, otherwise the first rule there is. A user with one rule never
   * has to pick, and one who removed the linked rule keeps a working feature
   * rather than a dangling link. */
  private linkedRule(): NatPmpRule | undefined {
    const rules = effectiveNatPmpRules(this.deps.natPmpSettings());
    const recorded = this.deps.config()?.rule;
    if (recorded !== undefined) {
      const match = rules.find(
        (candidate) =>
          candidate.internalPort === recorded.internalPort &&
          candidate.protocol === recorded.protocol,
      );
      if (match !== undefined) {
        return match;
      }
    }
    return rules[0];
  }

  private grantedPort(): number | undefined {
    const linked = this.linkedRule();
    if (linked === undefined) {
      return undefined;
    }
    const mapping = (this.deps.natPmpStatus()?.mappings ?? []).find(
      (candidate) =>
        candidate.internalPort === linked.internalPort && candidate.protocol === linked.protocol,
    );
    return mapping?.status.state === 'mapped' ? mapping.status.externalPort : undefined;
  }

  private async repoint(linked: NatPmpRule, port: number): Promise<void> {
    if (this.repointed.has(port)) {
      return;
    }
    this.repointed.add(port);

    const settings = this.deps.natPmpSettings();
    const rules = effectiveNatPmpRules(settings).map((candidate) =>
      candidate.internalPort === linked.internalPort && candidate.protocol === linked.protocol
        ? { protocol: candidate.protocol, internalPort: port, suggestedExternalPort: port }
        : candidate,
    );
    // Writes the rule list as the source of truth and zeroes the legacy
    // single-port fields, the same way the settings view does.
    await this.deps.setNatPmpSettings({
      ...settings,
      rules,
      protocol: NatPmpProto.udp,
      suggestedExternalPort: 0,
      internalPort: 0,
    });
    this.deps.setRule({ internalPort: port, protocol: linked.protocol });
    this.emit({ state: 'waiting', at: this.deps.now() });
  }

  /**
   * Writes one port, and keeps writing whichever port is current.
   *
   * A single loop owns the whole push, so two changes arriving a second apart
   * cannot both open a session on the client. The loop re-reads `target` after
   * every attempt: a port that moved while an attempt was in the air is
   * written next rather than the app settling on the stale one.
   */
  private async push(port: number): Promise<void> {
    this.target = port;
    if (this.pushing) {
      return;
    }
    this.pushing = true;
    try {
      while (this.target !== undefined) {
        const current: number = this.target;
        this.emit({ state: 'pushing', port: current, at: this.deps.now() });
        const failure = await this.attempt(current);
        if (this.target !== current) {
          continue;
        }
        this.target = undefined;
        this.emit(
          failure === undefined
            ? { state: 'synced', port: current, at: this.deps.now() }
            : { state: 'error', port: current, error: failure, at: this.deps.now() },
        );
      }
    } finally {
      this.pushing = false;
    }
  }

  private async attempt(port: number): Promise<TorrentClientError | undefined> {
    let failure: TorrentClientError | undefined;
    for (let retry = 0; retry <= PUSH_RETRY_DELAYS_MS.length; retry++) {
      if (retry > 0) {
        await this.deps.sleep(PUSH_RETRY_DELAYS_MS[retry - 1]);
      }
      const config = this.deps.config();
      if (config === undefined || config.kind === 'none') {
        return undefined;
      }
      try {
        await this.deps.adapterFor(config, this.deps.password()).setListenPort(port);
        return undefined;
      } catch (error) {
        failure = toTorrentClientError(error);
      }
      // A port that moved again makes the rest of this chain pointless: the
      // loop above will start over on the new one.
      if (this.target !== port) {
        return failure;
      }
    }
    return failure;
  }

  private emit(status: TorrentClientStatus): TorrentClientStatus {
    this.last = status;
    this.deps.onStatus(status);
    return status;
  }
}
