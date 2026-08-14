import { sprintf } from 'sprintf-js';

import { messages } from '../../../shared/gettext';

// The daemon samples the live tunnel every few seconds and publishes both
// counts: how many transport legs carry the traffic, and how many of those kept
// sending while nothing came back. The label states that observation, because a
// stalled leg costs downlink capacity without taking the tunnel down, and the
// connection panel would otherwise show nothing but "Connected".
//
// Both numbers are needed for the detailed form: without the bundle width
// (older daemon, or a sample that has not landed yet) "1 of ?" would say less
// than the plain warning.
export function degradedBondIndicatorLabel(stalled?: number, bonded?: number): string {
  if (stalled && bonded) {
    return sprintf(
      // TRANSLATORS: Feature indicator (warning) shown when part of the tunnel's parallel
      // TRANSLATORS: connections stopped receiving, so downlink capacity is reduced.
      // TRANSLATORS: Available placeholders:
      // TRANSLATORS: %(stalled)s - how many connections stopped receiving
      // TRANSLATORS: %(bonded)s - how many connections the tunnel uses in total
      messages.pgettext('connect-view', 'Degraded bond (%(stalled)s of %(bonded)s)'),
      { stalled, bonded },
    );
  }
  // TRANSLATORS: Feature indicator (warning) shown when part of the tunnel's parallel
  // TRANSLATORS: connections stopped receiving, so downlink capacity is reduced.
  return messages.pgettext('connect-view', 'Degraded bond');
}
