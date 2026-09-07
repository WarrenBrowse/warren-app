import { closeToExpiry, hasExpired } from '../shared/account-expiry';
import { isBetaBuild } from '../shared/constants/product-env';
import {
  DeviceEvent,
  DeviceState,
  IAccountData,
  LogoutSource,
  TunnelState,
  VoucherResponse,
  WarrenPubKey,
} from '../shared/daemon-rpc-types';
import log from '../shared/logging';
import {
  AccountExpiredNotificationProvider,
  CloseToAccountExpiryNotificationProvider,
  SystemNotificationCategory,
} from '../shared/notifications';
import { Scheduler } from '../shared/scheduler';
import AccountDataCache from './account-data-cache';
import { DaemonRpc } from './daemon-rpc';
import { IpcMainEventChannel } from './ipc-event-channel';
import { NotificationSender } from './notification-controller';
import { systemTimeMonitor } from './system-time-monitor';
import { TunnelStateProvider } from './tunnel-state';

export interface LocaleProvider {
  getLocale(): string;
}

export interface AccountDelegate {
  onDeviceEvent(): void;
  // Fired whenever fresh account data (expiry included) lands: the
  // renewal scheduler re-evaluates on it instead of waiting for its
  // slow periodic recheck.
  onAccountData?(): void;
}

export interface AccountOptions {
  // Injectable for tests; the build constant in production.
  betaBuild: boolean;
}

export default class Account {
  private accountDataValue?: IAccountData = undefined;
  private accountHistoryValue?: WarrenPubKey = undefined;
  private expiryNotificationFrequencyScheduler = new Scheduler();
  private firstExpiryNotificationScheduler = new Scheduler();

  private hasExpired = false;

  private accountDataCache = new AccountDataCache(
    (pubkey) => {
      return this.daemonRpc.getAccountData(pubkey);
    },
    (accountData) => {
      this.handleAccountData(accountData);
    },
  );

  private deviceStateValue?: DeviceState;

  public constructor(
    private delegate: AccountDelegate & TunnelStateProvider & LocaleProvider & NotificationSender,
    private daemonRpc: DaemonRpc,
    private options: AccountOptions = { betaBuild: isBetaBuild },
  ) {
    this.monitorExpiredChange();
  }

  public get accountData() {
    return this.accountDataValue;
  }

  public get accountHistory() {
    return this.accountHistoryValue;
  }

  public get deviceState() {
    return this.deviceStateValue;
  }

  public registerIpcListeners() {
    IpcMainEventChannel.account.handleCreate(() => this.createNewAccount());
    IpcMainEventChannel.account.handleLogout((source) => this.logout(source));
    IpcMainEventChannel.account.handleGetWarrenMnemonic(() => this.daemonRpc.getWarrenMnemonic());
    IpcMainEventChannel.account.handleSetWarrenMnemonic((mnemonic: string) =>
      this.daemonRpc.setWarrenMnemonic(mnemonic),
    );
    IpcMainEventChannel.account.handleSubmitVoucher((voucherCode: string) =>
      this.submitVoucher(voucherCode),
    );
    IpcMainEventChannel.account.handleUpdateData(() => this.updateAccountData());
  }

  // Shared by the IPC handler (manual voucher entry) and the
  // main-process purchase poll (wpid auto-redeem): a successful
  // redemption must update the cached expiry and notify the renderer
  // no matter who submitted.
  public submitVoucher = async (voucherCode: string): Promise<VoucherResponse> => {
    const currentPubKey = this.getWarrenPubKey();
    const response = await this.daemonRpc.submitVoucher(voucherCode);

    if (currentPubKey) {
      this.accountDataCache.handleVoucherResponse(currentPubKey, response);
    }

    return response;
  };

  public isLoggedIn(): boolean {
    return this.deviceState?.type === 'logged in';
  }

  public updateAccountData = (): Promise<void> => {
    if (!this.daemonRpc.isConnected || !this.isLoggedIn()) {
      return Promise.resolve();
    }
    return new Promise<void>((resolve, reject) => {
      this.accountDataCache.fetch(this.getWarrenPubKey()!, {
        onFinish: () => resolve(),
        onError: (error) => reject(new Error(`Account data fetch failed: ${error}`)),
      });
    });
  };

