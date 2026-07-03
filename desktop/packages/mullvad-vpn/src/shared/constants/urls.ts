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
  // Community + support forum. Login is wallet-based (DiscourseConnect wallet
  // SSO, doc 55); the `warren://forum-login` deep link is handled in the main
  // process. See forum-login.ts.
  forum: 'https://forum.warrenbrowse.com/',
} as const;

type BaseUrl = (typeof urls)[keyof typeof urls];
type ExtendedBaseUrl = `${BaseUrl}${string}`;
export type Url = BaseUrl | ExtendedBaseUrl;
