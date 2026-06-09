import { closeToExpiry, hasExpired } from '../shared/account-expiry';
import {
  AccountDataError,
  AccountDataResponse,
  IAccountData,
  VoucherResponse,
  WarrenPubKey,
} from '../shared/daemon-rpc-types';
import { dateByAddingComponent, DateComponent } from '../shared/date-helper';
import log from '../shared/logging';
import { Scheduler } from '../shared/scheduler';

export type AccountFetchError = AccountDataError['error'] | 'cancelled';

interface IAccountFetchWatcher {
  onFinish: () => void;
  onError: (error: AccountFetchError) => void;
}

// Account data is valid for 1 minute unless the account has expired.
const ACCOUNT_DATA_VALIDITY_SECONDS = 60_000;
// Account data is valid for 10 seconds if the account has expired.
const ACCOUNT_DATA_EXPIRED_VALIDITY_SECONDS = 10_000;

// An account data cache that helps to throttle RPC requests to get_account_data and retain the
// cached value for 1 minute.
export default class AccountDataCache {
  private currentAccount?: WarrenPubKey;
  private validUntil?: Date;
  private performingFetch = false;
  private waitStrategy = new WaitStrategy();
  private fetchRetryScheduler = new Scheduler();
  private watchers: IAccountFetchWatcher[] = [];

  constructor(
    private fetchHandler: (number: WarrenPubKey) => Promise<AccountDataResponse>,
    private updateHandler: (data?: IAccountData) => void,
  ) {}

  public fetch(pubkey: WarrenPubKey, watcher?: IAccountFetchWatcher) {
    // invalidate cache if pubkey has changed
    if (pubkey !== this.currentAccount) {
      this.invalidate();
      this.currentAccount = pubkey;
    }

    // Only fetch if value has expired
    if (!this.isValid()) {
      if (watcher) {
        this.watchers.push(watcher);
      }

      this.fetchRetryScheduler.cancel();
      // If a scheduled retry is cancelled the fetchAttempt shouldn't be increased.
      this.waitStrategy.decrease();

      // Only fetch if there's no fetch for this pubkey in progress.
      if (!this.performingFetch) {
        void this.performFetch(pubkey);
      }
    } else if (watcher) {
      watcher.onFinish();
    }
  }

  public invalidate() {
    this.fetchRetryScheduler.cancel();
    this.waitStrategy.reset();

    this.performingFetch = false;
    this.validUntil = undefined;
    this.updateHandler();
    this.notifyWatchers((watcher) => {
      watcher.onError('cancelled');
    });
  }

  public handleVoucherResponse(pubkey: WarrenPubKey, voucherResponse: VoucherResponse) {
    if (pubkey === this.currentAccount && voucherResponse.type === 'success') {
      this.setValue({ expiry: voucherResponse.newExpiry });
    }
  }

  private setValue(accountData: IAccountData) {
    this.validUntil = this.getValidUntil(accountData);
    this.updateHandler(accountData);
    this.notifyWatchers((watcher) => watcher.onFinish());
  }

  private isValid() {
    return this.validUntil && this.validUntil > new Date();
  }

  private getValidUntil(accountData: IAccountData): Date {
    if (hasExpired(accountData.expiry)) {
      return new Date(Date.now() + ACCOUNT_DATA_EXPIRED_VALIDITY_SECONDS);
    } else {
      return new Date(Date.now() + ACCOUNT_DATA_VALIDITY_SECONDS);
    }
  }

  private async performFetch(pubkey: WarrenPubKey) {
    this.performingFetch = true;
    try {
      // it's possible for invalidate() to be called or for a fetch for a different pubkey
      // to start before this fetch completes, so checking if the current pubkey is the one
      // used is necessary below.
      const response = await this.fetchHandler(pubkey);
      if ('error' in response) {
        if (this.currentAccount === pubkey) {
          this.handleFetchError(pubkey, response.error);
        }
      } else {
        if (this.currentAccount === pubkey) {
          this.setValue(response);

          const refetchDelay = this.calculateRefetchDelay(response.expiry);
          if (refetchDelay) {
            this.scheduleFetch(pubkey, refetchDelay);
          }

          this.waitStrategy.reset();
        }
      }
    } catch {
      log.warn('Error occurred in account data fetch');
    } finally {
      this.performingFetch = false;
    }
  }

