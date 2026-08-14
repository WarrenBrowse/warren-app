import { useEffect, useMemo, useRef } from 'react';

import { RoutePath } from '../../shared/routes';
import { getNavigationBase } from '../lib/functions/navigation-base';
import { TransitionType, useHistory } from '../lib/history';
import { useEffectEvent } from '../lib/utility-hooks';
import { useSelector } from '../redux/store';

export default function StateTriggeredNavigation() {
  const { location, reset } = useHistory();

  const connectedToDaemon = useSelector((state) => state.userInterface.connectedToDaemon);
  const loginState = useSelector((state) => state.account.status);
  const onboardingPending = useSelector((state) => state.settings.guiSettings.onboardingPending);

  const prevPath = useRef<RoutePath>(
    getNavigationBase(connectedToDaemon, loginState, onboardingPending),
  );
  const nextPath = useMemo(
    () => getNavigationBase(connectedToDaemon, loginState, onboardingPending),
    [connectedToDaemon, loginState, onboardingPending],
  );

  const updatePath = useEffectEvent((nextPath: RoutePath) => {
    const currentPath = location.pathname as RoutePath;

    // `voucherSuccess` is a parameterized child route of `timeAdded`
    // (`/main/voucher/success/:newExpiry/:secondsAdded` vs
    // `/main/time-added`) - both surface the "subscription credit was
    // applied" UX, with `voucherSuccess` carrying a more specific
    // title ("Voucher was successfully redeemed"). When a user
    // redeems a voucher, `RedeemVoucher.tsx` imperatively pushes
    // `voucherSuccess` AND the account-expiry refresh that follows
    // transitions `expiredState` from `'expired'` to `'time_added'`,
    // which makes `getNavigationBase` reactively return `timeAdded`.
    // Without this guard the user is bounced from `voucherSuccess`
    // to the generic `timeAdded` view a moment after seeing the
    // voucher-specific one - observed as "the success screen appears
    // twice in a row".
    if (nextPath === RoutePath.timeAdded && currentPath.startsWith('/main/voucher/success/')) {
      return;
    }

    // While the user is inside the onboarding wizard, account state churns by
    // design: minting and crediting the first subscription flips `expiredState`
    // from `expired` to `time_added` to `undefined`, and each change makes
    // `getNavigationBase` return `expired`, then `timeAdded`, then
    // `onboardingWelcome`. Letting those drive navigation yanks the user out of
    // whatever step they are on and dumps them back on step 1 (welcome), so they
    // can never reach the final step that clears `onboardingPending` -
    // an inescapable loop (observed on Windows). The wizard owns its forward
    // navigation (each view pushes the next and the final step routes to `main`),
    // so ignore these state-triggered redirects while the user is within it.
    // Genuine interrupts (logout -> login, daemon disconnect -> launch, device
    // revoked) are not in this set and still go through.
    const inOnboarding =
      currentPath === RoutePath.onboardingWelcome ||
      currentPath.startsWith(`${RoutePath.onboardingWelcome}/`);
    if (
      inOnboarding &&
      (nextPath === RoutePath.expired ||
        nextPath === RoutePath.timeAdded ||
        nextPath === RoutePath.onboardingWelcome)
    ) {
      return;
    }

    if (currentPath !== nextPath) {
      // Navigate on the same render cycle as the state change. The
      // 1000ms login-to-main/expired delay inherited from Mullvad
      // existed to show its "logged in" checkmark; Warren has no such
      // interstitial, so any delay here is a window where the source
      // view renders content for a state the user already left
      // (observed as the welcome screen flashing between the backup
      // step and the expired/congrats screen).
      reset(nextPath, { transition: getNavigationTransition(currentPath, nextPath) });
    }
  });

  useEffect(() => {
    if (nextPath !== prevPath.current) {
      prevPath.current = nextPath;
      updatePath(nextPath);
    }
    // eslint-disable-next-line react-compiler/react-compiler
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nextPath]);

  return null;
}

function getNavigationTransition(currentPath: RoutePath, nextPath: RoutePath) {
  // First level contains the possible next locations and the second level contains the
  // possible current locations.
  const navigationTransitions: Partial<
    Record<RoutePath, Partial<Record<RoutePath | '*', TransitionType>>>
  > = {
    [RoutePath.launch]: {
      [RoutePath.login]: TransitionType.pop,
      [RoutePath.main]: TransitionType.pop,
      '*': TransitionType.dismiss,
    },
    [RoutePath.login]: {
      [RoutePath.launch]: TransitionType.push,
      [RoutePath.main]: TransitionType.pop,
      [RoutePath.deviceRevoked]: TransitionType.pop,
      '*': TransitionType.dismiss,
    },
    [RoutePath.main]: {
      [RoutePath.launch]: TransitionType.push,
      [RoutePath.login]: TransitionType.push,
      '*': TransitionType.dismiss,
    },
    [RoutePath.expired]: {
      [RoutePath.launch]: TransitionType.push,
      [RoutePath.login]: TransitionType.push,
      '*': TransitionType.dismiss,
    },
    [RoutePath.timeAdded]: {
      [RoutePath.expired]: TransitionType.push,
      [RoutePath.redeemVoucher]: TransitionType.push,
      '*': TransitionType.dismiss,
    },
    [RoutePath.deviceRevoked]: {
      '*': TransitionType.pop,
    },
  };

  return navigationTransitions[nextPath]?.[currentPath] ?? navigationTransitions[nextPath]?.['*'];
}
