/**
 * What this GUI does with the broadcast forum digest, which is what sets how
 * often the daemon fetches it. The daemon fetches nothing for a GUI with no
 * forum account or with the forum notifications setting off, fetches on a
 * slow cadence while the window is hidden (the tray dot and its banner are
 * the only readers), and every minute while the window is on screen.
 */
export type ForumDigestDemand = 'none' | 'background' | 'foreground';

export interface ForumDigestReader {
  notificationsEnabled: boolean;
  hasForumAccount: boolean;
  windowVisible: boolean;
}

export function forumDigestDemand(reader: ForumDigestReader): ForumDigestDemand {
  if (!reader.notificationsEnabled || !reader.hasForumAccount) {
    return 'none';
  }
  return reader.windowVisible ? 'foreground' : 'background';
}

/**
 * Sends the demand to the daemon when it changes. The daemon starts every
 * run at `none`, so a reconnect is reported again from scratch, and a send
 * that failed is retried at the next update rather than taken as delivered.
 */
export class ForumDigestDemandReporter {
  private reported?: ForumDigestDemand;

  public constructor(private send: (demand: ForumDigestDemand) => Promise<void>) {}

  public update(demand: ForumDigestDemand) {
    if (demand === this.reported) {
      return;
    }
    this.reported = demand;
    this.send(demand).catch(() => {
      if (this.reported === demand) {
        this.reported = undefined;
      }
    });
  }

  public reset() {
    this.reported = undefined;
  }
}
