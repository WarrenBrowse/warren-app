import { sprintf } from 'sprintf-js';

import { messages } from '../../../shared/gettext';

// The daemon sets ReducedMtu from the NEGOTIATED endpoint once the live path
// measured below the default packet size, and carries the measured usable
// size when it has one. The label surfaces that number so the user can tell a
// mildly reduced path from a severely constrained one; without it (older
// daemon, value not yet sampled) the plain warning still shows.
export function reducedMtuIndicatorLabel(effectiveMtu?: number): string {
  if (effectiveMtu) {
    return sprintf(
      // TRANSLATORS: Feature indicator (warning) shown when the network path cannot carry
      // TRANSLATORS: full-size packets; the tunnel adapts automatically.
      // TRANSLATORS: Available placeholders:
      // TRANSLATORS: %(mtu)s - the measured usable packet size in bytes
      messages.pgettext('connect-view', 'Reduced MTU (%(mtu)s)'),
      { mtu: effectiveMtu },
    );
  }
  // TRANSLATORS: Feature indicator (warning) shown when the network path cannot carry
  // TRANSLATORS: full-size packets; the tunnel adapts automatically.
  return messages.pgettext('connect-view', 'Reduced MTU');
}
