import { describe, expect, it, vi } from 'vitest';

import NotificationController from '../../src/main/notification-controller';
import {
  SystemNotification,
  SystemNotificationCategory,
  SystemNotificationSeverityType,
} from '../../src/shared/notifications';

function createController() {
  class TestNotificationController extends NotificationController {
    // @ts-expect-error Way too many methods to mock.
    private createElectronNotification() {
      return {
        show: () => {
          /* no-op */
        },
        close: () => {
          /* no-op */
        },
        on: () => {
          /* no-op */
        },
        removeAllListeners: () => {
          /* no-op */
        },
      };
    }
  }

  const showNotificationIcon = vi.fn();
  const controller = new TestNotificationController({
    openApp: vi.fn(),
    openLink: vi.fn().mockReturnValue(Promise.resolve()),
    openRoute: vi.fn(),
    showNotificationIcon,
  });
  return { controller, showNotificationIcon };
}

const UPDATE: SystemNotification = {
  message: 'Update available',
  category: SystemNotificationCategory.newVersion,
  severity: SystemNotificationSeverityType.medium,
};

/** Whether the dot is lit after the calls made so far. */
function lit(showNotificationIcon: ReturnType<typeof vi.fn>): boolean {
  const calls = showNotificationIcon.mock.calls;
  return calls.length > 0 && calls[calls.length - 1][0] === true;
}

describe('the tray dot and unread forum activity', () => {
  it('lights on unread forum activity alone', () => {
    const { controller, showNotificationIcon } = createController();

    controller.setForumActivityIndicator(true);

    expect(lit(showNotificationIcon)).toBe(true);
  });

  it('goes out when the forum has been read', () => {
    const { controller, showNotificationIcon } = createController();

    controller.setForumActivityIndicator(true);
    controller.setForumActivityIndicator(false);

    expect(lit(showNotificationIcon)).toBe(false);
  });

  it('stays lit for a notification once the forum has been read', () => {
    // The two inputs are independent: reading the forum must not clear a dot
    // that an app update put there.
    const { controller, showNotificationIcon } = createController();

    controller.setForumActivityIndicator(true);
    controller.notify(UPDATE, false, true);
    controller.setForumActivityIndicator(false);

    expect(lit(showNotificationIcon)).toBe(true);
  });

  it('stays lit for the forum once a notification is gone', () => {
    // The other direction, and the reason the dot cannot just be a long-lived
    // notification: unread activity outlives any banner about it.
    const { controller, showNotificationIcon } = createController();

    controller.notify(UPDATE, false, true);
    controller.setForumActivityIndicator(true);
    controller.closeNotificationsInCategory(SystemNotificationCategory.newVersion);

    expect(lit(showNotificationIcon)).toBe(true);
  });

  it('reports the forum as a reason so a stuck dot can be traced', () => {
    const { controller, showNotificationIcon } = createController();

    controller.setForumActivityIndicator(true);

    const [, reason] = showNotificationIcon.mock.calls[showNotificationIcon.mock.calls.length - 1];
    expect(reason).toContain('forum');
  });

  it('reads the same state the tray icon is built from', () => {
    // The tray is created on the first tunnel state, which can land after the
    // first digest. Building it from `false` would leave a dot the digest has
    // already raised invisible until the next change.
    const { controller } = createController();

    controller.setForumActivityIndicator(true);

    expect(controller.notificationIconState.show).toBe(true);
  });
});
