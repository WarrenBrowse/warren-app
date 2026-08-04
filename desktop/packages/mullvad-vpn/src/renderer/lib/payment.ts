export enum PaymentRecoveryAction {
  openBrowser,
  disconnect,
  disableLockdownMode,
}

// The external checkout opens in the system browser, which cannot
// reach the network while the app firewall blocks (connecting loop,
// blocking error state, lockdown). Deciding the unblocking step here
// keeps every paywall surface (account view, expired view, onboarding)
// consistent.
export function paymentRecoveryAction(
  lockdownMode: boolean,
  isBlocked: boolean,
): PaymentRecoveryAction {
  if (lockdownMode && isBlocked) {
    return PaymentRecoveryAction.disableLockdownMode;
  } else if (isBlocked) {
    return PaymentRecoveryAction.disconnect;
  } else {
    return PaymentRecoveryAction.openBrowser;
  }
}
