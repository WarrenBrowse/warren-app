import { formatRemainingTime } from '../../../../shared/account-expiry';

// The footer's end: "14 days left" on its own, the bare "14 days" after the
// beta cap, where the mockup keeps the line short.
export function footerTimeLeft(expiry: string | undefined, afterBeta: boolean): string {
  if (!expiry) {
    return '';
  }
  return formatRemainingTime(expiry, { suffix: !afterBeta });
}
