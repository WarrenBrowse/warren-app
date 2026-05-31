import { RoutePath } from '../../../shared/routes';
import { LoginState } from '../../redux/account/reducers';

export function getNavigationBase(
  connectedToDaemon: boolean,
  loginState: LoginState,
  onboardingCompletedUnix?: number,
): RoutePath {
  if (connectedToDaemon) {
    if (loginState.type === 'none' && loginState.deviceRevoked) {
      return RoutePath.deviceRevoked;
    } else if (
      loginState.type === 'none' ||
      loginState.type === 'logging in' ||
      loginState.type === 'failed'
    ) {
      return RoutePath.login;
    } else if (loginState.type === 'ok' && loginState.expiredState === 'expired') {
      return RoutePath.expired;
    } else if (loginState.type === 'ok' && loginState.expiredState === 'time_added') {
      return RoutePath.timeAdded;
    } else if (loginState.type === 'ok' && onboardingCompletedUnix === undefined) {
      // M5.B.3: first launch on a freshly-logged-in account routes
      // to the onboarding wizard. Once the user finishes (or skips)
      // the wizard, `onboardingCompletedUnix` is persisted in
      // `IGuiSettingsState`, and subsequent boots fall through to
      // `RoutePath.main`. The wizard can also be replayed manually
      // from Settings, which clears the field.
      return RoutePath.onboardingWelcome;
    } else {
      return RoutePath.main;
    }
  } else {
    return RoutePath.launch;
  }
}
