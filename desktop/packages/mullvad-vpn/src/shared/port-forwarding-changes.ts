import { NatPmpMapping, NatPmpProto, NatPmpStatus } from './daemon-rpc-types';

/**
 * One rule whose granted public port changed between two NAT-PMP snapshots.
 *
 * `state` says which side of the grant moved: `mapped` carries the port the
 * rule now holds (and the one it held before, when it moved), `lost` carries
 * only the port it used to hold.
 */
export interface PortChange {
  internalPort: number;
  protocol: NatPmpProto;
  state: 'mapped' | 'lost';
  port: number | undefined;
  previousPort: number | undefined;
}

/** The public port a mapping currently holds, or `undefined` when it holds
 * none. Only `mapped` counts: a port carried on any other state is stale, and
 * announcing it would point an application at a port the exit no longer
 * forwards. */
function grantedPort(mapping: NatPmpMapping): number | undefined {
  return mapping.status.state === 'mapped' ? mapping.status.externalPort : undefined;
}

function sameRule(a: NatPmpMapping, b: NatPmpMapping): boolean {
  return a.internalPort === b.internalPort && a.protocol === b.protocol;
}

/**
 * The rules whose grant changed between two snapshots, in `next` snapshot
 * order, followed by the rules that vanished from it.
 *
 * A renewal that keeps the same port produces nothing: the daemon renews every
 * half-lifetime, and an event on each renewal would restart a torrent client
 * twice an hour for nothing. The semantics are the ones the `warren
 * port-forward --watch --exec` hook froze (mullvad-cli/src/cmds/port_forward.rs,
 * `port_changes`), so the desktop notification and the CLI hook can never
 * disagree about what counts as a port change.
 *
 * Pure: no clock, no I/O, no electron. A missing `previous` means nothing was
 * granted yet, so every live grant in `next` is a change; the caller that
 * wants a silent first snapshot holds that baseline itself.
 */
export function portForwardingChanges(
  previous: NatPmpStatus | undefined,
  next: NatPmpStatus,
): PortChange[] {
  const before = previous?.mappings ?? [];

  const changed: PortChange[] = next.mappings.flatMap((mapping) => {
    const port = grantedPort(mapping);
    const counterpart = before.find((candidate) => sameRule(candidate, mapping));
    const was = counterpart === undefined ? undefined : grantedPort(counterpart);
    if (port === was) {
      return [];
    }
    return [
      {
        internalPort: mapping.internalPort,
        protocol: mapping.protocol,
        state: port === undefined ? ('lost' as const) : ('mapped' as const),
        port,
        previousPort: was,
      },
    ];
  });

  const vanished: PortChange[] = before.flatMap((mapping) => {
    const was = grantedPort(mapping);
    if (was === undefined || next.mappings.some((candidate) => sameRule(candidate, mapping))) {
      return [];
    }
    return [
      {
        internalPort: mapping.internalPort,
        protocol: mapping.protocol,
        state: 'lost' as const,
        port: undefined,
        previousPort: was,
      },
    ];
  });

  return [...changed, ...vanished];
}
