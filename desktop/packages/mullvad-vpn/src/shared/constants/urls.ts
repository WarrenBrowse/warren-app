// Warren VPN website links. TODO: confirm final URLs with the team
// once warrenbrowse.com content pages are published.
//
// `purchase` and `pricing` point at the Stripe checkout funnel
// (warren-checkout, served at checkout.warrenbrowse.com): the user buys a
// plan there and receives a voucher to redeem in the app.
export const urls = {
  purchase: 'https://checkout.warrenbrowse.com/',
  pricing: 'https://checkout.warrenbrowse.com/',
  faq: 'https://warrenbrowse.com/help/',
  privacyGuide: 'https://warrenbrowse.com/privacy-guide/',
  download: 'https://warrenbrowse.com/download/',
} as const;

type BaseUrl = (typeof urls)[keyof typeof urls];
type ExtendedBaseUrl = `${BaseUrl}${string}`;
export type Url = BaseUrl | ExtendedBaseUrl;
