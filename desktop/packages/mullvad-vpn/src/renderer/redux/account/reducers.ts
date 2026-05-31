import { AccountDataError, WarrenPubKey } from '../../../shared/daemon-rpc-types';
import { ReduxAction } from '../store';

type LoginMethod = 'existing_account' | 'new_account';
type ExpiredState = 'expired' | 'time_added';

export type LoginState =
  | { type: 'none'; deviceRevoked: boolean }
  | { type: 'logging in'; method: LoginMethod }
  | { type: 'ok'; method: LoginMethod; expiredState?: ExpiredState }
  | { type: 'failed'; method: 'existing_account'; error: AccountDataError['error'] }
  | { type: 'failed'; method: 'new_account'; error: Error };
export interface IAccountReduxState {
  pubkey?: WarrenPubKey;
  pubkeyHistory?: WarrenPubKey;
  expiry?: string; // ISO8601
  status: LoginState;
}

const initialState: IAccountReduxState = {
  pubkey: undefined,
  pubkeyHistory: undefined,
  expiry: undefined,
  status: { type: 'none', deviceRevoked: false },
};

export default function (
  state: IAccountReduxState = initialState,
  action: ReduxAction,
): IAccountReduxState {
  switch (action.type) {
    case 'START_LOGIN':
      return {
        ...state,
        status: { type: 'logging in', method: 'existing_account' },
        pubkey: action.pubkey,
      };
    case 'LOGGED_IN':
      return {
        ...state,
        status: {
          type: 'ok',
          method: 'existing_account',
        },
        pubkey: action.pubkey,
      };
    case 'LOGIN_FAILED':
      return {
        ...state,
        status: { type: 'failed', method: 'existing_account', error: action.error },
      };
    case 'LOGGED_OUT':
      return {
        ...state,
        status: { type: 'none', deviceRevoked: false },
        pubkey: undefined,
        expiry: undefined,
      };
    case 'RESET_LOGIN_ERROR':
      return {
        ...state,
        status: { type: 'none', deviceRevoked: false },
      };
    case 'DEVICE_REVOKED':
      return {
        ...state,
        status: { type: 'none', deviceRevoked: true },
      };
    case 'START_CREATE_ACCOUNT':
      return {
        ...state,
        status: { type: 'logging in', method: 'new_account' },
      };
    case 'CREATE_ACCOUNT_FAILED':
      return {
        ...state,
        status: { type: 'failed', method: 'new_account', error: action.error },
      };
    case 'ACCOUNT_CREATED':
      return {
        ...state,
        status: {
          type: 'ok',
          method: 'new_account',
          expiredState: 'expired',
        },
        pubkey: action.pubkey,
        expiry: action.expiry,
      };
    case 'ACCOUNT_SETUP_FINISHED':
      return {
        ...state,
        status: { type: 'ok', method: 'existing_account' },
      };
    case 'UPDATE_PUBKEY':
      return {
        ...state,
        pubkey: action.pubkey,
      };
    case 'UPDATE_PUBKEY_HISTORY':
      return {
        ...state,
        pubkeyHistory: action.pubkeyHistory,
      };
    case 'UPDATE_ACCOUNT_EXPIRY': {
      const status = { ...state.status };
      if (status.type === 'ok') {
        if (action.expired) {
          status.expiredState = 'expired';
        } else if (
          status.expiredState === 'expired' &&
          action.expired === false &&
          // If the system clock changes from something that makes the expiry out of time, backwards
          // to something that is before the expiry, then the time added view shouldn't be displayed
          // since the expiry hasn't changed.
          state.expiry !== action.expiry
        ) {
          status.expiredState = 'time_added';
        } else {
          status.expiredState = undefined;
        }
      }

      return {
        ...state,
        expiry: action.expiry,
        status,
      };
    }
  }

  return state;
}
