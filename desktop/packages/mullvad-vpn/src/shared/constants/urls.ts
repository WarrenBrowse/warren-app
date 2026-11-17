import { productAnchors } from './product-env';

// Warren VPN website links, on the website of the compiled channel. The
// website auto-detects the browser language, so no language path segment is
// needed.
const website = productAnchors.websiteUrl;
const checkout = productAnchors.checkoutUrl;

// `purchase` and `pricing` point at the Stripe checkout funnel of the
// compiled channel (warren-checkout): the user buys a plan there and receives
// a voucher to redeem in the app.
export const urls = {
  purchase: `${checkout}/`,
  pricing: `${checkout}/`,
  // Public account API of the compiled product environment, used by the MAIN
  // process only for the renewal-handoff pull (warren-core doc 65); every
  // other API access goes through the daemon and its bootstrap-privacy
  // machinery.
  api: productAnchors.apiBaseUrl,
  faq: `${website}/faq`,
  privacyGuide: `${website}/no-log`,
  // The compiled channel's download page, like `api` above.
  download: productAnchors.downloadUrl,
  // Community + support forum. Login is wallet-based (DiscourseConnect wallet
  // SSO, doc 55); the `warren://forum-login` deep link is handled in the main
  // process. See forum-login.ts.
  forum: 'https://forum.warrenbrowse.com/',
  // Support triage for users without forum access (never-paid wallets).
  help: `${website}/aide`,
  // Prepaid refund policy (art. 11a withdrawal lives on the website).
  refundPolicy: `${website}/remboursement`,
  // Terms in force. The website serves the free-beta terms here today and the
  // paid terms once a legal entity exists, so this one URL always resolves to
  // the contract the user is actually under.
  terms: `${website}/cgu`,
  // Privacy policy, linked from the Support view as on Android and iOS.
  privacyPolicy: `${website}/confidentialite`,
  // Cancellation page of the checkout: the way out every renewal notice names.
  cancelContract: `${checkout}/resiliation`,
  // Abuse-report and appeal policy. Addressed to rights holders, hosts and
  // authorities, and the page a user lands on to contest a port-forwarding
  // strike, so it is linked from the port-forwarding view and the ban notice.
  reports: `${website}/signalements`,
  // How to point a torrent client at a forwarded port: the copy button, the
  // notification and the status file the desktop app writes. Linked from the
  // port-forwarding view.
  portForwardingGuide: `${website}/port-forwarding`,
  // Public source of the app (GPL-3.0) and of its tunnel engine (AGPL-3.0),
  // linked from the licences screen at the revisions this build was made from
  // (`shared/source-links.ts`).
  sourceCode: 'https://github.com/WarrenBrowse/warren-app/',
  engineSourceCode: 'https://github.com/WarrenBrowse/warrenguard/',
} as const;

type BaseUrl = (typeof urls)[keyof typeof urls];
type ExtendedBaseUrl = `${BaseUrl}${string}`;
export type Url = BaseUrl | ExtendedBaseUrl;