  private calculateRefetchDelay(accountExpiry: string) {
    const currentDate = new Date();
    const oneMinuteBeforeExpiry = dateByAddingComponent(accountExpiry, DateComponent.minute, -1);

    if (oneMinuteBeforeExpiry >= currentDate && closeToExpiry(accountExpiry)) {
      return oneMinuteBeforeExpiry.getTime() - currentDate.getTime();
    } else {
      return undefined;
    }
  }

  private handleFetchError(pubkey: WarrenPubKey, error: AccountDataError['error']) {
    // Warren-specific: a 404 from warren-api (mapped here as
    // 'no-subscription') is not a transient failure - it is a
    // semantic "the current pubkey has no active subscription yet"
    // state. Synthesize an epoch-past expiry so the renderer sees
    // the account as expired (Redux `expiredState: 'expired'`),
    // which makes `StateTriggeredNavigation` redirect the user to
    // `ExpiredAccountErrorView` with the "buy plan" CTA. Without
    // this redirect the user stays on the main view and a Connect
    // click triggers a doomed handshake that locks down the
    // firewall via the tunnel state machine's error branch.
    //
    // We still kick off the retry loop so the UI flips to "active"
    // the moment the user purchases a plan. `setValue` resolves any
    // pending watchers as `onFinish` (the data IS available - just
    // expired) and sets a 10 s cache validity window for `fetch()`
    // callers; the retry loop drives background polling beyond
    // that.
    if (error === 'no-subscription') {
      this.setValue({ expiry: new Date(0).toISOString() });
      this.scheduleRetry(pubkey, error);
      return;
    }

    this.notifyWatchers((w) => w.onError(error));
    if (error !== 'invalid-account') {
      this.scheduleRetry(pubkey, error);
    }
  }

  private scheduleRetry(pubkey: WarrenPubKey, error: AccountDataError['error']) {
    this.waitStrategy.increase();
    const delay = this.waitStrategy.delay();

    // Both `'communication'` (gRPC Unknown - could be the 404
    // pre-fix, transient network, or an undecoded API error) and
    // `'no-subscription'` (gRPC NOT_FOUND - the explicit 404 path
    // post-fix) are expected steady states for a freshly
    // bootstrapped Warren identity until the user purchases a plan.
    // The retry loop is essential (so the UI updates the moment a
    // subscription is purchased) but logging at warn level for
    // every retry floods the dev console. Demote to debug for both
    // expected variants and keep warn for genuinely unusual failure
    // modes (too-many-devices, list-devices).
    if (error === 'communication') {
      log.debug(`Account data fetch: retrying in ${delay} ms (no subscription yet?)`);
    } else if (error === 'no-subscription') {
      log.debug(`Account data fetch: 404 - no active subscription, retrying in ${delay} ms`);
    } else {
      log.warn(`Failed to fetch account data (${error}). Retrying in ${delay} ms`);
    }

    this.scheduleFetch(pubkey, delay);
  }

  private scheduleFetch(pubkey: WarrenPubKey, delay: number) {
    this.fetchRetryScheduler.schedule(() => {
      void this.performFetch(pubkey);
    }, delay);
  }

  private notifyWatchers(notify: (watcher: IAccountFetchWatcher) => void) {
    this.watchers.splice(0).forEach(notify);
  }
}

const MAX_ATTEMPT = 9;

class WaitStrategy {
  private counter = 0;

  public increase() {
    if (this.counter < MAX_ATTEMPT) {
      this.counter += 1;
    }
  }
  public decrease() {
    if (this.counter > 0) {
      this.counter -= 1;
    }
  }

  public reset() {
    this.counter = 0;
  }

  public delay(): number {
    // Max delay: 2^11 = 2048
    return Math.pow(2, this.counter + 2) * 1000;
  }
}
