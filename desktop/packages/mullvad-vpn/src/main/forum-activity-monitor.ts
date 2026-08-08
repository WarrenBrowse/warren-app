import { unreadForSlot } from '../shared/forum-identity';
import { ForumActivityNotificationProvider, SystemNotification } from '../shared/notifications';

export interface ForumActivityMonitorDelegate {
  notify(notification: SystemNotification): void;
  /** Drives the dot on the tray icon, independently of any banner's lifetime. */
  showForumActivityIndicator(unread: boolean): void;
}

/**
 * Turns the broadcast forum digest into a banner and a tray dot.
 *
 * Everything it needs is already here: the daemon has checked the
 * document's signature and freshness, and only this process knows which
 * slot belongs to this installation. So the whole feature costs no
 * request, and the server is never told that this account is watching.
 *
 * Two rules do most of the work:
 *
 * A banner is for activity that arrived while this run was watching. The
 * count already waiting when the app starts gets the dot but no banner,
 * otherwise every relaunch would re-announce the same notifications.
 *
 * An absent digest means unknown, never zero. The daemon drops the
 * document when it cannot refresh it, and reading that gap as "all read"
 * would fire a banner for notifications the user has already seen as soon
 * as it came back.
 *
 * Reading on the forum through any other channel needs no handling: it
 * advances the reader's own bookmark there, the next digest carries a
 * lower count, and the badge, the banner and the dot follow the same
 * number.
 */
export default class ForumActivityMonitor {
  private digest: string | null = null;
  private slot: number | null = null;
  private enabled = true;
  private indicator = false;

  // Count this run has already accounted for, `undefined` until a digest
  // has actually been seen for the current slot: what separates "nothing
  // new" from "nothing known yet".
  private acknowledged?: number;

  public constructor(private delegate: ForumActivityMonitorDelegate) {}

  public setDigest(digest: string | null | undefined) {
    this.digest = digest ?? null;
    this.refresh();
  }

  public setSlot(slot: number | null) {
    if (slot === this.slot) {
      return;
    }
    // Another forum account, or none: its predecessor's count says nothing
    // about this one.
    this.slot = slot;
    this.acknowledged = undefined;
    this.refresh();
  }

  public setEnabled(enabled: boolean) {
    if (enabled === this.enabled) {
      return;
    }
    this.enabled = enabled;
    this.refresh();
  }

  private refresh() {
    const unread = unreadForSlot(this.digest, this.slot);
    this.showIndicator(this.enabled && unread > 0);

    if (this.digest === null || this.slot === null) {
      // Keep the watermark: this is a gap in what we know, not a read.
      return;
    }

    const previous = this.acknowledged;
    // Advanced even while the setting is off, so turning it back on does
    // not announce what happened in the meantime.
    this.acknowledged = unread;

    if (previous === undefined || unread <= previous || !this.enabled) {
      return;
    }

    this.delegate.notify(new ForumActivityNotificationProvider({ unread }).getSystemNotification());
  }

  private showIndicator(value: boolean) {
    if (value === this.indicator) {
      return;
    }
    this.indicator = value;
    this.delegate.showForumActivityIndicator(value);
  }
}
