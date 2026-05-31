// Warren VPN website links. TODO: confirm final URLs with the team
// once warrenbrowse.com content pages are published.
export const urls = {
  purchase: 'https://warrenbrowse.com/account/',
  pricing: 'https://warrenbrowse.com/pricing',
  faq: 'https://warrenbrowse.com/help/',
  privacyGuide: 'https://warrenbrowse.com/privacy-guide/',
  download: 'https://warrenbrowse.com/download/',
} as const;

type BaseUrl = (typeof urls)[keyof typeof urls];
type ExtendedBaseUrl = `${BaseUrl}${string}`;
export type Url = BaseUrl | ExtendedBaseUrl;
