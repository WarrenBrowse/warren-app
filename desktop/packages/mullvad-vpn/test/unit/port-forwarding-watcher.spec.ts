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

describe('PortForwardingWatcher', () => {
  // The daemon keeps its mappings across a GUI restart and replays them as the
  // first snapshot of the new subscription. Announcing that would tell the user
  // their port "is open" every time they reopen the app, naming a port that has
  // not moved.
  it('says nothing about the first snapshot it sees', () => {
    expect(new PortForwardingWatcher().observe(mapped(58291))).toEqual([]);
  });

  it('reports what changed between the baseline and the next snapshot', () => {
    const watcher = new PortForwardingWatcher();
    watcher.observe(mapped(58291));

    const changes = watcher.observe(mapped(49152));

    expect(changes).toHaveLength(1);
    expect(changes[0].port).toBe(49152);
    expect(changes[0].previousPort).toBe(58291);
  });

  it('keeps following the stream after a change', () => {
    const watcher = new PortForwardingWatcher();
    watcher.observe(mapped(58291));
    watcher.observe(mapped(49152));

    expect(watcher.observe(mapped(49152))).toEqual([]);
    expect(watcher.observe(mapped(40001))).toHaveLength(1);
  });

  // A daemon disconnect tears the subscription down. The next stream starts
  // with a replay of whatever the daemon holds, which is a baseline again and
  // not news.
  it('takes the next snapshot as a baseline again after a reset', () => {
    const watcher = new PortForwardingWatcher();
    watcher.observe(mapped(58291));
    watcher.reset();

    expect(watcher.observe(mapped(49152))).toEqual([]);
  });
});
