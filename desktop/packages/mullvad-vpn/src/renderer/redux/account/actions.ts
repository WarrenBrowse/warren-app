import { hasExpired } from '../../../shared/account-expiry';
import { WarrenPubKey } from '../../../shared/daemon-rpc-types';
import { ForumIdentity } from '../../../shared/forum-identity';
import { RenewalUiState } from '../../../shared/renewal';

interface ILoggedInAction {
  type: 'LOGGED_IN';
  pubkey: WarrenPubKey;
}

interface ILoggedOutAction {
  type: 'LOGGED_OUT';
}

interface IDeviceRevokedAction {
  type: 'DEVICE_REVOKED';
}

interface IStartCreateAccount {
  type: 'START_CREATE_ACCOUNT';
}

interface ICreateAccountFailed {
  type: 'CREATE_ACCOUNT_FAILED';
  error: Error;
}

interface IAccountAwaitingBackup {
  type: 'ACCOUNT_AWAITING_BACKUP';
  pubkey: WarrenPubKey;
}

interface IAccountCreated {
  type: 'ACCOUNT_CREATED';
  pubkey: WarrenPubKey;
  expiry: string;
}

interface IAccountSetupFinished {
  type: 'ACCOUNT_SETUP_FINISHED';
}

interface IUpdatePubKeyHistoryAction {
  type: 'UPDATE_PUBKEY_HISTORY';
  pubkeyHistory?: WarrenPubKey;
}

interface IUpdateAccountExpiryAction {
  type: 'UPDATE_ACCOUNT_EXPIRY';
  expiry?: string;
  expired?: boolean;
}

interface IUpdateRenewalStateAction {
  type: 'UPDATE_RENEWAL_STATE';
  renewalState?: RenewalUiState;
}

interface IUpdatePurchaseInFlightAction {
  type: 'UPDATE_PURCHASE_IN_FLIGHT';
  purchaseInFlight: boolean;
}

interface IUpdateForumIdentityAction {
  type: 'UPDATE_FORUM_IDENTITY';
  forumIdentity?: ForumIdentity;
}

interface IUpdateForumUnreadAction {
  type: 'UPDATE_FORUM_UNREAD';
  forumUnread: number;
}

export type AccountAction =
  | ILoggedInAction
  | ILoggedOutAction
  | IDeviceRevokedAction
  | IStartCreateAccount
  | ICreateAccountFailed
  | IAccountAwaitingBackup
  | IAccountCreated
  | IAccountSetupFinished
  | IUpdatePubKeyHistoryAction
  | IUpdateAccountExpiryAction
  | IUpdatePurchaseInFlightAction
  | IUpdateRenewalStateAction
  | IUpdateForumIdentityAction
  | IUpdateForumUnreadAction;

function loggedIn(pubkey: WarrenPubKey): ILoggedInAction {
  return {
    type: 'LOGGED_IN',
    pubkey,
  };
}

function loggedOut(): ILoggedOutAction {
  return {
    type: 'LOGGED_OUT',
  };
}

function deviceRevoked(): IDeviceRevokedAction {
  return {
    type: 'DEVICE_REVOKED',
  };
}

function startCreateAccount(): IStartCreateAccount {
  return {
    type: 'START_CREATE_ACCOUNT',
  };
}

function createAccountFailed(error: Error): ICreateAccountFailed {
  return {
    type: 'CREATE_ACCOUNT_FAILED',
    error,
  };
}

function accountAwaitingBackup(pubkey: WarrenPubKey): IAccountAwaitingBackup {
  return {
    type: 'ACCOUNT_AWAITING_BACKUP',
    pubkey,
  };
}

function accountCreated(pubkey: WarrenPubKey, expiry: string): IAccountCreated {
  return {
    type: 'ACCOUNT_CREATED',
    pubkey,
    expiry,
  };
}

function accountSetupFinished(): IAccountSetupFinished {
  return { type: 'ACCOUNT_SETUP_FINISHED' };
}

function updatePubKeyHistory(pubkeyHistory?: WarrenPubKey): IUpdatePubKeyHistoryAction {
  return {
    type: 'UPDATE_PUBKEY_HISTORY',
    pubkeyHistory,
  };
}

function updateAccountExpiry(expiry?: string): IUpdateAccountExpiryAction {
  return {
    type: 'UPDATE_ACCOUNT_EXPIRY',
    expiry,
    expired: expiry === undefined ? undefined : hasExpired(expiry),
  };
}

function updatePurchaseInFlight(purchaseInFlight: boolean): IUpdatePurchaseInFlightAction {
  return {
    type: 'UPDATE_PURCHASE_IN_FLIGHT',
    purchaseInFlight,
  };
}

function updateRenewalState(renewalState?: RenewalUiState): IUpdateRenewalStateAction {
  return {
    type: 'UPDATE_RENEWAL_STATE',
    renewalState,
  };
}

function updateForumIdentity(forumIdentity?: ForumIdentity): IUpdateForumIdentityAction {
  return {
    type: 'UPDATE_FORUM_IDENTITY',
    forumIdentity,
  };
}

function updateForumUnread(forumUnread: number): IUpdateForumUnreadAction {
  return {
    type: 'UPDATE_FORUM_UNREAD',
    forumUnread,
  };
}

export default {
  loggedIn,
  loggedOut,
  deviceRevoked,
  startCreateAccount,
  createAccountFailed,
  accountAwaitingBackup,
  accountCreated,
  accountSetupFinished,
  updatePubKeyHistory,
  updateAccountExpiry,
  updatePurchaseInFlight,
  updateRenewalState,
  updateForumIdentity,
  updateForumUnread,
};
