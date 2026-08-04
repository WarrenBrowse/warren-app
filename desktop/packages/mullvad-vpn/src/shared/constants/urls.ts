import { productAnchors } from './product-env';

// Warren VPN website links. The website auto-detects the browser language,
// so no language path segment is needed.
//
// `purchase` and `pricing` point at the Stripe checkout funnel
// (warren-checkout, served at checkout.warrenbrowse.com): the user buys a
// plan there and receives a voucher to redeem in the app.
export const urls = {
  purchase: 'https://checkout.warrenbrowse.com/',
  pricing: 'https://checkout.warrenbrowse.com/',
  // Public account API of the compiled product environment, used by the MAIN
  // process only for the renewal-handoff pull (warren-core doc 65); every
  // other API access goes through the daemon and its bootstrap-privacy
  // machinery.
  api: productAnchors.apiBaseUrl,
  faq: 'https://warren.ro/faq',
  privacyGuide: 'https://warren.ro/no-log',
  download: 'https://warren.ro/telecharger',
  // Community + support forum. Login is wallet-based (DiscourseConnect wallet
  // SSO, doc 55); the `warren://forum-login` deep link is handled in the main
  // process. See forum-login.ts.
  forum: 'https://forum.warrenbrowse.com/',
  // Support triage for users without forum access (never-paid wallets).
  help: 'https://warren.ro/aide',
  // Prepaid refund policy (art. 11a withdrawal lives on the website).
  refundPolicy: 'https://warren.ro/remboursement',
  // Terms in force. The website serves the free-beta terms here today and the
  // paid terms once a legal entity exists, so this one URL always resolves to
  // the contract the user is actually under.
  terms: 'https://warren.ro/cgu',
  // Abuse-report and appeal policy. Addressed to rights holders, hosts and
  // authorities, and the page a user lands on to contest a port-forwarding
  // strike, so it is linked from the port-forwarding view and the ban notice.
  reports: 'https://warren.ro/signalements',
} as const;

type BaseUrl = (typeof urls)[keyof typeof urls];
type ExtendedBaseUrl = `${BaseUrl}${string}`;
export type Url = BaseUrl | ExtendedBaseUrl;
