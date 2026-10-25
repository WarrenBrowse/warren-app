import { NatPmpStatus } from '../shared/daemon-rpc-types';
import { PortChange, portForwardingChanges } from '../shared/port-forwarding-changes';

/** The granted mappings of a snapshot, the only part worth remembering: a
 * rule still asking holds no port anyone could have been told about. */
function grantedOnly(snapshot: NatPmpStatus): NatPmpStatus {
  return { mappings: snapshot.mappings.filter((mapping) => mapping.status.state === 'mapped') };
}

/**
 * Turns the daemon's NAT-PMP snapshot stream into port-change events.
 *
 * The first snapshot of a subscription is diffed against the ports the app
 * last saw, which the main process keeps across restarts (`remembered()`), and
 * only its grants count. The daemon keeps its mappings across a GUI restart
 * and replays them as soon as the new subscription opens, so the same port is
 * not news; but at boot the daemon is a service that gets the day's port
 * before the GUI starts, and that port is news when it moved. A remembered
 * port missing from that first snapshot is not announced as lost: the GUI may
 * simply have started before the daemon's answer.
 *
 * Holds no clock and no I/O, so the whole rule is testable without electron.
 */
export class PortForwardingWatcher {
  private previous?: NatPmpStatus;
  private baseline: NatPmpStatus;
  private last: NatPmpStatus;

  public constructor(remembered?: NatPmpStatus) {
    this.last = remembered === undefined ? { mappings: [] } : grantedOnly(remembered);
    this.baseline = this.last;
  }

  public observe(snapshot: NatPmpStatus): PortChange[] {
    const previous = this.previous;
    this.previous = snapshot;
    this.last = grantedOnly(snapshot);
    if (previous === undefined) {
      return portForwardingChanges(this.baseline, snapshot).filter(
        (change) => change.state === 'mapped',
      );
    }
    return portForwardingChanges(previous, snapshot);
  }

  /** The grants of the last snapshot seen, for the main process to keep. */
  public remembered(): NatPmpStatus {
    return this.last;
  }

  /** Starts a new subscription: its first snapshot is diffed against the last
   * ports seen, the way a GUI restart's is. */
  public reset() {
    this.baseline = this.last;
    this.previous = undefined;
  }
}
