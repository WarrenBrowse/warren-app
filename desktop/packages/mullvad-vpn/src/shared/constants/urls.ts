// Warren VPN website links. TODO: confirm final URLs with the team
// once warrenvpn.com content pages are published.
export const urls = {
  purchase: 'https://warrenvpn.com/account/',
  pricing: 'https://warrenvpn.com/pricing',
  faq: 'https://warrenvpn.com/help/',
  privacyGuide: 'https://warrenvpn.com/privacy-guide/',
  download: 'https://warrenvpn.com/download/',
} as const;

type BaseUrl = (typeof urls)[keyof typeof urls];
type ExtendedBaseUrl = `${BaseUrl}${string}`;
export type Url = BaseUrl | ExtendedBaseUrl;
