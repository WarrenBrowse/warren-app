import { useCallback, useEffect, useRef, useState } from 'react';
import { sprintf } from 'sprintf-js';

import { formatDate, hasExpired } from '../../../../shared/account-expiry';
import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Spinner, Text } from '../../../lib/components';
import { useHistory } from '../../../lib/history';
import { useSelector } from '../../../redux/store';
import { OnboardingForumHint, OnboardingLayout } from './components';

// Auto-retry cadence for the beta access activation. Capped: a user
// offline at first launch retries quietly in the background and can
// always proceed and come back later.
const RETRY_DELAYS_MS = [5_000, 15_000, 30_000, 60_000];

const BPS_PER_MBPS = 1_000_000;

// Beta replacement for the subscription onboarding step: instead of a
// checkout, the app redeems the beta campaign voucher server-side
// (empty voucher = auto-redeem, idempotent) and explains the degraded
// free network. Never blocks: activation failures retry with backoff
// and the user can continue regardless.
export function OnboardingBetaAccessView() {
  const { push } = useHistory();
  const { submitVoucher, updateAccountData } = useAppContext();
  const accountExpiry = useSelector((state) => state.account.expiry);
  const networkInfo = useSelector((state) => state.settings.warrenStatus?.networkInfo);
  const locale = useSelector((state) => state.userInterface.locale);

  const hasAccess = accountExpiry !== undefined && !hasExpired(accountExpiry);
  const [activating, setActivating] = useState(!hasAccess);
  const [failed, setFailed] = useState(false);

  const attemptRef = useRef(0);
  const timerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const unmountedRef = useRef(false);
  useEffect(() => {
    return () => {
      unmountedRef.current = true;
      clearTimeout(timerRef.current);
    };
  }, []);

  const activate = useCallback(async () => {
    setActivating(true);
    setFailed(false);
    try {
      // Empty voucher = redeem the server's beta auto-voucher for this
      // wallet, idempotently (an already-active wallet gets its current
      // expiry back).
      await submitVoucher('');
      await updateAccountData();
      if (!unmountedRef.current) {
        setActivating(false);
      }
    } catch (e) {
      const error = e as Error;
      log.error(`Beta access activation failed: ${error.message}`);
      if (unmountedRef.current) {
        return;
      }
      setActivating(false);
      setFailed(true);
      const delay = RETRY_DELAYS_MS[Math.min(attemptRef.current, RETRY_DELAYS_MS.length - 1)];
      attemptRef.current += 1;
      timerRef.current = setTimeout(() => void activate(), delay);
    }
  }, [submitVoucher, updateAccountData]);

  // One activation kick per mount (the ref makes re-renders and the
  // StrictMode double-mount inert; the retry chain is timer-driven).
  const startedRef = useRef(false);
  useEffect(() => {
    if (!startedRef.current && !hasAccess) {
      startedRef.current = true;
      void activate();
    }
  }, [activate, hasAccess]);

  const handleContinue = useCallback(() => {
    push(RoutePath.onboardingPreferences);
  }, [push]);

  const handleRetryNow = useCallback(() => {
    clearTimeout(timerRef.current);
    void activate();
  }, [activate]);

  const capMbps = networkInfo?.defaultRateBps
    ? Math.round(networkInfo.defaultRateBps / BPS_PER_MBPS)
    : undefined;
  const capLine =
    capMbps !== undefined
      ? sprintf(
          // TRANSLATORS: Beta onboarding line describing the degraded service.
          // TRANSLATORS: Available placeholders:
          // TRANSLATORS: %(mbps)d - the bandwidth cap in Mbps
          messages.pgettext(
            'warren-onboarding',
            'The beta runs on a free, separate network with bandwidth capped at %(mbps)d Mbps. These are not the final service conditions.',
          ),
          { mbps: capMbps },
        )
      : // TRANSLATORS: Beta onboarding line describing the degraded service when the cap figure is unknown.
        messages.pgettext(
          'warren-onboarding',
          'The beta runs on a free, separate network with limited bandwidth. These are not the final service conditions.',
        );

  return (
    <OnboardingLayout
      title={
        // TRANSLATORS: Title of the beta onboarding access step.
        messages.pgettext('warren-onboarding', 'Your beta access')
      }
      description={
        // TRANSLATORS: Description of the beta onboarding access step.
        messages.pgettext(
          'warren-onboarding',
          'A prepaid beta access voucher is linked to your account automatically. No payment, nothing to do.',
        )
      }
      actions={
        <Button
          variant="success"
          disabled={activating}
          onClick={handleContinue}
          data-testid="onboarding-beta-continue">
          {activating ? <Spinner /> : <Button.Text>{messages.gettext('Continue')}</Button.Text>}
        </Button>
      }>
      <Text variant="bodySmall" color="whiteAlpha60">
        {capLine}
      </Text>
      {hasAccess && accountExpiry && (
        <Text variant="bodySmall" data-testid="onboarding-beta-expiry">
          {sprintf(
            // TRANSLATORS: Confirmation that the beta access is active.
            // TRANSLATORS: Available placeholders:
            // TRANSLATORS: %(date)s - the expiry date of the granted access
            messages.pgettext('warren-onboarding', 'Beta access active until %(date)s.'),
            { date: formatDate(accountExpiry, locale) },
          )}
        </Text>
      )}
      {failed && (
        <>
          <Text variant="bodySmall" color="red">
            {
              // TRANSLATORS: Error shown when the beta access could not be activated yet.
              messages.pgettext(
                'warren-onboarding',
                'Could not activate the beta access yet (are you online?). Retrying automatically, and again at next launch. You can continue anyway.',
              )
            }
          </Text>
          <Button variant="primary" onClick={handleRetryNow} data-testid="onboarding-beta-retry">
            <Button.Text>{messages.pgettext('warren-onboarding', 'Retry now')}</Button.Text>
          </Button>
          <OnboardingForumHint />
        </>
      )}
    </OnboardingLayout>
  );
}
