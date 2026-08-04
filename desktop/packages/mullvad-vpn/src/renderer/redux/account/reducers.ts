import { WarrenPubKey } from '../../../shared/daemon-rpc-types';
import { RenewalUiState } from '../../../shared/renewal';
import { ReduxAction } from '../store';

type LoginMethod = 'existing_account' | 'new_account';
type ExpiredState = 'expired' | 'time_added';

export type LoginState =
  | { type: 'none'; deviceRevoked: boolean }
  | { type: 'logging in'; method: LoginMethod }
  // A new account has been minted + logged in daemon-side, but the GUI
  // is holding on the login screen until the user backs up the freshly
  // generated recovery phrase. Carries the new pubkey so the backup
  // step can finalize via `accountCreated`.
  | { type: 'backup-pending'; pubkey: WarrenPubKey }
  | { type: 'ok'; method: LoginMethod; expiredState?: ExpiredState }
  | { type: 'failed'; method: 'new_account'; error: Error };
export interface IAccountReduxState {
  pubkey?: WarrenPubKey;
  pubkeyHistory?: WarrenPubKey;
  expiry?: string; // ISO8601
  status: LoginState;
  // Mirror of the main-process purchase poll (app-initiated checkout,
  // doc 35). Drives the "Checking..." labels in the paywall views.
  purchaseInFlight: boolean;
  // Device-held auto-renewal mandate display state (warren-core doc 65); undefined
  // when the logged-in account holds none.
  renewalState?: RenewalUiState;
  // This wallet's community-forum handle. Undefined until the user has signed
  // in to the forum at least once: the derivation is keyed server side, so the
  // app learns it from the login response and cannot compute it.
  forumHandle?: string;
}

const initialState: IAccountReduxState = {
  pubkey: undefined,
  pubkeyHistory: undefined,
  expiry: undefined,
  status: { type: 'none', deviceRevoked: false },
  purchaseInFlight: false,
  renewalState: undefined,
  forumHandle: undefined,
};

export default function (
  state: IAccountReduxState = initialState,
  action: ReduxAction,
): IAccountReduxState {
  switch (action.type) {
    case 'LOGGED_IN':
      return {
        ...state,
        status: {
          type: 'ok',
          method: 'existing_account',
        },
        pubkey: action.pubkey,
      };
    case 'LOGGED_OUT':
      return {
        ...state,
        status: { type: 'none', deviceRevoked: false },
        pubkey: undefined,
        expiry: undefined,
        forumHandle: undefined,
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
    case 'ACCOUNT_AWAITING_BACKUP':
      return {
        ...state,
        status: { type: 'backup-pending', pubkey: action.pubkey },
        pubkey: action.pubkey,
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
    case 'UPDATE_PURCHASE_IN_FLIGHT':
      return {
        ...state,
        purchaseInFlight: action.purchaseInFlight,
      };
    case 'UPDATE_RENEWAL_STATE':
      return {
        ...state,
        renewalState: action.renewalState,
      };
    case 'UPDATE_FORUM_HANDLE':
      return {
        ...state,
        forumHandle: action.forumHandle,
      };
  }

  return state;
}
