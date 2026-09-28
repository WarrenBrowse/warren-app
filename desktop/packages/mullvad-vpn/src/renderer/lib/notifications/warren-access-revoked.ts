import { banInForce, banLine } from '../../../shared/account-standing';
import { urls } from '../../../shared/constants';
import { TunnelState, WarrenAccountStanding } from '../../../shared/daemon-rpc-types';
import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenAccessRevokedNotificationContext {
  tunnelState: TunnelState;
  // The wallet's standing as the daemon last knew it, `null` while unknown.
  accountStanding: WarrenAccountStanding | null;
  locale: string;
  // Injected so the lapse can be pinned in a test.
  nowMs?: () => number;
}

// While a ban holds, the daemon refuses every connect (the issuers refuse the
// wallet its credentials anyway), so the disconnected screen would otherwise
// show a connect button that does nothing. A tunnel the ban caught while up
// sits in the error state instead, where the error banner already says it.
export class WarrenAccessRevokedNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenAccessRevokedNotificationContext) {}

  public mayDisplay = () =>
    this.context.tunnelState.state === 'disconnected' && this.ban() !== undefined;

  public getInAppNotification(): InAppNotification {
    const ban = this.ban()!;
    return {
      indicator: 'error',
      title: messages.pgettext('in-app-notifications', 'ACCESS REVOKED'),
      subtitle: `${banLine(ban, this.context.locale)} ${messages.pgettext(
        'in-app-notifications',
        'Connecting is not possible while access is revoked.',
      )}`,
      action:
        ban.reason === 'port-forwarding-abuse'
          ? {
              type: 'navigate-external',
              link: {
                to: urls.reports,
                'aria-label': messages.pgettext('accessibility', 'How to contest a warning'),
              },
            }
          : undefined,
    };
  }

  private ban() {
    const now = this.context.nowMs?.() ?? Date.now();
    return banInForce(this.context.accountStanding, now);
  }
}
