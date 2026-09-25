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

/**
 * The range the exit allocates public ports from
 * (`warren_config::NATPMP_EXTERNAL_PORT_MIN/MAX`, the same bounds the port
 * editor refuses outside of).
 *
 * A re-point writes the exit's number into the rule's `internalPort`, which
 * is the LOCAL port the exit forwards inbound internet traffic to. Without
 * this bound, an exit answering with 22 would have the app open the user's
 * SSH port to the internet, and the rule would survive a restart and follow
 * the user to another exit. The number the exit chooses is the one thing in
 * this flow the user did not.
 */
const EXIT_PORT_MIN = 49152;
const EXIT_PORT_MAX = 65535;

/** Only a failure that can plausibly become a success is worth another try.
 * Credentials and a refused port will not change in two seconds, and
 * qBittorrent bans the calling address after a handful of failed logins,
 * which on the loopback is the user's own. */
function worthRetrying(error: TorrentClientError): boolean {
  return error.kind === 'unreachable' || error.kind === 'bad-response';
}

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
  /** The last port the client took. */
  private syncedPort?: number;
  /** Set while the client is held after an abuse report on its port: the
   * port that was closed. Survives a daemon reconnect, since only the user
   * lifts it. */
  private heldAfterStrike?: number;

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
    if (this.heldAfterStrike !== undefined) {
      if (changes.length > 0) {
        this.emit({ state: 'held', port: this.heldAfterStrike, at: this.deps.now() });
      }
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

  /**
   * A forwarded port was closed after an abuse report. When it is the
   * client's own port (the one it listens on, or the one its rule names), the
   * app stops following port changes: a torrent client that just drew a
   * report and silently gets a new port carries on with what was reported,
   * and three reports revoke the account. The user resumes with "Apply now",
   * having read the warning.
   */
  public onStrike(port: number) {
    const config = this.deps.config();
    if (config === undefined || config.kind === 'none') {
      return;
    }
    const linked = this.linkedRule();
    const ownPorts = [this.syncedPort, linked?.internalPort, this.grantedPort()];
    if (!ownPorts.includes(port)) {
      return;
    }
    this.heldAfterStrike = port;
    this.target = undefined;
    this.emit({ state: 'held', port, at: this.deps.now() });
  }

  /** Writes the port the linked rule holds right now, on the user's command.
   * Answers the status it ended on so the settings view can show it without
   * waiting for the push notification. Lifts a hold after an abuse report:
   * applying a port is the user's decision to carry on. */
  public async applyNow(): Promise<TorrentClientStatus> {
    this.heldAfterStrike = undefined;
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

  /**
   * Re-states where things stand, after a configuration change.
   *
   * A client that is still configured keeps whatever the last push reported:
   * the settings form saves on every blur, and discarding the line the user
   * is reading on each of those would erase the one confirmation the feature
   * gives them.
   */
  public refresh(): TorrentClientStatus {
    const config = this.deps.config();
    if (config === undefined || config.kind === 'none') {
      return this.emit({ state: 'off', at: this.deps.now() });
    }
    if (this.last !== undefined && this.last.state !== 'off') {
      return this.emit(this.last);
    }
    return this.emit({ state: 'waiting', at: this.deps.now() });
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

    const settings = this.deps.natPmpSettings();
    const configured = effectiveNatPmpRules(settings);
    if (!this.acceptableRepoint(port, linked, configured)) {
      this.emit({ state: 'waiting', at: this.deps.now() });
      return;
    }

    const rules = configured.map((candidate) =>
      candidate.internalPort === linked.internalPort && candidate.protocol === linked.protocol
        ? { protocol: candidate.protocol, internalPort: port, suggestedExternalPort: port }
        : candidate,
    );
    try {
      // Writes the rule list as the source of truth and zeroes the legacy
      // single-port fields, the same way the settings view does.
      await this.deps.setNatPmpSettings({
        ...settings,
        rules,
        protocol: NatPmpProto.udp,
        suggestedExternalPort: 0,
        internalPort: 0,
      });
    } catch {
      // Not remembered as re-pointed: a write the daemon refused once must
      // be attempted again on the next grant, or the client never gets a
      // port and nothing says why.
      this.emit({
        state: 'error',
        port,
        error: { kind: 'unreachable' },
        at: this.deps.now(),
      });
      return;
    }
    this.repointed.add(port);
    this.deps.setRule({ internalPort: port, protocol: linked.protocol });
    this.emit({ state: 'waiting', at: this.deps.now() });
  }

  /** Whether the exit's number may become a rule's local port: inside the
   * range the exit allocates from, and not already the identity of another
   * rule (which the exit allocator keys on, so a collision would collapse
   * two forwards into one). */
  private acceptableRepoint(port: number, linked: NatPmpRule, configured: NatPmpRule[]): boolean {
    if (!Number.isInteger(port) || port < EXIT_PORT_MIN || port > EXIT_PORT_MAX) {
      return false;
    }
    return !configured.some(
      (candidate) =>
        candidate.protocol === linked.protocol &&
        candidate.internalPort !== linked.internalPort &&
        candidate.internalPort === port,
    );
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
        const outcome = await this.attempt(current);
        if (this.target !== current) {
          // A newer port arrived, or a reset dropped this one. Either way the
          // loop must not settle on a number that is already stale, and it
          // must not leave `pushing` as the last word: the settings view
          // disables its buttons on that state.
          if (this.target === undefined) {
            this.emit({ state: 'waiting', at: this.deps.now() });
          }
          continue;
        }
        this.target = undefined;
        if (outcome === 'abandoned') {
          this.emit({ state: 'off', at: this.deps.now() });
          continue;
        }
        if (outcome === undefined) {
          this.syncedPort = current;
        }
        this.emit(
          outcome === undefined
            ? { state: 'synced', port: current, at: this.deps.now() }
            : { state: 'error', port: current, error: outcome, at: this.deps.now() },
        );
      }
    } finally {
      this.pushing = false;
    }
  }

  /**
   * One push, with its retries.
   *
   * Answers `undefined` when the client took the port, the sealed error when
   * it would not, and `abandoned` when the user turned the client off while
   * the chain was running: that last case is neither a success nor a failure
   * of the client, and reusing the success value for it made the app report
   * a write it never performed.
   */
  private async attempt(port: number): Promise<TorrentClientError | undefined | 'abandoned'> {
    let failure: TorrentClientError | undefined;
    for (let retry = 0; retry <= PUSH_RETRY_DELAYS_MS.length; retry++) {
      if (retry > 0) {
        await this.deps.sleep(PUSH_RETRY_DELAYS_MS[retry - 1]);
      }
      const config = this.deps.config();
      if (config === undefined || config.kind === 'none') {
        return 'abandoned';
      }
      try {
        await this.deps.adapterFor(config, this.deps.password()).setListenPort(port);
        return undefined;
      } catch (error) {
        failure = toTorrentClientError(error);
      }
      if (!worthRetrying(failure)) {
        return failure;
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
