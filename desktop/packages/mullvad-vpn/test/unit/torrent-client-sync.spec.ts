import { describe, expect, it } from 'vitest';

import { TorrentClientFailure } from '../../src/main/torrent-client/adapters';
import { planForChange, TorrentClientSync } from '../../src/main/torrent-client/sync';
import {
  NatPmpMapping,
  NatPmpProto,
  NatPmpRule,
  NatPmpSettings,
} from '../../src/shared/daemon-rpc-types';
import { PortChange } from '../../src/shared/port-forwarding-changes';
import {
  ProbeResult,
  TorrentClientConfig,
  TorrentClientError,
  TorrentClientRuleRef,
  TorrentClientStatus,
} from '../../src/shared/torrent-client';

const NOW = 1_700_000_000_000;
const PASSWORD = 'hunter2';

function rule(port: number): NatPmpRule {
  return { protocol: NatPmpProto.both, internalPort: port, suggestedExternalPort: port };
}

/** A rule that let the exit pick the public port, which is the shape that
 * makes the granted port differ from the rule's own. */
function autoRule(port: number): NatPmpRule {
  return { protocol: NatPmpProto.both, internalPort: port, suggestedExternalPort: 0 };
}

function mapped(internalPort: number, port: number): PortChange {
  return {
    internalPort,
    protocol: NatPmpProto.both,
    state: 'mapped',
    port,
    previousPort: undefined,
  };
}

function lost(internalPort: number, previousPort: number): PortChange {
  return {
    internalPort,
    protocol: NatPmpProto.both,
    state: 'lost',
    port: undefined,
    previousPort,
  };
}

function liveMapping(internalPort: number, externalPort: number): NatPmpMapping {
  return {
    internalPort,
    protocol: NatPmpProto.both,
    status: { state: 'mapped', externalPort, lifetimeGrantedSecs: 3600, windowResetSecs: 60 },
  };
}

function deferred() {
  let resolve: () => void = () => undefined;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve: () => resolve() };
}

type SetHandler = (port: number) => Promise<void>;

function fails(error: TorrentClientError): SetHandler {
  return () => Promise.reject(new TorrentClientFailure(error));
}

interface HarnessOptions {
  kind?: TorrentClientConfig['kind'];
  /** Makes the daemon refuse the settings write, the way a gRPC failure
   * during a reconnect does. */
  refuseNatPmpWrites?: boolean;
  rules?: NatPmpRule[];
  configRule?: TorrentClientRuleRef;
  mappings?: NatPmpMapping[];
  handlers?: SetHandler[];
  probe?: ProbeResult | TorrentClientError;
  /** Simulates a daemon that does not apply the settings write, which is the
   * only way the same grant comes back on the same identity twice. */
  ignoreNatPmpWrites?: boolean;
}

function buildHarness(options: HarnessOptions = {}) {
  const statuses: TorrentClientStatus[] = [];
  const sleeps: number[] = [];
  const attempts: number[] = [];
  const passwords: (string | undefined)[] = [];
  const natPmpWrites: NatPmpSettings[] = [];
  const handlers = [...(options.handlers ?? [])];

  let config: TorrentClientConfig = {
    kind: options.kind ?? 'qbittorrent',
    url: 'http://127.0.0.1:8080',
    username: 'alice',
    rule: options.configRule,
  };
  let settings: NatPmpSettings = {
    enabled: true,
    lifetimeSecs: 3600,
    rules: options.rules ?? [rule(6881)],
    protocol: NatPmpProto.udp,
    suggestedExternalPort: 0,
    internalPort: 0,
  };
  let mappings = options.mappings ?? [];

  const sync = new TorrentClientSync({
    config: () => config,
    password: () => PASSWORD,
    natPmpSettings: () => settings,
    setNatPmpSettings: (next) => {
      natPmpWrites.push(next);
      if (options.refuseNatPmpWrites === true) {
        return Promise.reject(new Error('daemon gone'));
      }
      if (options.ignoreNatPmpWrites !== true) {
        settings = next;
      }
      return Promise.resolve();
    },
    natPmpStatus: () => ({ mappings }),
    setRule: (linked) => {
      config = { ...config, rule: linked };
    },
    adapterFor: (_config, password) => {
      passwords.push(password);
      return {
        probe: () => {
          const probe = options.probe ?? { version: 'v5.0.3', listenPort: 6881 };
          return 'kind' in probe
            ? Promise.reject(new TorrentClientFailure(probe))
            : Promise.resolve(probe);
        },
        setListenPort: (port) => {
          attempts.push(port);
          return (handlers.shift() ?? (() => Promise.resolve()))(port);
        },
      };
    },
    onStatus: (status) => statuses.push(status),
    sleep: (ms) => {
      sleeps.push(ms);
      return Promise.resolve();
    },
    now: () => NOW,
  });

  return {
    sync,
    statuses,
    sleeps,
    attempts,
    passwords,
    natPmpWrites,
    get config() {
      return config;
    },
    get settings() {
      return settings;
    },
    setRules: (next: NatPmpRule[]) => {
      settings = { ...settings, rules: next };
    },
    setConfigRule: (linked: TorrentClientRuleRef) => {
      config = { ...config, rule: linked };
    },
    setMappings: (next: NatPmpMapping[]) => {
      mappings = next;
    },
    setKind: (kind: TorrentClientConfig['kind']) => {
      config = { ...config, kind };
    },
  };
}

