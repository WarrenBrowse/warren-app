import { banInForce } from '../shared/account-standing';
import { WarrenAccountStanding } from '../shared/daemon-rpc-types';

// The longest delay a Node timer honours; a longer one fires at once.
const MAX_TIMER_MS = 2 ** 31 - 1;

/**
 * Reports each time a ban on the logged-in account starts or stops holding
 * (warren-core doc 105). A ban stops holding in two ways: a new standing drops
 * it (an operator's early lift), or its lapse passes, which no new standing
 * announces, so the lapse has a timer of its own.
 */
export default class BanLiftWatch {
  private banned = false;
  private standing?: WarrenAccountStanding | null;
  private lapseTimer?: NodeJS.Timeout;

  public constructor(private readonly onChange: (banned: boolean) => void) {}

  public observe(standing: WarrenAccountStanding | null | undefined): void {
    this.standing = standing;
    clearTimeout(this.lapseTimer);
    this.lapseTimer = undefined;

    const ban = banInForce(standing, Date.now());
    const banned = ban !== undefined;
    if (banned !== this.banned) {
      this.banned = banned;
      this.onChange(banned);
    }
    if (ban?.lapsesAtUnixSecs != null) {
      const untilLapseMs = ban.lapsesAtUnixSecs * 1000 - Date.now();
      // Past the timer's reach, wake up at its limit and look again.
      this.lapseTimer = setTimeout(
        () => this.observe(this.standing),
        Math.min(MAX_TIMER_MS, Math.max(0, untilLapseMs) + 1),
      );
    }
  }

  public dispose(): void {
    clearTimeout(this.lapseTimer);
    this.lapseTimer = undefined;
  }
}
