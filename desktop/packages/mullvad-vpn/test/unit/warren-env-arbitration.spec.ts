import fs from 'fs';
import path from 'path';
import { describe, expect, it, vi } from 'vitest';

import { connectButtonDisabled } from '../../src/renderer/lib/env-yield';
import {
  WarrenEnvStandDownNotificationProvider,
  WarrenLowerEnvActiveNotificationProvider,
} from '../../src/renderer/lib/notifications';
import { WarrenForeignEnv } from '../../src/shared/daemon-rpc-types';
import {
  InAppNotification,
  InAppNotificationAction,
  InAppNotificationSubtitle,
} from '../../src/shared/notifications';

const NOTIFICATION_AREA_SOURCE = path.resolve(
  __dirname,
  '../../src/renderer/components/NotificationArea.tsx',
);

function foreign(overrides: Partial<WarrenForeignEnv> = {}): WarrenForeignEnv {
  return { name: 'prod', outranksUs: true, asserting: false, ...overrides };
}

function segments(notification: InAppNotification): InAppNotificationSubtitle[] {
  const { subtitle } = notification;
  if (!Array.isArray(subtitle)) {
    throw new Error('this banner is expected to carry its text as segments');
  }
  return subtitle;
}

function text(notification: InAppNotification): string {
  return segments(notification)
    .map((segment) => segment.content)
    .join(' ');
}

function actions(notification: InAppNotification): InAppNotificationAction[] {
  const inSubtitle = segments(notification)
    .map((segment) => segment.action)
    .filter((action): action is InAppNotificationAction => action !== undefined);
  return notification.action ? [notification.action, ...inSubtitle] : inSubtitle;
}

describe('Warren product environment arbitration', () => {
  it('says nothing while no other environment is asserting the machine', () => {
    const standDown = new WarrenEnvStandDownNotificationProvider({
      envYield: null,
      clearEnvYield: vi.fn(),
    });
    const lower = new WarrenLowerEnvActiveNotificationProvider({
      foreignEnvironments: [foreign(), foreign({ name: 'beta', outranksUs: false })],
    });

    expect(standDown.mayDisplay()).to.be.false;
    expect(lower.mayDisplay()).to.be.false;
  });

  describe('the build that stood down', () => {
    it('names the environment that took priority', () => {
      const provider = new WarrenEnvStandDownNotificationProvider({
        envYield: { yieldedTo: 'prod', restorable: false },
        clearEnvYield: vi.fn(),
      });

      expect(provider.mayDisplay()).to.be.true;
      const notification = provider.getInAppNotification();
      // A deliberate stand-down, so not the red of a failure, and not the
      // green of an informational banner either: this build will not connect.
      expect(notification.indicator).to.equal('warning');
      expect(text(notification)).to.contain('Warren VPN');
    });

    it('offers no re-enable while the other environment still asserts, and says why', () => {
      const notification = new WarrenEnvStandDownNotificationProvider({
        envYield: { yieldedTo: 'prod', restorable: false },
        clearEnvYield: vi.fn(),
      }).getInAppNotification();

      // An inert greyed control explains nothing, so the banner drops the
      // action entirely and spends a sentence on the reason instead.
      expect(actions(notification)).to.have.lengthOf(0);
      expect(text(notification)).to.contain('once Warren VPN is disconnected');
    });

    it('offers the re-enable once that environment has stopped asserting', () => {
      const clearEnvYield = vi.fn();
      const notification = new WarrenEnvStandDownNotificationProvider({
        envYield: { yieldedTo: 'prod', restorable: true },
        clearEnvYield,
      }).getInAppNotification();

      const offered = actions(notification);
      expect(offered).to.have.lengthOf(1);
      expect(offered[0].type).to.equal('run-function');
      if (offered[0].type === 'run-function') {
        (offered[0].button.onClick as () => void)();
      }
      expect(clearEnvYield).toHaveBeenCalledOnce();
    });

    it('outranks the connection-state banners', () => {
      // The provider array in NotificationArea is the ranking rule, and this
      // banner explains why the app will not connect at all, so every banner
      // describing the connection has to sit below it.
      const source = fs.readFileSync(NOTIFICATION_AREA_SOURCE, 'utf8');
      const rankOf = (provider: string) => {
        const rank = source.indexOf(`new ${provider}(`);
        expect(rank, `${provider} is not in the notification area`).to.be.greaterThan(-1);
        return rank;
      };

      for (const outranked of [
        'WarrenHostOfflineNotificationProvider',
        'WarrenConnectingStuckNotificationProvider',
        'ConnectingNotificationProvider',
        'ReconnectingNotificationProvider',
        'LockdownModeNotificationProvider',
        'ErrorNotificationProvider',
      ]) {
        expect(rankOf('WarrenEnvStandDownNotificationProvider'), outranked).to.be.lessThan(
          rankOf(outranked),
        );
      }
    });

    it('refuses the connect button, rather than pressing it into a daemon error', () => {
      expect(connectButtonDisabled('disconnected', null)).to.be.false;
      expect(connectButtonDisabled('disconnected', { yieldedTo: 'prod', restorable: true })).to.be
        .true;
      expect(connectButtonDisabled('disconnecting', null)).to.be.true;
    });
  });

  describe('the build that holds priority', () => {
    it('says a lower environment holds a tunnel and will stand down', () => {
      const provider = new WarrenLowerEnvActiveNotificationProvider({
        foreignEnvironments: [foreign({ name: 'beta', outranksUs: false, asserting: true })],
      });

      expect(provider.mayDisplay()).to.be.true;
      const notification = provider.getInAppNotification();
      expect(notification.indicator).to.equal('success');
      expect(text(notification)).to.contain('Warren VPN Beta');
      // Informational only: this build never gains a control over another
      // environment, which is the whole safety of the arbitration.
      expect(actions(notification)).to.have.lengthOf(0);
    });

    it('stays silent about an environment that outranks this build', () => {
      const provider = new WarrenLowerEnvActiveNotificationProvider({
        foreignEnvironments: [foreign({ name: 'prod', outranksUs: true, asserting: true })],
      });

      expect(provider.mayDisplay()).to.be.false;
    });
  });
});
