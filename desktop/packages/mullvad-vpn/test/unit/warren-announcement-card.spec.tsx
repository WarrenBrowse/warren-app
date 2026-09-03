import fs from 'fs';
import os from 'os';
import path from 'path';
import { renderToStaticMarkup } from 'react-dom/server';
import { ServerStyleSheet } from 'styled-components';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

// `vi.hoisted` runs before any import is evaluated. The renderer component
// library reads `window.env.platform` at top level when it builds the global
// style, so a stub `window` has to exist before the imports below execute.
vi.hoisted(() => {
  (globalThis as { window?: unknown }).window = {
    env: { platform: 'linux', development: false },
  };
});

// GuiSettings resolves its file through `app.getPath`, and the unit suite
// aliases `electron` to an empty module. A temporary user-data directory is
// enough to exercise the store/load round trip a dismissal has to survive.
const USER_DATA = fs.mkdtempSync(path.join(os.tmpdir(), 'warren-gui-settings-'));
vi.mock('electron', () => ({ app: { getPath: () => USER_DATA } }));

import GuiSettings from '../../src/main/gui-settings';
import {
  NotificationSubtitleText,
  NotificationTitle,
} from '../../src/renderer/components/NotificationBanner';
import { WarrenAnnouncementCard } from '../../src/renderer/components/WarrenAnnouncementCard';
import { copyToClipboard } from '../../src/renderer/lib/clipboard';
import {
  announcementCtaUrl,
  WarrenAnnouncementNotificationProvider,
  WarrenNoticeNotificationProvider,
} from '../../src/renderer/lib/notifications';
import { WarrenAnnouncement } from '../../src/shared/daemon-rpc-types';
import { InAppAnnouncementCard, InAppNotification } from '../../src/shared/notifications';

const NOTIFICATION_AREA_SOURCE = path.resolve(
  __dirname,
  '../../src/renderer/components/NotificationArea.tsx',
);

function announcement(overrides: Partial<WarrenAnnouncement> = {}): WarrenAnnouncement {
  return {
    id: 'prod-launch-2026',
    headline: 'Warren production is open',
    body: 'The production network is live. Your beta account keeps working, and this code is one free month on it.',
    level: 'info',
    cta: null,
    voucherCode: null,
    ...overrides,
  };
}

function card(notification: InAppNotification): InAppAnnouncementCard {
  const { action } = notification;
  if (action?.type !== 'announcement-card') {
    throw new Error('an announcement is expected to render as a card');
  }
  return action.announcement;
}

function provider(
  announcements: WarrenAnnouncement[],
  dismissedIds: string[] = [],
  dismiss: (id: string) => void = () => undefined,
) {
  return new WarrenAnnouncementNotificationProvider({ announcements, dismissedIds, dismiss });
}

interface Rendered {
  html: string;
  css: string;
}

function render(
  overrides: Partial<WarrenAnnouncement> = {},
  onOpenCta: (url: string) => void = () => undefined,
): Rendered {
  const notification = provider([announcement(overrides)]).getInAppNotification();
  const sheet = new ServerStyleSheet();
  const html = renderToStaticMarkup(
    sheet.collectStyles(
      <WarrenAnnouncementCard
        title={notification.title}
        indicator={notification.indicator}
        announcement={card(notification)}
        onOpenCta={onOpenCta}
      />,
    ),
  );
  return { html, css: sheet.getStyleTags() };
}

// The class the code block carries, so an assertion on its rules picks the
// right block out of the whole collected sheet.
function rulesFor(rendered: Rendered, marker: string): string {
  const element = new RegExp(`<[a-z]+[^>]*data-testid="${marker}"[^>]*class="([^"]*)"`).exec(
    rendered.html,
  );
  const alternate = new RegExp(`<[a-z]+[^>]*class="([^"]*)"[^>]*data-testid="${marker}"`).exec(
    rendered.html,
  );
  const classes = (element ?? alternate)?.[1];
  expect(classes, `no element carries data-testid="${marker}"`).to.be.a('string');
  const generated = classes!.split(' ').filter((name) => name.length > 0);
  return generated
    .map((name) => {
      const rule = new RegExp(`\\.${name}\\{([^}]*)\\}`).exec(rendered.css);
      return rule?.[1] ?? '';
    })
    .join('');
}