  public async createNewAccount(): Promise<string> {
    let pubkey: string;
    try {
      pubkey = await this.daemonRpc.createNewAccount();
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to create account: ${error.message}`);
      throw error;
    }
    if (this.options.betaBuild) {
      void this.activateBetaAccess();
    }
    return pubkey;
  }

  public detectStaleAccountExpiry(tunnelState: TunnelState) {
    const expired = !this.accountData || hasExpired(this.accountData.expiry);

    // It's likely that the account expiry is stale if the daemon managed to establish the tunnel.
    if (tunnelState.state === 'connected' && expired) {
      log.info('Detected the stale account expiry.');
      this.accountDataCache.invalidate();
    }
  }

  public handleDeviceEvent(deviceEvent: DeviceEvent) {
    this.delegate.closeNotificationsInCategory(SystemNotificationCategory.expiry);

    this.deviceStateValue = deviceEvent.deviceState;

    void this.updateAccountHistory();
    this.delegate.onDeviceEvent();

    // When logging out the renderer process needs to receive the device update before the account
    // data update. This means that the ipc-call `account.notifyDevice` needs to be called before
    // invalidating the accountDateCache since that triggers the ipc-call `account.notify`.
    IpcMainEventChannel.account.notifyDevice?.(deviceEvent);

    switch (deviceEvent.deviceState.type) {
      case 'logged in':
        this.accountDataCache.fetch(deviceEvent.deviceState.warrenIdentity.pubkey);
        break;
      case 'logged out':
      case 'revoked':
        this.accountDataCache.invalidate();
        break;
    }
  }

  public setAccountHistory(accountHistory?: WarrenPubKey) {
    this.accountHistoryValue = accountHistory;

    IpcMainEventChannel.accountHistory.notify?.(accountHistory);
  }

  // This function monitors if the account is expired due to system clock changes.
  private monitorExpiredChange() {
    systemTimeMonitor(() => {
      const expired = this.accountData && hasExpired(this.accountData.expiry);
      if (expired !== this.hasExpired) {
        this.handleAccountData(this.accountData);
      }
    });
  }

  // A beta wallet is granted its access the moment it exists, so a first
  // run that never reaches the wizard's activation step (the window hid,
  // the user quit) does not leave the wallet unknown to the API and every
  // screen saying "out of time" (topic 195). The server call is
  // idempotent; the wizard step and the "refresh beta access" button
  // remain as the visible confirmation and the offline retry.
  private async activateBetaAccess(): Promise<void> {
    try {
      const response = await this.submitVoucher('');
      if (response.type !== 'success') {
        log.info(`Beta access not activated at creation (${response.type}), the wizard retries`);
      }
    } catch (e) {
      const error = e as Error;
      log.info(`Beta access activation at creation failed, the wizard retries: ${error.message}`);
    }
  }

  private async logout(source: LogoutSource): Promise<void> {
    try {
      await this.daemonRpc.logoutAccount(source);

      this.delegate.closeNotificationsInCategory(SystemNotificationCategory.expiry);
      this.expiryNotificationFrequencyScheduler.cancel();
      this.firstExpiryNotificationScheduler.cancel();
    } catch (e) {
      const error = e as Error;
      log.info(`Failed to logout: ${error.message}`);

      throw error;
    }
  }

  private handleAccountData(accountData?: IAccountData) {
    this.accountDataValue = accountData;
    this.hasExpired = this.accountData !== undefined && hasExpired(this.accountData?.expiry);
    IpcMainEventChannel.account.notify?.(this.accountData);
    this.delegate.onAccountData?.();
    this.showNotifications();
  }

  private showNotifications() {
    if (this.accountData) {
      const expiredNotification = new AccountExpiredNotificationProvider({
        accountExpiry: this.accountData.expiry,
        tunnelState: this.delegate.getTunnelState(),
        betaBuild: this.options.betaBuild,
      });
      const closeToExpiryNotification = new CloseToAccountExpiryNotificationProvider({
        accountExpiry: this.accountData.expiry,
        locale: this.delegate.getLocale(),
      });

      if (expiredNotification.mayDisplay()) {
        this.expiryNotificationFrequencyScheduler.cancel();
        this.firstExpiryNotificationScheduler.cancel();
        this.delegate.notify(expiredNotification.getSystemNotification());
      } else if (hasExpired(this.accountData.expiry)) {
        // Expired but not announced (tunnel not disconnected, or a beta
        // wallet not yet activated): nothing to schedule. The branches
        // below compute a delay from a future expiry; on a past one it is
        // negative, setTimeout clamps it to 1 ms, and this method re-ran
        // itself about a thousand times a second until the state changed.
        this.expiryNotificationFrequencyScheduler.cancel();
        this.firstExpiryNotificationScheduler.cancel();
      } else if (
        !this.expiryNotificationFrequencyScheduler.isRunning &&
        closeToExpiryNotification.mayDisplay()
      ) {
        this.firstExpiryNotificationScheduler.cancel();
        this.delegate.notify(closeToExpiryNotification.getSystemNotification());

        const twelveHours = 12 * 60 * 60 * 1000;
        const remainingMilliseconds = new Date(this.accountData.expiry).getTime() - Date.now();
        const delay = Math.min(twelveHours, remainingMilliseconds);
        this.expiryNotificationFrequencyScheduler.schedule(() => this.showNotifications(), delay);
      } else if (!closeToExpiry(this.accountData.expiry)) {
        this.expiryNotificationFrequencyScheduler.cancel();
        // If no longer close to expiry, all previous notifications should be closed
        this.delegate.closeNotificationsInCategory(SystemNotificationCategory.expiry);

        const expiry = new Date(this.accountData.expiry).getTime();
        const now = new Date().getTime();
        const threeDays = 3 * 24 * 60 * 60 * 1000;
        // Add 10 seconds to be on the safe side. Never make it longer than a 24 days since
        // the timeout needs to fit into a signed 32-bit integer.
        const timeout = Math.min(expiry - now - threeDays + 10_000, 24 * 24 * 60 * 60 * 1000);
        this.firstExpiryNotificationScheduler.schedule(() => this.showNotifications(), timeout);
      }
    }
  }

  private async updateAccountHistory(): Promise<void> {
    try {
      this.setAccountHistory(await this.daemonRpc.getAccountHistory());
    } catch (e) {
      const error = e as Error;
      log.error(`Failed to fetch the account history: ${error.message}`);
    }
  }

  private getWarrenPubKey(): WarrenPubKey | undefined {
    return this.deviceState?.type === 'logged in'
      ? this.deviceState.warrenIdentity.pubkey
      : undefined;
  }
}
