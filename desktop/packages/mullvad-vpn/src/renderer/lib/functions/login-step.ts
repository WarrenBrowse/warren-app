import { LoginState } from '../../redux/account/reducers';

export type LoginViewStep = 'loggedIn' | 'backup' | 'restore' | 'pick';

// Single authority for what the login view renders. The view stays
// mounted for a moment after login settles (navigation transition), so
// deriving the step ad hoc from the account status let it fall back to
// the pick/welcome step in that window, flashing the wrong screen
// between the backup step and the expired/congrats screen. 'loggedIn'
// renders a neutral transition state instead.
export function loginViewStep(
  statusType: LoginState['type'],
  mode: 'pick' | 'restore',
): LoginViewStep {
  if (statusType === 'ok') {
    return 'loggedIn';
  }
  if (statusType === 'backup-pending') {
    return 'backup';
  }
  return mode === 'restore' ? 'restore' : 'pick';
}

export type LoginBackAction = 'none' | 'pick' | 'discard';

// What leaving a login step means. Every step the user reached by making a
// choice offers a way back to that choice, so picking "Create a new account"
// by mistake is recoverable. The backup step is the one that costs something:
// the daemon has already minted and logged into the identity, so going back
// there discards it rather than merely swapping the screen.
export function loginStepBack(step: LoginViewStep): LoginBackAction {
  switch (step) {
    case 'restore':
      return 'pick';
    case 'backup':
      return 'discard';
    case 'pick':
    case 'loggedIn':
      return 'none';
  }
}
