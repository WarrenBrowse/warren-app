import { useCallback, useEffect, useRef, useState } from 'react';

import { hasExpired } from '../../../../shared/account-expiry';
import { messages } from '../../../../shared/gettext';
import log from '../../../../shared/logging';
import { RoutePath } from '../../../../shared/routes';
import { useAppContext } from '../../../context';
import { Button, Spinner, Text } from '../../../lib/components';
import { useHistory } from '../../../lib/history';
import { useSelector } from '../../../redux/store';
import { ExternalPaymentButton } from '../../payment';
import { RedeemVoucherAlert, RedeemVoucherContainer } from '../../RedeemVoucher';
import { OnboardingLayout } from './components';

// Subscription step of the onboarding wizard. Three ways forward, all
// converging on the account expiry turning valid: the external
// checkout (auto-credited by the main-process purchase poll, doc 35),
// a voucher code, or "I already have a subscription" for restored
// wallets (manual re-check).
export function OnboardingSubscriptionView() {
  const { push } = useHistory();
  const { checkPendingPurchases, updateAccountData } = useAppContext();
  const accountExpiry = useSelector((state) => state.account.expiry);
  const purchaseInFlight = useSelector((state) => state.account.purchaseInFlight);

  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | undefined>(undefined);
  const [voucherOpen, setVoucherOpen] = useState(false);

  // Advance the moment the account is funded, whatever credited it
  // (purchase poll, voucher, another device). Held while the voucher
  // success modal is open so the user reads the confirmation first.
  // The ref makes this instance navigate at most once: the outgoing
  // view stays mounted during the 450ms view transition (and mounts
  // twice under StrictMode), so an unguarded effect can double-push.
  const navigatedRef = useRef(false);
  useEffect(() => {
    if (!navigatedRef.current && !voucherOpen && accountExpiry && !hasExpired(accountExpiry)) {
      navigatedRef.current = true;
      push(RoutePath.onboardingPreferences);
    }
  }, [voucherOpen, accountExpiry, push]);

  const verifySubscription = useCallback(async () => {
    setChecking(true);
    setError(undefined);
    try {
      // A pending app-initiated purchase may be waiting server-side:
      // redeem it first (that is what credits the account), then
      // refresh the expiry read below.
      await checkPendingPurchases();
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
  }, [checkPendingPurchases, updateAccountData]);

  // After a verification round-trip that still shows no time, surface
  // an inline error. The success case is NOT handled here: the
  // expiry-watch effect above owns the forward navigation (a second
  // push from this effect would duplicate the history entry).
  const prevCheckingRef = useRef(false);
  useEffect(() => {
    if (prevCheckingRef.current && !checking && !error) {
      if (!accountExpiry || hasExpired(accountExpiry)) {
        setError(
          messages.pgettext(
            'warren-onboarding',
            'No active subscription found. Please purchase one first.',
          ),
        );
      }
    }
    prevCheckingRef.current = checking;
  }, [checking, error, accountExpiry]);

  const handleAlreadyHave = useCallback(() => {
    void verifySubscription();
  }, [verifySubscription]);

  const handleCheckAgain = useCallback(() => {
    void verifySubscription();
  }, [verifySubscription]);

  const openVoucher = useCallback(() => setVoucherOpen(true), []);
  const closeVoucher = useCallback(() => setVoucherOpen(false), []);

  return (
    <OnboardingLayout
      title={messages.pgettext('warren-onboarding', 'Your subscription')}
      description={messages.pgettext(
        'warren-onboarding',
        "You don't have an active subscription yet. Plans start at a few euros per month - no recurring billing, no account creation, pay as you go.",
      )}
      actions={
        <>
          <ExternalPaymentButton
            buttonText={messages.pgettext(
              'warren-onboarding',
              'View plans (opens in your browser)',
            )}
            data-testid="onboarding-subscription-link"
          />

          <Button
            variant="success"
            onClick={openVoucher}
            data-testid="onboarding-subscription-redeem-voucher">
            <Button.Text>
              {
                // TRANSLATORS: Button label for redeeming a voucher.
                messages.pgettext('redeem-voucher-alert', 'Redeem voucher')
              }
            </Button.Text>
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

          {(error || purchaseInFlight) && (
            <Button
              variant="primary"
              disabled={checking}
              onClick={handleCheckAgain}
              data-testid="onboarding-subscription-check-again">
              {checking ? (
                <Spinner />
              ) : (
                <Button.Text>
                  {purchaseInFlight
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
      <RedeemVoucherContainer>
        <RedeemVoucherAlert show={voucherOpen} onClose={closeVoucher} />
      </RedeemVoucherContainer>
    </OnboardingLayout>
  );
}
