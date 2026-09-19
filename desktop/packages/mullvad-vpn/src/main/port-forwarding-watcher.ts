import { NatPmpStatus } from '../shared/daemon-rpc-types';
import { PortChange, portForwardingChanges } from '../shared/port-forwarding-changes';

/**
 * Turns the daemon's NAT-PMP snapshot stream into port-change events.
 *
 * The first snapshot of a subscription is a baseline and produces nothing: the
 * daemon keeps its mappings across a GUI restart and replays them as soon as
 * the new subscription opens, so diffing it against nothing would announce a
 * port that has not moved every time the app is reopened. `reset()` puts the
 * watcher back in that state, which is what a daemon disconnect does to the
 * stream.
 *
 * Holds no clock and no I/O, so the whole baseline-then-diff rule is testable
 * without electron.
 */
export class PortForwardingWatcher {
  private previous?: NatPmpStatus;

  public observe(snapshot: NatPmpStatus): PortChange[] {
    const previous = this.previous;
    this.previous = snapshot;
    return previous === undefined ? [] : portForwardingChanges(previous, snapshot);
  }

  public reset() {
    this.previous = undefined;
  }
}
