import { hasExpired } from '../../../shared/account-expiry';
import { AccountDataError, WarrenPubKey } from '../../../shared/daemon-rpc-types';

interface IStartLoginAction {
  type: 'START_LOGIN';
  pubkey: WarrenPubKey;
}

interface ILoggedInAction {
  type: 'LOGGED_IN';
  pubkey: WarrenPubKey;
}

interface ILoginFailedAction {
  type: 'LOGIN_FAILED';
  error: AccountDataError['error'];
}

interface ILoggedOutAction {
  type: 'LOGGED_OUT';
}

interface IResetLoginErrorAction {
  type: 'RESET_LOGIN_ERROR';
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

interface IAccountCreated {
  type: 'ACCOUNT_CREATED';
  pubkey: WarrenPubKey;
  expiry: string;
}

interface IAccountSetupFinished {
  type: 'ACCOUNT_SETUP_FINISHED';
}

interface IUpdatePubKeyAction {
  type: 'UPDATE_PUBKEY';
  pubkey: WarrenPubKey;
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

export type AccountAction =
  | IStartLoginAction
  | ILoggedInAction
  | ILoginFailedAction
  | ILoggedOutAction
  | IResetLoginErrorAction
  | IDeviceRevokedAction
  | IStartCreateAccount
  | ICreateAccountFailed
  | IAccountCreated
  | IAccountSetupFinished
  | IUpdatePubKeyAction
  | IUpdatePubKeyHistoryAction
  | IUpdateAccountExpiryAction;

function startLogin(pubkey: WarrenPubKey): IStartLoginAction {
  return {
    type: 'START_LOGIN',
    pubkey,
  };
}

function loggedIn(pubkey: WarrenPubKey): ILoggedInAction {
  return {
    type: 'LOGGED_IN',
    pubkey,
  };
}

function loginFailed(error: AccountDataError['error']): ILoginFailedAction {
  return {
    type: 'LOGIN_FAILED',
    error,
  };
}

function loggedOut(): ILoggedOutAction {
  return {
    type: 'LOGGED_OUT',
  };
}

function resetLoginError(): IResetLoginErrorAction {
  return {
    type: 'RESET_LOGIN_ERROR',
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

function updatePubKey(pubkey: WarrenPubKey): IUpdatePubKeyAction {
  return {
    type: 'UPDATE_PUBKEY',
    pubkey,
  };
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

export default {
  startLogin,
  loggedIn,
  loginFailed,
  loggedOut,
  resetLoginError,
  deviceRevoked,
  startCreateAccount,
  createAccountFailed,
  accountCreated,
  accountSetupFinished,
  updatePubKey,
  updatePubKeyHistory,
  updateAccountExpiry,
};