function lastStatus(statuses: TorrentClientStatus[]): TorrentClientStatus {
  return statuses[statuses.length - 1];
}

describe('the torrent client push plan', () => {
  it('pushes when the granted public port is the one the rule names', () => {
    expect(planForChange(mapped(6881, 6881), rule(6881))).to.equal('push');
  });

  it('re-points when the exit granted another port than the rule names', () => {
    expect(planForChange(mapped(6881, 58291), autoRule(6881))).to.equal('repoint');
  });

  // Nothing better can be written: the client keeps the port it has, which is
  // dead, rather than being pointed at one that is just as dead.
  it('does nothing when the grant is gone', () => {
    expect(planForChange(lost(6881, 6881), rule(6881))).to.equal('ignore');
  });
});

describe('the torrent client controller', () => {
  it('writes the granted port into the client on the linked rule', async () => {
    const harness = buildHarness();

    await harness.sync.onChanges([mapped(6881, 6881)]);

    expect(harness.attempts).to.deep.equal([6881]);
    expect(harness.passwords).to.deep.equal([PASSWORD]);
    expect(harness.statuses).to.deep.equal([
      { state: 'pushing', port: 6881, at: NOW },
      { state: 'synced', port: 6881, at: NOW },
    ]);
  });

  it('links to the first configured rule and ignores the others', async () => {
    const harness = buildHarness({ rules: [rule(6881), rule(7000)] });

    await harness.sync.onChanges([mapped(7000, 7000)]);
    expect(harness.attempts).to.deep.equal([]);

    await harness.sync.onChanges([mapped(6881, 6881)]);
    expect(harness.attempts).to.deep.equal([6881]);
  });

  it('links to the recorded rule when it is still configured', async () => {
    const harness = buildHarness({
      rules: [rule(6881), rule(7000)],
      configRule: { internalPort: 7000, protocol: NatPmpProto.both },
    });

    await harness.sync.onChanges([mapped(7000, 7000)]);

    expect(harness.attempts).to.deep.equal([7000]);
  });

  it('waits instead of pushing when the grant is lost', async () => {
    const harness = buildHarness();

    await harness.sync.onChanges([lost(6881, 6881)]);

    expect(harness.attempts).to.deep.equal([]);
    expect(lastStatus(harness.statuses)).to.deep.equal({ state: 'waiting', at: NOW });
  });

  it('retries on the injected delays before reporting the failure', async () => {
    const unreachable: TorrentClientError = { kind: 'unreachable' };
    const harness = buildHarness({
      handlers: [fails(unreachable), fails(unreachable), fails(unreachable), fails(unreachable)],
    });

    await harness.sync.onChanges([mapped(6881, 6881)]);

    expect(harness.attempts).to.deep.equal([6881, 6881, 6881, 6881]);
    expect(harness.sleeps).to.deep.equal([2_000, 5_000, 10_000]);
    expect(lastStatus(harness.statuses)).to.deep.equal({
      state: 'error',
      port: 6881,
      error: unreachable,
      at: NOW,
    });
  });

  it('stops retrying as soon as the client takes the port', async () => {
    const harness = buildHarness({ handlers: [fails({ kind: 'unreachable' })] });

    await harness.sync.onChanges([mapped(6881, 6881)]);

    expect(harness.attempts).to.deep.equal([6881, 6881]);
    expect(harness.sleeps).to.deep.equal([2_000]);
    expect(lastStatus(harness.statuses).state).to.equal('synced');
  });

  it('does not push the same port twice while a push is in flight', async () => {
    const gate = deferred();
    const harness = buildHarness({ handlers: [() => gate.promise] });

    const inFlight = harness.sync.onChanges([mapped(6881, 6881)]);
    void harness.sync.onChanges([mapped(6881, 6881)]);
    gate.resolve();
    await inFlight;

    expect(harness.attempts).to.deep.equal([6881]);
  });

  it('lets a newer port supersede one still being pushed', async () => {
    const gate = deferred();
    const harness = buildHarness({ handlers: [() => gate.promise] });

    const inFlight = harness.sync.onChanges([mapped(6881, 6881)]);
    harness.setRules([rule(58291)]);
    harness.setConfigRule({ internalPort: 58291, protocol: NatPmpProto.both });
    void harness.sync.onChanges([mapped(58291, 58291)]);
    gate.resolve();
    await inFlight;

    expect(harness.attempts).to.deep.equal([6881, 58291]);
    expect(lastStatus(harness.statuses)).to.deep.equal({
      state: 'synced',
      port: 58291,
      at: NOW,
    });
  });

  it('re-points the rule onto the granted port and waits for the next grant', async () => {
    const harness = buildHarness({ rules: [autoRule(6881)] });

    await harness.sync.onChanges([mapped(6881, 58291)]);

    expect(harness.attempts).to.deep.equal([]);
    expect(harness.settings.rules).to.deep.equal([
      { protocol: NatPmpProto.both, internalPort: 58291, suggestedExternalPort: 58291 },
    ]);
    expect(harness.config.rule).to.deep.equal({
      internalPort: 58291,
      protocol: NatPmpProto.both,
    });
    expect(lastStatus(harness.statuses)).to.deep.equal({ state: 'waiting', at: NOW });

    await harness.sync.onChanges([mapped(58291, 58291)]);
    expect(harness.attempts).to.deep.equal([58291]);
  });

  // A grant that flaps (lost, then granted again on the same port) replays the
  // same change, and a controller that re-pointed on each one would rewrite
  // the daemon settings for as long as the flapping lasts.
  it('re-points at most once per granted port', async () => {
    const harness = buildHarness({ rules: [autoRule(6881)], ignoreNatPmpWrites: true });

    await harness.sync.onChanges([mapped(6881, 58291)]);
    await harness.sync.onChanges([mapped(6881, 58291)]);

    expect(harness.natPmpWrites).to.have.length(1);
  });

  it('forgets the re-point guard when the daemon goes away', async () => {
    const harness = buildHarness({ rules: [autoRule(6881)], ignoreNatPmpWrites: true });

    await harness.sync.onChanges([mapped(6881, 58291)]);
    harness.sync.reset();
    await harness.sync.onChanges([mapped(6881, 58291)]);

    expect(harness.natPmpWrites).to.have.length(2);
  });

  it('applies the port the linked rule currently holds', async () => {
    const harness = buildHarness({ mappings: [liveMapping(6881, 6881)] });

    const status = await harness.sync.applyNow();

    expect(harness.attempts).to.deep.equal([6881]);
    expect(status).to.deep.equal({ state: 'synced', port: 6881, at: NOW });
  });

  it('answers waiting when the linked rule holds no port', async () => {
    const harness = buildHarness({ mappings: [] });

    const status = await harness.sync.applyNow();

    expect(harness.attempts).to.deep.equal([]);
    expect(status).to.deep.equal({ state: 'waiting', at: NOW });
  });

  it('surfaces what the probe found', async () => {
    const harness = buildHarness({ probe: { version: 'v5.0.3', listenPort: 6881 } });

    expect(await harness.sync.testConnection()).to.deep.equal({
      version: 'v5.0.3',
      listenPort: 6881,
    });
  });

  it('surfaces a failed probe as the sealed error', async () => {
    const harness = buildHarness({ probe: { kind: 'login-refused' } });

    expect(await harness.sync.testConnection()).to.deep.equal({ kind: 'login-refused' });
  });

  it('does nothing at all while no client is configured', async () => {
    const harness = buildHarness({ kind: 'none' });

    await harness.sync.onChanges([mapped(6881, 6881)]);

    expect(harness.attempts).to.deep.equal([]);
    expect(harness.statuses).to.deep.equal([]);
  });

  it('reports off with no client and waiting with one', () => {
    expect(buildHarness({ kind: 'none' }).sync.refresh()).to.deep.equal({
      state: 'off',
      at: NOW,
    });
    expect(buildHarness().sync.refresh()).to.deep.equal({ state: 'waiting', at: NOW });
  });

  // The exit chooses this number, and it lands in the rule's `internalPort`,
  // which is the local port the exit forwards inbound internet traffic to. An
  // exit that answered with 22 would otherwise have the app open the user's
  // SSH port to the internet, a number the port editor would have refused.
  it('refuses to re-point onto a port outside the range the exit allocates from', async () => {
    const harness = buildHarness({ rules: [autoRule(6881)] });

    await harness.sync.onChanges([mapped(6881, 22)]);

    expect(harness.natPmpWrites).to.deep.equal([]);
    expect(harness.config.rule).to.equal(undefined);
    expect(lastStatus(harness.statuses)).to.deep.equal({ state: 'waiting', at: NOW });
  });

  // The rule identity the exit allocator keys on is `(internalPort,
  // protocol)`, so re-pointing onto a number another rule already holds would
  // collapse two forwards into one.
  it('refuses to re-point onto a port another rule already holds', async () => {
    const harness = buildHarness({ rules: [autoRule(6881), rule(58291)] });

    await harness.sync.onChanges([mapped(6881, 58291)]);

    expect(harness.natPmpWrites).to.deep.equal([]);
    expect(harness.attempts).to.deep.equal([]);
  });

  it('does not latch the re-point guard when the daemon refuses the write', async () => {
    const harness = buildHarness({ rules: [autoRule(6881)], refuseNatPmpWrites: true });

    await harness.sync.onChanges([mapped(6881, 58291)]);
    await harness.sync.onChanges([mapped(6881, 58291)]);

    expect(harness.natPmpWrites).to.have.length(2);
    expect(lastStatus(harness.statuses).state).to.equal('error');
  });

  // qBittorrent bans the calling address after a handful of failed logins,
  // and on the loopback that address is the user's own. A password that is
  // wrong now will still be wrong in two seconds.
  it('does not retry a refused login', async () => {
    const harness = buildHarness({
      handlers: [fails({ kind: 'login-refused' }), fails({ kind: 'login-refused' })],
    });

    await harness.sync.onChanges([mapped(6881, 6881)]);

    expect(harness.attempts).to.deep.equal([6881]);
    expect(harness.sleeps).to.deep.equal([]);
    expect(lastStatus(harness.statuses)).to.deep.equal({
      state: 'error',
      port: 6881,
      error: { kind: 'login-refused' },
      at: NOW,
    });
  });

  it('does not retry a port the client refused', async () => {
    const harness = buildHarness({
      handlers: [
        fails({ kind: 'rejected', detail: 'no' }),
        fails({ kind: 'rejected', detail: 'no' }),
      ],
    });

    await harness.sync.onChanges([mapped(6881, 6881)]);

    expect(harness.attempts).to.deep.equal([6881]);
    expect(harness.sleeps).to.deep.equal([]);
  });

  it('reports off, not synced, when the client is turned off mid-retry', async () => {
    const harness = buildHarness({ handlers: [fails({ kind: 'unreachable' })] });
    const turnOff = () => harness.setKind('none');

    const inFlight = harness.sync.onChanges([mapped(6881, 6881)]);
    turnOff();
    await inFlight;

    expect(lastStatus(harness.statuses)).to.deep.equal({ state: 'off', at: NOW });
  });

  // `reset()` fires on every daemon disconnect, which is exactly when the
  // mappings churn. A push abandoned there used to leave the last word on
  // `pushing`, which disables the settings buttons for good.
  it('says where it ended when a reset drops an in-flight push', async () => {
    const gate = deferred();
    const harness = buildHarness({ handlers: [() => gate.promise] });

    const inFlight = harness.sync.onChanges([mapped(6881, 6881)]);
    harness.sync.reset();
    gate.resolve();
    await inFlight;

    expect(lastStatus(harness.statuses)).to.deep.equal({ state: 'waiting', at: NOW });
  });

  it('keeps the last status when a save leaves the client configured', async () => {
    const harness = buildHarness({ mappings: [liveMapping(6881, 6881)] });
    await harness.sync.applyNow();

    const refreshed = harness.sync.refresh();

    expect(refreshed).to.deep.equal({ state: 'synced', port: 6881, at: NOW });
  });

  // The status travels to the renderer and into any report a user pastes.
  it('never carries the password into a status', async () => {
    const harness = buildHarness({
      handlers: [
        fails({ kind: 'rejected', detail: 'no' }),
        fails({ kind: 'rejected', detail: 'no' }),
        fails({ kind: 'rejected', detail: 'no' }),
        fails({ kind: 'rejected', detail: 'no' }),
      ],
    });

    await harness.sync.onChanges([mapped(6881, 6881)]);

    expect(harness.passwords).to.contain(PASSWORD);
    expect(JSON.stringify(harness.statuses)).to.not.contain(PASSWORD);
  });
});
