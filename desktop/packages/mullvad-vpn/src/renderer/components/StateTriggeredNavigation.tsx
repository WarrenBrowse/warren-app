import { useEffect, useMemo, useRef } from 'react';

import { RoutePath } from '../../shared/routes';
import { useScheduler } from '../../shared/scheduler';
import { getNavigationBase } from '../lib/functions/navigation-base';
import { TransitionType, useHistory } from '../lib/history';
import { useEffectEvent } from '../lib/utility-hooks';
import { useSelector } from '../redux/store';

export default function StateTriggeredNavigation() {
  const { location, reset } = useHistory();

  const connectedToDaemon = useSelector((state) => state.userInterface.connectedToDaemon);
  const loginState = useSelector((state) => state.account.status);
  const onboardingCompletedUnix = useSelector(
    (state) => state.settings.guiSettings.onboardingCompletedUnix,
  );

  const delayScheduler = useScheduler();

  const prevPath = useRef<RoutePath>(
    getNavigationBase(connectedToDaemon, loginState, onboardingCompletedUnix),
  );
  const nextPath = useMemo(
    () => getNavigationBase(connectedToDaemon, loginState, onboardingCompletedUnix),
    [connectedToDaemon, loginState, onboardingCompletedUnix],
  );

  const updatePath = useEffectEvent((nextPath: RoutePath) => {
    const currentPath = location.pathname as RoutePath;

    // `voucherSuccess` is a parameterized child route of `timeAdded`
    // (`/main/voucher/success/:newExpiry/:secondsAdded` vs
    // `/main/time-added`) — both surface the "subscription credit was
    // applied" UX, with `voucherSuccess` carrying a more specific
    // title ("Voucher was successfully redeemed"). When a user
    // redeems a voucher, `RedeemVoucher.tsx` imperatively pushes
    // `voucherSuccess` AND the account-expiry refresh that follows
    // transitions `expiredState` from `'expired'` to `'time_added'`,
    // which makes `getNavigationBase` reactively return `timeAdded`.
    // Without this guard the user is bounced from `voucherSuccess`
    // to the generic `timeAdded` view a moment after seeing the
    // voucher-specific one — observed as "the success screen appears
    // twice in a row" (M5.C.x bug report 2026-05-28).
    if (nextPath === RoutePath.timeAdded && currentPath.startsWith('/main/voucher/success/')) {
      return;
    }

    if (currentPath !== nextPath) {
      delayScheduler.cancel();

      const transition = getNavigationTransition(currentPath, nextPath);
      const delay = getNavigationDelay(currentPath, nextPath);

      const navigate = () => {
        reset(nextPath, { transition });
      };

      if (delay) {
        delayScheduler.schedule(navigate, delay);
      } else {
        navigate();
      }
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

function getNavigationDelay(currentPath: RoutePath, nextPath: RoutePath): number | void {
  if (
    currentPath === RoutePath.login &&
    (nextPath === RoutePath.main || nextPath === RoutePath.expired)
  ) {
    return 1000;
  }
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
