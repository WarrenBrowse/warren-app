import { sprintf } from 'sprintf-js';

import { UNREAD_SATURATED } from '../forum-identity';
import { messages } from '../gettext';
import { RoutePath } from '../routes';
import {
  SystemNotification,
  SystemNotificationCategory,
  SystemNotificationProvider,
  SystemNotificationSeverityType,
} from './notification';

interface ForumActivityNotificationContext {
  unread: number;
}

/**
 * Desktop banner for new community-forum activity.
 *
 * Severity `low` on purpose: that is the band the global notification
 * setting is allowed to suppress, so turning system notifications off
 * silences this too, and the forum-specific setting is an extra gate on
 * top rather than a way around it.
 *
 * The count is all the broadcast digest carries. Nothing here names a
 * topic or an author, which is what keeps the badge free of any per-user
 * request; the content is only ever read when the user opens the panel.
 */
export class ForumActivityNotificationProvider implements SystemNotificationProvider {
  public constructor(private context: ForumActivityNotificationContext) {}

  public mayDisplay() {
    return this.context.unread > 0;
  }

  public getSystemNotification(): SystemNotification {
    return {
      message: this.systemMessage(),
      category: SystemNotificationCategory.forumActivity,
      severity: SystemNotificationSeverityType.low,
      action: {
        type: 'navigate-internal',
        link: {
          // TRANSLATORS: Button on the notification about new forum activity,
          // TRANSLATORS: which opens the activity panel in the app.
          text: messages.pgettext('notifications', 'Open'),
          to: RoutePath.forumActivity,
        },
      },
    };
  }

  private systemMessage(): string {
    const { unread } = this.context;

    // One digest character per slot, so the count stops climbing at its
    // ceiling. Saying "15" there would be a number the user can check and
    // find wrong.
    if (unread >= UNREAD_SATURATED) {
      return sprintf(
        // TRANSLATORS: Notification shown when a lot of forum activity is
        // TRANSLATORS: waiting, above the count the app can measure exactly.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(count)d - the highest count the app can measure
        messages.pgettext('notifications', 'More than %(count)d new notifications on the forum'),
        { count: unread - 1 },
      );
    }

    return sprintf(
      messages.npgettext(
        'notifications',
        // TRANSLATORS: Notification shown when one reply, like or mention is
        // TRANSLATORS: waiting on the community forum.
        'New notification on the forum',
        // TRANSLATORS: Notification shown when several replies, likes or
        // TRANSLATORS: mentions are waiting on the community forum.
        // TRANSLATORS: Available placeholders:
        // TRANSLATORS: %(count)d - how many are waiting
        '%(count)d new notifications on the forum',
        unread,
      ),
      { count: unread },
    );
  }
}
