import { describe, expect, it, vi } from 'vitest';

import NotificationController from '../../src/main/notification-controller';
import { NatPmpProto } from '../../src/shared/daemon-rpc-types';
import { SystemNotificationCategory } from '../../src/shared/notifications';
import { PortForwardingNotificationProvider } from '../../src/shared/notifications/port-forwarding';
import { PortChange } from '../../src/shared/port-forwarding-changes';
import { RoutePath } from '../../src/shared/routes';

const granted: PortChange = {
  internalPort: 58291,
  protocol: NatPmpProto.both,
  state: 'mapped',
  port: 58291,
  previousPort: undefined,
};

const lost: PortChange = {
  internalPort: 58291,
  protocol: NatPmpProto.both,
  state: 'lost',
  port: undefined,
  previousPort: 58291,
};

describe('PortForwardingNotificationProvider', () => {
  it('names the port that is now open', () => {
    const notification = new PortForwardingNotificationProvider(granted).getSystemNotification();

    expect(notification.message).to.equal('Port forwarding: public port 58291 is open');
  });

  // A lost grant carries no current port, so the message has to name the one
  // the user was told about, which is the one their torrent client still holds.
  it('names the port that is no longer open', () => {
    const notification = new PortForwardingNotificationProvider(lost).getSystemNotification();

    expect(notification.message).to.equal('Port forwarding: public port 58291 is no longer open');
  });

  it('files both under the port-forwarding category, so a new one replaces the last', () => {
    expect(
      new PortForwardingNotificationProvider(granted).getSystemNotification().category,
    ).to.equal(SystemNotificationCategory.portForwarding);
    expect(new PortForwardingNotificationProvider(lost).getSystemNotification().category).to.equal(
      SystemNotificationCategory.portForwarding,
    );
  });

  it('offers the port-forwarding settings as its action', () => {
    const notification = new PortForwardingNotificationProvider(granted).getSystemNotification();

    expect(notification.action?.type).to.equal('navigate-internal');
    expect(notification.action?.link.to).to.equal(RoutePath.portForwardingSettings);
  });

  it('displays every change it is given', () => {
    expect(new PortForwardingNotificationProvider(granted).mayDisplay()).to.be.true;
    expect(new PortForwardingNotificationProvider(lost).mayDisplay()).to.be.true;
  });
});

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

  return new TestNotificationController({
    openApp: vi.fn(),
    openLink: vi.fn().mockReturnValue(Promise.resolve()),
    openRoute: vi.fn(),
    showNotificationIcon: vi.fn(),
  });
}

describe('NotificationController.notifyPortForwardingChange', () => {
  it('raises the toast when the window is hidden', () => {
    expect(createController().notifyPortForwardingChange(granted, false, true)).to.be.true;
  });

  // The port is on screen in the app itself, so a toast over it would repeat
  // what the user is already reading.
  it('stays quiet while the window is visible', () => {
    expect(createController().notifyPortForwardingChange(granted, true, true)).to.be.false;
  });

  it('stays quiet when system notifications are turned off', () => {
    expect(createController().notifyPortForwardingChange(granted, false, false)).to.be.false;
  });
});
