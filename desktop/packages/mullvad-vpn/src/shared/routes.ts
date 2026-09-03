export enum RoutePath {
  launch = '/',
  login = '/login',
  deviceRevoked = '/login/device-revoked',
  main = '/main',
  redeemVoucher = '/main/voucher/redeem',
  voucherSuccess = '/main/voucher/success/:newExpiry/:secondsAdded',
  expired = '/main/expired',
  timeAdded = '/main/time-added',
  setupFinished = '/main/setup-finished',
  settings = '/settings',
  selectLanguage = '/settings/language',
  account = '/account',
  // Community-forum activity panel, opened from the header bell. A view
  // rather than a popover: this app opens Account and Settings the same
  // way, and a dropdown over the connect screen exists nowhere else.
  forumActivity = '/forum-activity',
  keys = '/account/keys',
  restoreKeys = '/account/keys/restore',
  userInterfaceSettings = '/settings/interface',
  multihopSettings = '/settings/multihop',
  warrenMultiHopSettings = '/settings/warren-multi-hop',
  // Warren NAT-PMP port-forwarding (differentiator vs Mullvad
  // / IVPN abandon 2023). View opens from the settings home view.
  portForwardingSettings = '/settings/port-forwarding',
  vpnSettings = '/settings/vpn',
  daitaSettings = '/settings/daita',
  udpOverTcp = '/settings/advanced/wireguard/udp-over-tcp',
  shadowsocks = '/settings/advanced/shadowsocks',
  splitTunneling = '/settings/split-tunneling',
  apiAccessMethods = '/settings/api-access-methods',
  settingsImport = '/settings/settings-import',
  settingsTextImport = '/settings/settings-import/text-import',
  editApiAccessMethods = '/settings/api-access-methods/edit/:id?',
  support = '/settings/support',
  // The forum sign-in finished by hand with the code the approval page
  // shows when its button did not open the app.
  forumSignInCode = '/settings/support/forum-sign-in-code',
  // The in-app bug report (doc 55): the forum's "Report a bug" form filed
  // with the wallet signature and the redacted logs, without a browser.
  reportProblem = '/settings/support/report-problem',
  debug = '/settings/debug',
  selectLocation = '/select-location',
  filter = '/select-location/filter',
  appInfo = '/settings/app-info',
  changelog = '/settings/changelog',
  appUpgrade = '/settings/app-upgrade',
  antiCensorship = '/settings/advanced/anti-censorship',
  lwo = '/settings/advanced/lwo',
  // Onboarding wizard (first-launch welcome + wallet
  // generate/import + subscription pointer + privacy preferences +
  // done). Re-triggerable from Settings ("Replay onboarding"). Uses
  // a route-based flow (vs modal overlay) so demo links and support
  // can deep-link a user to a specific step.
  onboardingWelcome = '/onboarding',
  onboardingWallet = '/onboarding/wallet',
  onboardingSubscription = '/onboarding/subscription',
  onboardingPreferences = '/onboarding/preferences',
  onboardingDone = '/onboarding/done',
}
