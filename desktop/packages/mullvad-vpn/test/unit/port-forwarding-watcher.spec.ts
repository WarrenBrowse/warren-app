import { describe, expect, it } from 'vitest';

import { PortForwardingWatcher } from '../../src/main/port-forwarding-watcher';
import { NatPmpProto, NatPmpStatus } from '../../src/shared/daemon-rpc-types';

function mapped(externalPort: number): NatPmpStatus {
  return {
    mappings: [
      {
        internalPort: 58291,
        protocol: NatPmpProto.both,
        status: { state: 'mapped', externalPort, lifetimeGrantedSecs: 3599, windowResetSecs: 60 },
      },
    ],
  };
}

function requesting(): NatPmpStatus {
  return {
    mappings: [
      { internalPort: 58291, protocol: NatPmpProto.both, status: { state: 'requesting' } },
    ],
  };
}

describe('PortForwardingWatcher', () => {
  // The daemon keeps its mappings across a GUI restart and replays them as the
  // first snapshot of the new subscription. Announcing a port the app already
  // announced would tell the user their port "is open" every time they reopen
  // the app, naming a port that has not moved.
  it('says nothing about a first snapshot naming the ports it already announced', () => {
    expect(new PortForwardingWatcher(mapped(58291)).observe(mapped(58291))).toEqual([]);
  });

  // At boot the daemon is a service that connects and gets its port before
  // the GUI process even starts, so the port of the day reaches the GUI as the
  // first snapshot. Taking that snapshot as a silent baseline left the torrent
  // client on yesterday's port and the user without a notification until the
  // next change, typically a change of country (forum topic 226).
  it('announces a port granted while the GUI was closed', () => {
    const changes = new PortForwardingWatcher(mapped(58291)).observe(mapped(49152));

    expect(changes).toHaveLength(1);
    expect(changes[0].port).toBe(49152);
    expect(changes[0].previousPort).toBe(58291);
  });

  it('announces the granted port once when it remembers nothing', () => {
    const changes = new PortForwardingWatcher().observe(mapped(58291));

    expect(changes).toHaveLength(1);
    expect(changes[0].state).toBe('mapped');
    expect(changes[0].port).toBe(58291);
  });

  // A GUI that starts while the daemon is still asking would otherwise
  // announce the remembered port as lost, and then as open again a second
  // later.
  it('does not announce a remembered port as lost on the first snapshot', () => {
    expect(new PortForwardingWatcher(mapped(58291)).observe(requesting())).toEqual([]);
  });

  it('reports what changed between the baseline and the next snapshot', () => {
    const watcher = new PortForwardingWatcher(mapped(58291));
    watcher.observe(mapped(58291));

    const changes = watcher.observe(mapped(49152));

    expect(changes).toHaveLength(1);
    expect(changes[0].port).toBe(49152);
    expect(changes[0].previousPort).toBe(58291);
  });

  it('keeps following the stream after a change', () => {
    const watcher = new PortForwardingWatcher(mapped(58291));
    watcher.observe(mapped(58291));
    watcher.observe(mapped(49152));

    expect(watcher.observe(mapped(49152))).toEqual([]);
    expect(watcher.observe(mapped(40001))).toHaveLength(1);
  });

  // A daemon disconnect tears the subscription down, and the next stream
  // starts with a replay of whatever the daemon holds. A port that moved
  // meanwhile (the daemon restarted and connected elsewhere) is news; the same
  // port is not.
  it('diffs the replay after a reset against the last ports it saw', () => {
    const watcher = new PortForwardingWatcher(mapped(58291));
    watcher.observe(mapped(58291));
    watcher.reset();

    expect(watcher.observe(mapped(58291))).toEqual([]);
    watcher.reset();
    expect(watcher.observe(mapped(49152))).toHaveLength(1);
  });

  it('remembers only the granted ports of the snapshots it saw', () => {
    const watcher = new PortForwardingWatcher();
    watcher.observe(mapped(58291));
    expect(watcher.remembered()).toEqual(mapped(58291));

    watcher.observe(requesting());
    expect(watcher.remembered()).toEqual({ mappings: [] });
  });
});