describe('Warren launch announcement card', () => {
  describe('the model the card renders', () => {
    it('stays hidden when the operator has published nothing', () => {
      expect(provider([]).mayDisplay()).to.be.false;
    });

    it('makes the operator headline the title, with no level word on top of it', () => {
      // The notice banner derives its label from the level, so every notice is
      // headed by a generic word. An announcement brings its own headline and
      // must never be demoted under one.
      for (const level of ['info', 'warning', 'error'] as const) {
        const notification = provider([
          announcement({ level, headline: 'Warren production is open' }),
        ]).getInAppNotification();
        expect(notification.title).to.equal('Warren production is open');
      }

      const noticeTitles = (['info', 'warning', 'error'] as const).map(
        (level) =>
          new WarrenNoticeNotificationProvider({
            notices: [{ id: 'n1', message: 'anything', level }],
          }).getInAppNotification().title,
      );
      const announcementTitle = provider([announcement()]).getInAppNotification().title;
      expect(noticeTitles).to.not.contain(announcementTitle);
    });

    it('carries the code exactly as the operator grouped it', () => {
      const grouped = provider([
        announcement({ voucherCode: 'ABCD-EFGH-JKMN-PQRS' }),
      ]).getInAppNotification();
      expect(card(grouped).voucherCode).to.equal('ABCD-EFGH-JKMN-PQRS');

      const ungrouped = provider([
        announcement({ voucherCode: 'ABCDEFGHJKMNPQRS' }),
      ]).getInAppNotification();
      expect(card(ungrouped).voucherCode).to.equal('ABCDEFGHJKMNPQRS');
    });

    it('carries no code when the account holds none', () => {
      expect(card(provider([announcement()]).getInAppNotification()).voucherCode).to.be.null;
    });

    it('carries the call to action only when the announcement has one', () => {
      expect(card(provider([announcement()]).getInAppNotification()).cta).to.be.null;

      const withCta = provider([
        announcement({ cta: { label: 'Open the production site', url: 'https://warren.ro/prod' } }),
      ]).getInAppNotification();
      expect(card(withCta).cta).to.deep.equal({
        label: 'Open the production site',
        url: 'https://warren.ro/prod',
      });
    });

    it('refuses a call to action that is not https, at render time', () => {
      // The daemon already checked it, but the string still arrives from the
      // network and lands in a control that opens a browser.
      expect(announcementCtaUrl({ label: 'go', url: 'https://warren.ro/x' })).to.equal(
        'https://warren.ro/x',
      );
      expect(announcementCtaUrl({ label: 'go', url: 'http://warren.ro/x' })).to.be.undefined;
      expect(announcementCtaUrl({ label: 'go', url: 'javascript:alert(1)' })).to.be.undefined;
      expect(announcementCtaUrl({ label: 'go', url: 'not a url' })).to.be.undefined;

      const unsafe = provider([
        announcement({ cta: { label: 'go', url: 'http://warren.ro/x' } }),
      ]).getInAppNotification();
      expect(card(unsafe).cta, 'the text still reaches the reader, the link does not').to.be.null;
    });

    it('hides the dismissed announcement and only that one', () => {
      const first = announcement({ id: 'a1', headline: 'first' });
      const second = announcement({ id: 'a2', headline: 'second' });

      expect(provider([first, second]).getInAppNotification().title).to.equal('first');
      expect(provider([first, second], ['a1']).getInAppNotification().title).to.equal('second');
      expect(provider([first, second], ['a1', 'a2']).mayDisplay()).to.be.false;
    });

    it('dismisses by the announcement id', () => {
      const dismiss = vi.fn();
      const notification = provider(
        [announcement({ id: 'a1' })],
        [],
        dismiss,
      ).getInAppNotification();
      card(notification).dismiss();
      expect(dismiss).toHaveBeenCalledExactlyOnceWith('a1');
    });

    it('leaves the operator notice non-dismissible', () => {
      // A notice is a live operator statement and clears from the same signal
      // that raised it; an announcement is an event the reader is done with.
      const notice = new WarrenNoticeNotificationProvider({
        notices: [{ id: 'n1', message: 'Payments are down, we are on it.', level: 'warning' }],
      }).getInAppNotification();
      expect(notice.action?.type).to.not.equal('announcement-card');
      expect(notice.action?.type).to.not.equal('close');
    });

    it('outranks the notice and every connection-state banner', () => {
      // A notice is not dismissible, so ranking it above the card would let a
      // long-lived operator statement bury a card whose code expires.
      const source = fs.readFileSync(NOTIFICATION_AREA_SOURCE, 'utf8');
      const rankOf = (name: string) => {
        const rank = source.indexOf(`new ${name}(`);
        expect(rank, `${name} is not in the notification area`).to.be.greaterThan(-1);
        return rank;
      };

      for (const outranked of [
        'WarrenNoticeNotificationProvider',
        'WarrenHostOfflineNotificationProvider',
        'ConnectingNotificationProvider',
        'ErrorNotificationProvider',
      ]) {
        expect(rankOf('WarrenAnnouncementNotificationProvider'), outranked).to.be.lessThan(
          rankOf(outranked),
        );
      }
    });
  });

  describe('the rendered card', () => {
    it('prints the headline once and the body under it', () => {
      const { html } = render({
        headline: 'Warren production is open',
        body: 'The doors are open.',
      });
      expect(html.split('Warren production is open')).to.have.lengthOf(2);
      expect(html).to.contain('The doors are open.');
    });

    it('puts the code in a selectable monospace block with a copy control', () => {
      const rendered = render({ voucherCode: 'ABCD-EFGH-JKMN-PQRS' });
      expect(rendered.html).to.contain('ABCD-EFGH-JKMN-PQRS');

      // `html { user-select: none }` is global, so a code rendered without an
      // override of its own cannot even be selected, let alone copied by hand.
      const rules = rulesFor(rendered, 'announcementVoucherCode');
      expect(rules).to.contain('user-select:text');
      expect(rules).to.contain('monospace');

      expect(rendered.html).to.contain('data-testid="announcementCopyVoucher"');
    });

    it('renders no code block at all when the announcement carries no offer', () => {
      const rendered = render({ voucherCode: null });
      expect(rendered.html).to.not.contain('data-testid="announcementVoucherCode"');
      expect(rendered.html).to.not.contain('data-testid="announcementCopyVoucher"');
    });

    it('renders the call to action as a button, only when there is one', () => {
      const withCta = render({
        cta: { label: 'Open the production site', url: 'https://warren.ro/prod' },
      });
      expect(withCta.html).to.contain('Open the production site');
      expect(withCta.html).to.contain('data-testid="announcementCta"');

      const withoutCta = render();
      expect(withoutCta.html).to.not.contain('data-testid="announcementCta"');
    });

    it('offers a dismiss control', () => {
      expect(render().html).to.contain('data-testid="announcementDismiss"');
    });

    it('fits the banner width in a single column', () => {
      // The banner is capped at 300px, so a row layout collapses. Everything
      // stacks.
      const rendered = render({
        voucherCode: 'ABCD-EFGH-JKMN-PQRS',
        cta: { label: 'Open the production site', url: 'https://warren.ro/prod' },
      });
      const rules = rulesFor(rendered, 'announcementCard');
      expect(rules).to.contain('flex-direction:column');
    });
  });

  describe('the notice banner it shares the connect screen with', () => {
    it('carries the same typographic hierarchy the card introduces', () => {
      // A notice keeps its one-line banner and its level label, because the
      // operator publishes no headline with it, but the title and the body of
      // that banner have to separate at a glance the same way.
      const sheet = new ServerStyleSheet();
      const html = renderToStaticMarkup(
        sheet.collectStyles(
          <div>
            <NotificationTitle data-testid="bannerTitle">Title</NotificationTitle>
            <NotificationSubtitleText data-testid="bannerSubtitle">Body</NotificationSubtitleText>
          </div>,
        ),
      );
      const rendered = { html, css: sheet.getStyleTags() };

      const sizeOf = (marker: string) => {
        const size = /font-size:(\d+)px/.exec(rulesFor(rendered, marker));
        expect(size, `${marker} declares no font size`).to.not.be.null;
        return Number(size![1]);
      };

      expect(sizeOf('bannerTitle')).to.be.greaterThan(sizeOf('bannerSubtitle'));
      // The banner floats over the scenery photograph, so the body text needs
      // more than the faintest tint on the alpha ladder.
      expect(rulesFor(rendered, 'bannerSubtitle')).to.contain('--color-white-alpha80');
    });
  });

  describe('the copy control', () => {
    it('writes the code the operator supplied, and says nothing about it on failure', async () => {
      const writeText = vi.fn().mockResolvedValue(undefined);
      const clipboard = { writeText } as unknown as Clipboard;

      expect(await copyToClipboard('ABCD-EFGH-JKMN-PQRS', clipboard)).to.be.true;
      expect(writeText).toHaveBeenCalledExactlyOnceWith('ABCD-EFGH-JKMN-PQRS');

      // The rejection can quote the value it refused, so the outcome is
      // reported and the message is dropped.
      const refusing = {
        writeText: vi.fn().mockRejectedValue(new Error('ABCD-EFGH-JKMN-PQRS is not permitted')),
      } as unknown as Clipboard;
      expect(await copyToClipboard('ABCD-EFGH-JKMN-PQRS', refusing)).to.be.false;
    });
  });

  describe('the persisted dismissal', () => {
    const settingsFile = path.join(USER_DATA, 'gui_settings.json');

    afterAll(() => fs.rmSync(USER_DATA, { recursive: true, force: true }));

    it('survives a reload, and hides that id only', () => {
      const first = new GuiSettings();
      first.load();
      expect(first.dismissedAnnouncements).to.deep.equal([]);
      first.dismissAnnouncement('a1');

      const reloaded = new GuiSettings();
      reloaded.load();
      expect(reloaded.dismissedAnnouncements).to.deep.equal(['a1']);
      expect(fs.readFileSync(settingsFile, 'utf8')).to.contain('a1');

      // A second dismissal joins the first, and a repeat does not grow the list.
      reloaded.dismissAnnouncement('a2');
      reloaded.dismissAnnouncement('a1');
      const again = new GuiSettings();
      again.load();
      expect(again.dismissedAnnouncements).to.deep.equal(['a1', 'a2']);
    });
  });
});

