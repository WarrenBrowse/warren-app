import { DaemonAccessRefusal } from '../../../shared/daemon-access-refusal';
import { RoutePath } from '../../../shared/routes';
import { LoginState } from '../../redux/account/reducers';

// A refused account reaches the daemon, so `connectedToDaemon` can be true, yet
// nothing past the launch view works for it: the login view would offer to
// create or restore a wallet the daemon refuses to install.
export function getNavigationBase(
  connectedToDaemon: boolean,
  loginState: LoginState,
  onboardingPending?: boolean,
  daemonAccessRefusal: DaemonAccessRefusal | null = null,
): RoutePath {
  if (connectedToDaemon && daemonAccessRefusal === null) {
    if (loginState.type === 'none' && loginState.deviceRevoked) {
      return RoutePath.deviceRevoked;
    } else if (
      loginState.type === 'none' ||
      loginState.type === 'logging in' ||
      loginState.type === 'backup-pending' ||
      loginState.type === 'failed'
    ) {
      return RoutePath.login;
    } else if (loginState.type === 'ok' && loginState.expiredState === 'expired') {
      return RoutePath.expired;
    } else if (loginState.type === 'ok' && loginState.expiredState === 'time_added') {
      return RoutePath.timeAdded;
    } else if (loginState.type === 'ok' && onboardingPending === true) {
      // A wizard run is owed to this user: the account was just created
      // here, or the wizard was replayed from Settings. Finishing or
      // skipping it clears the flag in `IGuiSettingsState` and the boot
      // falls through to `RoutePath.main`. An account restored from a
      // recovery phrase never sets the flag, so it is never sent here.
      return RoutePath.onboardingWelcome;
    } else {
      return RoutePath.main;
    }
  } else {
    return RoutePath.launch;
  }
}
