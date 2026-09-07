import {
  dateByAddingComponent,
  DateComponent,
  DateType,
  FormatDateOptions,
  formatRelativeDate,
} from './date-helper';

export function hasExpired(expiry: DateType): boolean {
  return new Date(expiry).getTime() < Date.now();
}

// The expiry the account-data cache synthesises when the API answers 404
// for the wallet: no subscription row exists yet. Distinct from a real
// past expiry, which means an access that lapsed.
export const NEVER_ACTIVATED_EXPIRY = new Date(0).toISOString();

export function isNeverActivatedExpiry(expiry: DateType): boolean {
  return new Date(expiry).getTime() === 0;
}

export function closeToExpiry(expiry: DateType, days = 3): boolean {
  return (
    !hasExpired(expiry) &&
    new Date(expiry) <= dateByAddingComponent(new Date(), DateComponent.day, days)
  );
}

export function formatDate(date: DateType, locale: string): string {
  if (window.env.development && locale === 'sv-rö') {
    locale = 'sv';
  }

  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(
    new Date(date),
  );
}

export function formatRemainingTime(expiry: DateType, options?: FormatDateOptions): string {
  return formatRelativeDate(new Date(), expiry, options);
}