// A human eye is the only judge of "nice", so the same component the app
// renders is written out as a standalone page:
//
//   WARREN_CARD_PREVIEW=1 npx vitest run test/unit/warren-announcement-card.spec.tsx
//   open build/announcement-card-preview.html
describe('the preview page', () => {
  const previewPath = path.resolve(__dirname, '../../build/announcement-card-preview.html');
  let page = '';

  beforeAll(async () => {
    page = (await import('./announcement-card-preview')).renderAnnouncementCardPreview();
    if (process.env.WARREN_CARD_PREVIEW) {
      fs.mkdirSync(path.dirname(previewPath), { recursive: true });
      fs.writeFileSync(previewPath, page);
    }
  });

  it('renders the card the app ships, with the design tokens it needs', () => {
    expect(page).to.contain('--font-size-tiny');
    expect(page).to.contain('data-testid="announcementVoucherCode"');
    expect(page).to.contain('data-testid="announcementCta"');
  });

  it('reaches for no file beside it', () => {
    // Opened from the filesystem, a page that links a sibling asset renders
    // with no fonts and no icons, which is the one thing a preview cannot do.
    expect(page).to.not.contain("url('../fonts/");
    expect(page).to.not.contain('url(assets/');
    expect(page).to.contain('data:font/ttf;base64,');
    expect(page).to.contain('data:image/svg+xml;base64,');
  });

  it('defines every custom property it refers to', () => {
    // A preview that silently loses a colour is worse than no preview: it is
    // judged by eye, and a missing variable just paints nothing.
    // Declarations sit both in the `:root` block and inside the collected
    // rules, where a component defines its own locals.
    const declared = new Set(Array.from(page.matchAll(/(--[a-z0-9-]+)\s*:/g), (match) => match[1]));
    const used = new Set(Array.from(page.matchAll(/var\((--[a-z0-9-]+)\)/g), (match) => match[1]));
    const missing = Array.from(used).filter((name) => !declared.has(name));
    expect(missing, 'referenced but never declared').to.deep.equal([]);
  });
});
