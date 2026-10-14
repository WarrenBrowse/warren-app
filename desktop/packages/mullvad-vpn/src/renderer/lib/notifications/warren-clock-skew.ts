import { sprintf } from 'sprintf-js';

import { WarrenTokenClockSkew } from '../../../shared/daemon-rpc-types';
import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenClockSkewNotificationContext {
  // Set by the daemon while the token issuer refuses this wallet's refresh
  // because the device clock is outside its window. With no session token
  // the tunnel cannot come up, and the connect screen alone would only show
  // an endless "connecting".
  tokenClockSkew: WarrenTokenClockSkew | null;
}

/** Whole minutes the clock is off, never zero: the servers refuse a clock
 * only once it is more than a minute off. */
function minutesOff(offsetSecs: number): number {
  return Math.max(1, Math.ceil(Math.abs(offsetSecs) / 60));
}

// Shown in every tunnel state while the refusal holds; drops by itself once a
// refresh succeeds (the daemon pushes a fresh status). The fix is in the
// system settings, so there is no action button.
export class WarrenClockSkewNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenClockSkewNotificationContext) {}

  public mayDisplay = () => this.context.tokenClockSkew !== null;

  public getInAppNotification(): InAppNotification {
    const offsetSecs = this.context.tokenClockSkew?.offsetSecs;
    const subtitle =
      offsetSecs === undefined
        ? messages.pgettext(
            'in-app-notifications',
            'This device’s clock is off: set it right to connect.',
          )
        : sprintf(
            // TRANSLATORS: Banner shown while the servers refuse to hand out
            // TRANSLATORS: connection tokens because the device clock is wrong.
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(minutes)d - how many minutes the clock is off
            messages.pgettext(
              'in-app-notifications',
              'This device’s clock is off by %(minutes)d min: set it right to connect.',
            ),
            { minutes: minutesOff(offsetSecs) },
          );
    return {
      indicator: 'error',
      title: messages.pgettext('in-app-notifications', 'DEVICE CLOCK IS OFF'),
      subtitle,
    };
  }
}
