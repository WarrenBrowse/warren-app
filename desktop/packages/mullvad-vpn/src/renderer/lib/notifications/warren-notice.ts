import { WarrenNotice } from '../../../shared/daemon-rpc-types';
import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenNoticeNotificationContext {
  // Notices the daemon fetched from the API, verified against the pinned
  // server key, and filtered for expiry and this app's version. Already
  // display-ready: the renderer shows them verbatim.
  notices: WarrenNotice[];
}

// Operator broadcast banner. Ranked FIRST among the in-app notification
// providers, so an active notice outranks every connection-state banner:
// when the operator has something to say to everyone, that is the one
// thing the user must see, and the states it hides (connecting, offline,
// error) are already visible in the connect view's own status.
//
// It clears from the same signal that raised it: the daemon pushes an
// empty list when the notice is erased or lapses, so there is no dismiss
// button and no renderer-side timer.
export class WarrenNoticeNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenNoticeNotificationContext) {}

  public mayDisplay = () => this.context.notices.length > 0;

  public getInAppNotification(): InAppNotification {
    const notice = this.context.notices[0];
    const title = titleFor(notice.level);
    return {
      indicator: indicatorFor(notice.level),
      // The label is translated; the message body is the operator's own
      // words, shown as authored.
      title,
      subtitle: notice.message,
      // A notice can be as long as the publication cap allows, so the banner
      // clamps it and offers the full text in a scrollable modal. The banner
      // renders the affordance only when the text actually overflows.
      action: { type: 'expand-text', expand: { title, content: notice.message } },
    };
  }
}

function indicatorFor(level: WarrenNotice['level']): InAppNotification['indicator'] {
  switch (level) {
    case 'error':
      return 'error';
    case 'warning':
      return 'warning';
    default:
      return 'success';
  }
}

function titleFor(level: WarrenNotice['level']): string {
  switch (level) {
    case 'error':
      return messages.pgettext('in-app-notifications', 'IMPORTANT');
    case 'warning':
      return messages.pgettext('in-app-notifications', 'NOTICE');
    default:
      return messages.pgettext('in-app-notifications', 'WARREN');
  }
}
