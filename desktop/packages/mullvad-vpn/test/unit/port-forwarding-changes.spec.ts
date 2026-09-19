import { describe, expect, it } from 'vitest';

import { NatPmpMapping, NatPmpProto, NatPmpStatus } from '../../src/shared/daemon-rpc-types';
import { portForwardingChanges } from '../../src/shared/port-forwarding-changes';

function mapped(internalPort: number, protocol: NatPmpProto, externalPort: number): NatPmpMapping {
  return {
    internalPort,
    protocol,
    status: { state: 'mapped', externalPort, lifetimeGrantedSecs: 3599, windowResetSecs: 60 },
  };
}

function requesting(internalPort: number, protocol: NatPmpProto): NatPmpMapping {
  return { internalPort, protocol, status: { state: 'requesting' } };
}

function status(...mappings: NatPmpMapping[]): NatPmpStatus {
  return { mappings };
}

describe('portForwardingChanges', () => {
  it('fires mapped on a first grant', () => {
    const changes = portForwardingChanges(
      status(requesting(58291, NatPmpProto.both)),
      status(mapped(58291, NatPmpProto.both, 58291)),
    );

    expect(changes).toEqual([
      {
        internalPort: 58291,
        protocol: NatPmpProto.both,
        state: 'mapped',
        port: 58291,
        previousPort: undefined,
      },
    ]);
  });

  it('treats a missing previous snapshot as nothing granted', () => {
    const changes = portForwardingChanges(
      undefined,
      status(mapped(58291, NatPmpProto.both, 58291)),
    );

    expect(changes).toHaveLength(1);
    expect(changes[0].state).toBe('mapped');
    expect(changes[0].previousPort).toBeUndefined();
  });

  // The daemon renews a mapping every half-lifetime. Firing on every renewal
  // would restart a torrent client twice an hour for nothing, so only a port
  // that MOVED is an event.
  it('stays silent on a renewal that keeps the same port', () => {
    const changes = portForwardingChanges(
      status(mapped(58291, NatPmpProto.both, 58291)),
      status(mapped(58291, NatPmpProto.both, 58291)),
    );

    expect(changes).toEqual([]);
  });

  it('fires mapped with both ports when the grant moves to another port', () => {
    const changes = portForwardingChanges(
      status(mapped(58291, NatPmpProto.both, 58291)),
      status(mapped(58291, NatPmpProto.both, 49152)),
    );

    expect(changes).toEqual([
      {
        internalPort: 58291,
        protocol: NatPmpProto.both,
        state: 'mapped',
        port: 49152,
        previousPort: 58291,
      },
    ]);
  });

  it('fires lost with the previous port when a rule leaves the mapped state', () => {
    const changes = portForwardingChanges(
      status(mapped(58291, NatPmpProto.both, 58291)),
      status({
        internalPort: 58291,
        protocol: NatPmpProto.both,
        status: { state: 'failed', errorMessage: 'taken', errorReason: 'suggested-port-in-use' },
      }),
    );

    expect(changes).toEqual([
      {
        internalPort: 58291,
        protocol: NatPmpProto.both,
        state: 'lost',
        port: undefined,
        previousPort: 58291,
      },
    ]);
  });

  it('fires lost when a granted rule disappears from the snapshot', () => {
    const changes = portForwardingChanges(status(mapped(58291, NatPmpProto.both, 58291)), status());

    expect(changes).toEqual([
      {
        internalPort: 58291,
        protocol: NatPmpProto.both,
        state: 'lost',
        port: undefined,
        previousPort: 58291,
      },
    ]);
  });

  it('stays silent on a rule that never held a grant', () => {
    const ungranted = status(
      requesting(58291, NatPmpProto.both),
      {
        internalPort: 6881,
        protocol: NatPmpProto.tcp,
        status: { state: 'rate-limited', retryAfterSecs: 30 },
      },
      {
        internalPort: 6882,
        protocol: NatPmpProto.tcp,
        status: { state: 'failed', errorMessage: 'no port', errorReason: 'out-of-resources' },
      },
      { internalPort: 6883, protocol: NatPmpProto.udp, status: { state: 'disabled' } },
    );

    expect(portForwardingChanges(status(), ungranted)).toEqual([]);
  });

  it('produces one event per changed rule, in next snapshot order', () => {
    const changes = portForwardingChanges(
      status(mapped(6881, NatPmpProto.tcp, 40001), mapped(58291, NatPmpProto.both, 58291)),
      status(mapped(6881, NatPmpProto.tcp, 40002), mapped(58291, NatPmpProto.both, 49152)),
    );

    expect(changes.map((change) => change.internalPort)).toEqual([6881, 58291]);
    expect(changes.map((change) => change.port)).toEqual([40002, 49152]);
  });

  // The exit allocator keys a rule on (internalPort, protocol), so the same
  // device port under two protocols is two independent grants.
  it('keys a rule on its internal port and its protocol', () => {
    const changes = portForwardingChanges(
      status(mapped(6881, NatPmpProto.tcp, 40001)),
      status(mapped(6881, NatPmpProto.udp, 40001)),
    );

    expect(changes).toEqual([
      {
        internalPort: 6881,
        protocol: NatPmpProto.udp,
        state: 'mapped',
        port: 40001,
        previousPort: undefined,
      },
      {
        internalPort: 6881,
        protocol: NatPmpProto.tcp,
        state: 'lost',
        port: undefined,
        previousPort: 40001,
      },
    ]);
  });
});
