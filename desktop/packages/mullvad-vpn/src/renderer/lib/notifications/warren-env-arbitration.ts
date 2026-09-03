import { sprintf } from 'sprintf-js';

import { displayNameForEnvironment } from '../../../shared/constants/product-env';
import { WarrenEnvYield, WarrenForeignEnv } from '../../../shared/daemon-rpc-types';
import { messages } from '../../../shared/gettext';
import {
  InAppNotification,
  InAppNotificationProvider,
  InAppNotificationSubtitle,
} from '../../../shared/notifications';

interface WarrenEnvStandDownNotificationContext {
  // The stand-down the daemon holds, or `null` in the ordinary state. Set
  // while a higher-priority product environment (prod over staging over beta)
  // asserts this machine, and carrying whether the daemon would accept the
  // manual re-enable right now.
  envYield: WarrenEnvYield | null;
  // Calls the daemon's `ClearEnvYield`, restoring the auto-connect and kill
  // switch recorded at the stand-down. Offered only while the yield is
  // restorable: the daemon refuses it otherwise.
  clearEnvYield: () => void;
}

// Stand-down banner of the outranked build. Ranked above every
// connection-state banner because it explains why this app will not connect
// at all: a "connecting" or "blocked" banner shown on top of it would
// describe a state this build is not even trying to reach.
//
// It clears from the signal that raised it, like the operator notice: the
// daemon drops the yield when the user re-enables this build, so there is no
// dismiss button.
export class WarrenEnvStandDownNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenEnvStandDownNotificationContext) {}

  public mayDisplay = () => this.context.envYield !== null;

  public getInAppNotification(): InAppNotification {
    const envYield = this.context.envYield;
    const product = displayNameForEnvironment(envYield?.yieldedTo ?? '');

    const explanation: InAppNotificationSubtitle = {
      content: sprintf(
        // TRANSLATORS: Banner shown when another Warren install on the same
        // TRANSLATORS: device has taken priority, so this one disconnected
        // TRANSLATORS: itself and will not connect.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(product)s - Name of the install that took priority.
        messages.pgettext(
          'in-app-notifications',
          '%(product)s has taken priority on this device, so this build disconnected and turned its kill switch off.',
        ),
        { product },
      ),
    };

    return {
      // A deliberate stand-down rather than a failure, so not the error red;
      // it does stop this build from connecting, so not the informational
      // green either.
      indicator: 'warning',
      title: messages.pgettext('in-app-notifications', 'STANDING BY'),
      subtitle: [explanation, this.reEnable(product)],
    };
  }

  // The way back, or the one sentence saying why it is not on offer yet. A
  // greyed control would carry the same non-availability with none of the
  // reason, and the daemon refuses `ClearEnvYield` in that window anyway.
  private reEnable(product: string): InAppNotificationSubtitle {
    if (!this.context.envYield?.restorable) {
      return {
        content: sprintf(
          // TRANSLATORS: Follows the stand-down banner, telling the user when
          // TRANSLATORS: this install can be brought back.
          // TRANSLATORS: Available placeholders:
          // TRANSLATORS: %(product)s - Name of the install that took priority.
          messages.pgettext(
            'in-app-notifications',
            'You can bring it back once %(product)s is disconnected.',
          ),
          { product },
        ),
      };
    }

    return {
      content:
        // TRANSLATORS: Clickable text that ends this install's stand-down and
        // TRANSLATORS: lets it connect again.
        messages.pgettext('in-app-notifications', 'Bring this build back.'),
      action: {
        type: 'run-function',
        button: {
          onClick: () => this.context.clearEnvYield(),
          'aria-label':
            // TRANSLATORS: Accessibility label for the control that ends this
            // TRANSLATORS: install's stand-down.
            messages.pgettext('in-app-notifications', 'Bring this build back'),
        },
      },
    };
  }
}

interface WarrenLowerEnvActiveNotificationContext {
  // Every other product environment the daemon watches, with whether it
  // outranks this build and whether it is asserting the machine.
  foreignEnvironments: WarrenForeignEnv[];
}

// Courtesy banner of the build that holds priority. Read-only by
// construction: this build observes the lower environment and says what will
// happen to it, and it never gains a control that commands another
// environment. The lower install is the one that stands down, by itself.
export class WarrenLowerEnvActiveNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenLowerEnvActiveNotificationContext) {}

  public mayDisplay = () => this.lowerEnvironmentAsserting() !== undefined;

  public getInAppNotification(): InAppNotification {
    const product = displayNameForEnvironment(this.lowerEnvironmentAsserting()?.name ?? '');

    return {
      indicator: 'success',
      title: messages.pgettext('in-app-notifications', 'ANOTHER BUILD IS CONNECTED'),
      subtitle: [
        {
          content: sprintf(
            // TRANSLATORS: Banner shown when a lower-priority Warren install
            // TRANSLATORS: on the same device holds the tunnel.
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(product)s - Name of the install holding the tunnel.
            messages.pgettext(
              'in-app-notifications',
              '%(product)s holds the tunnel on this device. It stands down on its own as soon as you connect here.',
            ),
            { product },
          ),
        },
      ],
    };
  }

  private lowerEnvironmentAsserting(): WarrenForeignEnv | undefined {
    return this.context.foreignEnvironments.find(
      (environment) => !environment.outranksUs && environment.asserting,
    );
  }
}
