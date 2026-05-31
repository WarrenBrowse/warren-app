import { useCallback, useEffect, useRef, useState } from 'react';

import { hasExpired } from '../../../../shared/account-expiry';
import { urls } from '../../../../shared/constants';
import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Spinner, Text } from '../../../lib/components';
import { useHistory } from '../../../lib/history';
import { useSelector } from '../../../redux/store';
import { OnboardingLayout } from './components';

// M5.B.3 step 3: subscription pointer. Warren is paid (~7-10
// EUR/mo). We do **not** embed an iframe to warrenbrowse.com/pricing;
// the link opens in the user's default browser so the SPA UI is not
// coupled to the marketing page lifecycle (the page changes on every
// pricing tier review).
//
// G-4: "I already have a subscription" now verifies enrollment via
// updateAccountData() before advancing. If the daemon reports no
// active subscription, an inline error is shown.
export function OnboardingSubscriptionView() {
  const { push } = useHistory();
  const { updateAccountData, openUrl } = useAppContext();
  const accountExpiry = useSelector((state) => state.account.expiry);

  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | undefined>(undefined);

  // Auto-poll state: starts when the user opens the external pricing page.
  const [polling, setPolling] = useState(false);
  const pollTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const pollDeadlineRef = useRef<number>(0);

  // Cleanup auto-poll on unmount.
  useEffect(() => {
    return () => {
      if (pollTimerRef.current) {
        clearInterval(pollTimerRef.current);
      }
    };
  }, []);

  // Watch for expiry changes while polling: if subscription becomes
  // active, navigate forward automatically.
  useEffect(() => {
    if (polling && accountExpiry && !hasExpired(accountExpiry)) {
      if (pollTimerRef.current) {
        clearInterval(pollTimerRef.current);
        pollTimerRef.current = null;
      }
      setPolling(false);
      setError(undefined);
      push(RoutePath.onboardingPreferences);
    }
  }, [polling, accountExpiry, push]);

  const verifySubscription = useCallback(async () => {
    setChecking(true);
    setError(undefined);
    try {
      await updateAccountData();
    } catch (e) {
      const err = e as Error;
      log.error(`Failed to verify subscription: ${err.message}`);
      setError(
        messages.pgettext(
          'warren-onboarding',
          'Could not check subscription status. Please try again.',
        ),
      );
      setChecking(false);
      return;
    }
    setChecking(false);
  }, [updateAccountData]);

  // After a successful verification round-trip, check the (now-updated)
  // expiry from the store.
  const prevCheckingRef = useRef(false);
  useEffect(() => {
    if (prevCheckingRef.current && !checking && !error) {
      if (accountExpiry && !hasExpired(accountExpiry)) {
        push(RoutePath.onboardingPreferences);
      } else {
        setError(
          messages.pgettext(
            'warren-onboarding',
            'No active subscription found. Please purchase one first.',
          ),
        );
      }
    }
    prevCheckingRef.current = checking;
  }, [checking, error, accountExpiry, push]);

  const handleAlreadyHave = useCallback(() => {
    void verifySubscription();
  }, [verifySubscription]);

  const handleOpenPricing = useCallback(() => {
    void openUrl(urls.pricing);

    // Start auto-polling every 10s for 2 minutes after the user opens
    // the external payment page.
    if (pollTimerRef.current) {
      clearInterval(pollTimerRef.current);
    }
    pollDeadlineRef.current = Date.now() + 2 * 60 * 1000;
    setPolling(true);
    pollTimerRef.current = setInterval(() => {
      if (Date.now() > pollDeadlineRef.current) {
        if (pollTimerRef.current) {
          clearInterval(pollTimerRef.current);
          pollTimerRef.current = null;
        }
        setPolling(false);
        return;
      }
      void updateAccountData().catch((e: unknown) => {
        const err = e as Error;
        log.error(`Auto-poll subscription check failed: ${err.message}`);
      });
    }, 10_000);
  }, [openUrl, updateAccountData]);

  const handleCheckAgain = useCallback(() => {
    void verifySubscription();
  }, [verifySubscription]);

  return (
    <OnboardingLayout
      title={messages.pgettext('warren-onboarding', 'Your subscription')}
      description={messages.pgettext(
        'warren-onboarding',
        "You don't have an active subscription yet. Plans start at a few euros per month - no recurring billing, no account creation, pay as you go.",
      )}
      actions={
        <>
          <Button
            variant="success"
            onClick={handleOpenPricing}
            data-testid="onboarding-subscription-link">
            <Button.Text>
              {messages.pgettext('warren-onboarding', 'View plans (opens in your browser)')}
            </Button.Text>
            <Button.Icon icon="external" />
          </Button>

          <Button
            variant="primary"
            disabled={checking}
            onClick={handleAlreadyHave}
            data-testid="onboarding-subscription-already-have">
            {checking ? (
              <Spinner />
            ) : (
              <Button.Text>
                {messages.pgettext('warren-onboarding', 'I already have a subscription')}
              </Button.Text>
            )}
          </Button>

          {(error || polling) && (
            <Button
              variant="primary"
              disabled={checking}
              onClick={handleCheckAgain}
              data-testid="onboarding-subscription-check-again">
              {checking ? (
                <Spinner />
              ) : (
                <Button.Text>
                  {polling
                    ? messages.pgettext('warren-onboarding', 'Checking... (click to refresh now)')
                    : messages.pgettext('warren-onboarding', 'Check again')}
                </Button.Text>
              )}
            </Button>
          )}
        </>
      }>
      {error && (
        <Text variant="bodySmall" color="red">
          {error}
        </Text>
      )}
    </OnboardingLayout>
  );
}
