import { Url } from '../../../shared/constants';
import { WarrenAnnouncement, WarrenAnnouncementCta } from '../../../shared/daemon-rpc-types';
import {
  InAppAnnouncementCard,
  InAppNotification,
  InAppNotificationProvider,
} from '../../../shared/notifications';

interface WarrenAnnouncementNotificationContext {
  // Launch announcements the daemon verified against the pinned server key and
  // already filtered for expiry and this app's version. Each one is
  // display-ready, this account's voucher code included.
  announcements: WarrenAnnouncement[];
  // Announcement ids the user has put away, from the persisted GUI settings.
  dismissedIds: string[];
  dismiss: (id: string) => void;
}

// Operator launch announcement, rendered as a card on the connect screen.
//
// Ranked above the broadcast notice, which is the opposite of what their
// relative importance suggests, for a mechanical reason: a notice is not
// dismissible, so a long-lived one would bury the card for as long as it
// stands, and the card carries a code that stops being worth anything after
// the campaign. The card steps aside on its own the moment it is dismissed.
export class WarrenAnnouncementNotificationProvider implements InAppNotificationProvider {
  public constructor(private context: WarrenAnnouncementNotificationContext) {}

  public mayDisplay = () => this.current() !== undefined;

  public getInAppNotification(): InAppNotification {
    const announcement = this.current()!;
    const card: InAppAnnouncementCard = {
      id: announcement.id,
      body: announcement.body,
      voucherCode: announcement.voucherCode,
      cta: safeCta(announcement.cta),
      dismiss: () => this.context.dismiss(announcement.id),
    };
    return {
      indicator: indicatorFor(announcement.level),
      // The operator's own headline IS the title. Stacking a level word on top
      // of it would demote the words they wrote to a subtitle and say nothing
      // the indicator does not already say.
      title: announcement.headline,
      subtitle: announcement.body,
      action: { type: 'announcement-card', announcement: card },
    };
  }

  private current(): WarrenAnnouncement | undefined {
    return this.context.announcements.find(
      (announcement) => !this.context.dismissedIds.includes(announcement.id),
    );
  }
}

// The last gate before a control that opens a browser. The daemon already
// refused anything but https, and the envelope is signed, but this string
// still arrives from the network and the check costs nothing here.
export function announcementCtaUrl(cta: WarrenAnnouncementCta): Url | undefined {
  let parsed: URL;
  try {
    parsed = new URL(cta.url);
  } catch {
    return undefined;
  }
  return parsed.protocol === 'https:' ? (parsed.href as Url) : undefined;
}

function safeCta(cta: WarrenAnnouncementCta | null): InAppAnnouncementCard['cta'] {
  if (cta === null) {
    return null;
  }
  const url = announcementCtaUrl(cta);
  return url === undefined ? null : { label: cta.label, url };
}

function indicatorFor(level: WarrenAnnouncement['level']): InAppNotification['indicator'] {
  switch (level) {
    case 'error':
      return 'error';
    case 'warning':
      return 'warning';
    default:
      return 'success';
  }
}
