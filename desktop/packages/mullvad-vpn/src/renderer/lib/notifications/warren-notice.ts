import { WarrenNotice } from '../../../shared/daemon-rpc-types';
import { messages } from '../../../shared/gettext';
import { InAppNotification, InAppNotificationProvider } from '../../../shared/notifications';

interface WarrenNoticeNotificationContext {
  // Notices the daemon fetched from the API, verified against the pinned
  // server key, and filtered for expiry and this app's version. Already
  // display-ready: the renderer shows them verbatim.
  notices: WarrenNotice[];
  // Keys of the notices this user has put away, from the GUI settings.
  dismissedKeys: string[];
  dismiss: (key: string) => void;
}

/**
 * Key a dismissal is recorded under. The wording is part of it, not just the
 * id: an operator who rewrites a notice in place keeps its id, and a key on
 * the id alone would bury the new words for everyone who had put the old ones
 * away. Any stable digest does, this one is the classic djb2.
 */
export function noticeDismissalKey(notice: WarrenNotice): string {
  let hash = 5381;
  for (let i = 0; i < notice.message.length; i++) {
    hash = ((hash << 5) + hash + notice.message.charCodeAt(i)) | 0;
  }
  return `${notice.id}:${hash}`;
}

// Operator broadcast banner. Ranked FIRST among the in-app notification
// providers, so an active notice outranks every connection-state banner:
// when the operator has something to say to everyone, that is the one
// thing the user must see, and the states it hides (connecting, offline,
// error) are already visible in the connect view's own status.
//
// It clears from the signal that raised it: the daemon pushes an empty list
// when the notice is erased or lapses, so there is no renderer-side timer.
//
// An informational notice can also be put away by the reader, and only that
// one: ranked on top of a slot that holds one card, a message the operator
// leaves up for a week would otherwise hide the update prompt and the expiry
// warning for that whole week. A warning or an error keeps the slot, because
// it describes something live that the user cannot act on by hiding it.
export class WarrenNoticeNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenNoticeNotificationContext) {}

  public mayDisplay = () => this.displayable() !== undefined;

  public getInAppNotification(): InAppNotification {
    const notice = this.displayable()!;
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
      action: {
        type: 'expand-text',
        expand: { title, content: notice.message },
        dismiss:
          notice.level === 'info'
            ? () => this.context.dismiss(noticeDismissalKey(notice))
            : undefined,
      },
    };
  }

  private displayable(): WarrenNotice | undefined {
    return this.context.notices.find(
      (notice) =>
        notice.level !== 'info' || !this.context.dismissedKeys.includes(noticeDismissalKey(notice)),
    );
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
