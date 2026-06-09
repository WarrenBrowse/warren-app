# iOS UI/UX & User-Flow Parity Audit (vs Android + Desktop)

Date: 2026-06-09
Scope: Warren iOS client (`ios/WarrenVPN`, SwiftUI/UIKit) UI/UX and user-flow
parity against the Android app (`android/lib/feature/*`, `android/lib/ui/*`,
`android/lib/repository/*`) and the desktop Electron app
(`desktop/packages/mullvad-vpn/src/renderer`).

Method: read-only. No code changed.

---

## Severity-sorted summary

| # | Area | Finding | Severity | iOS file(s) | Reference |
|---|------|---------|----------|-------------|-----------|
| 1 | Account model / Login | iOS login is the **unmodified Mullvad account-number flow** ("Enter your account number" text field + Create-account). No wallet Create / Restore-by-phrase login. The wallet only appears as a one-time onboarding-wizard step, decoupled from the actual login/auth. | HIGH | `View controllers/Login/LoginViewController.swift`, `Coordinators/LoginCoordinator.swift`, `Coordinators/ApplicationCoordinator.swift:357-390` | desktop `views/login/LoginView.tsx` (Create / Restore-by-phrase, `backup-pending`) |
| 2 | Logout / sign-out | iOS `logout()` only calls `tunnelManager.unsetAccount()`; it does **not** wipe the wallet vault. Wallet erase is an unrelated, manual Settings item. Not a "true sign-out (wipe vault + history)". | HIGH | `View controllers/Account/AccountInteractor.swift:60`, `Coordinators/AccountCoordinator.swift:160`, `View controllers/Wallet/WarrenWalletEraseViewController.swift` | desktop logout = wipe vault + history |
| 3 | Mnemonic reveal/copy UX | iOS shows the phrase **blurred behind a hold-to-reveal gesture with NO copy button** (explicit anti-copy comment). This is the exact opposite of the agreed Warren UX (show directly + copy button, no blur). | HIGH | `View controllers/Wallet/WarrenMnemonicDisplayView.swift:8-13,63-75,89-119` | desktop `OnboardingWalletView.tsx:99-103` & `login/LoginView.tsx:323-324` (`MnemonicGrid revealed` + `CopyMnemonicButton`) |
| 4 | Blocked-state / error surfacing | The Warren tunnel actor never produces a real blocked/error state: `setErrorState(reason:)` is a logged no-op and all non-disconnect states collapse to `.initial`. The Mullvad notification UI plumbing exists but is **never fed a Warren blocking reason**. | HIGH | `PacketTunnelCore/Actor/WarrenQuinnActor.swift:326-330,131-134` ; provider intact at `Notifications/Notification Providers/TunnelStatusNotificationProvider.swift` | desktop `LoginView.tsx:401-445` BlockMessage; Android blocked-state reasons |
| 5 | NAT-PMP assigned port display | The NAT-PMP settings screen is a **non-functional scaffold**: toggle handler is a TODO, `refreshFromAppGroup()` is empty, so the assigned external port is never read/shown even though the extension writes it. NAT-PMP is also disabled in the tunnel actor. | HIGH | `View controllers/Settings/WarrenNatPmpSettingsView.swift:55-87`, `PacketTunnelCore/Actor/WarrenQuinnActor.swift:216` (`natPmpEnabled: false`) | extension writes `WarrenAppGroupKey.natPmpExternalPort`; Android port-forwarding UI |
| 6 | Onboarding wizard privacy prefs not persisted | Wizard "Privacy preferences" toggles (multi-hop, DAITA) are local `@Published` state that is read by nobody. Selecting them does **not** change tunnel settings. | MED | `View controllers/Onboarding/OnboardingWizardView.swift:31-52,270-315`, `Coordinators/OnboardingWizardCoordinator.swift` | desktop onboarding subscription/privacy steps persist |
| 7 | Wallet step skippable in onboarding | Onboarding wallet step offers a `onSkip` path (advance without a wallet). There is no mandatory "backup-pending" gate equivalent at the login level. | MED | `View controllers/Onboarding/OnboardingWizardView.swift:89-95,175-225` | desktop `backup-pending` mandatory gate in `LoginView.tsx` |
| 8 | Feature indicators: no QUIC chip, no DAITA_MULTIHOP collapse | iOS connection card uses the Mullvad chip set (DAITA, Multihop, Obfuscation, QuantumResistance, IPOverride, ...). QUIC mimicry is only a separate static banner, not a feature chip, and there is no collapsed `DAITA_MULTIHOP` indicator. | MED | `View controllers/Tunnel/ConnectionView/FeatureIndicatorsViewModel.swift:27-61`, `WarrenObfuscationIndicatorView.swift` | Android `lib/repository/ConnectionProxy.kt:85-93` (`DAITA`, `MULTIHOP`, `DAITA_MULTIHOP`, always-on `QUIC` chip) |
| 9 | Account view still Mullvad-shaped | Account screen shows the Mullvad account-number row + device row, not the wallet/SS58 identity. (Wallet identity lives in a separate Settings item.) | MED | `View controllers/Account/AccountContentView.swift:40-60`, `Account/AccountNumberRow.swift`, `Wallet/WarrenWalletIdentityView.swift` | desktop account = wallet identity |
| 10 | `allow-external-DNS` advanced toggle absent | The Warren advanced "allow external DNS" toggle (present on desktop/CLI) does not exist anywhere in iOS settings. | MED | none (grep `allowExternalDns` in `ios/**.swift` = 0 hits); `View controllers/VPNSettings/VPNSettingsViewModel.swift` has only custom-DNS | desktop/CLI `allow_external_dns` |
| 11 | Localized app name still "Mullvad VPN" | 16 localized `InfoPlist.strings` keep `CFBundleDisplayName = "Mullvad VPN"` / `CFBundleName = "MullvadVPN"` (home-screen + iOS Settings name for those locales). | MED (LOW user reach per-locale) | `Supporting Files/{fi,ja,da,es,it,ko,tr,ru,pl,nl,th,pt,my,nb,zh-Hans,zh-Hant}.lproj/InfoPlist.strings` | en/Base already corrected |
| 12 | Mnemonic display "NOT wired" comment | `WarrenMnemonicDisplayView` header says "NOT yet wired into the Xcode project" but it IS used by `WarrenWalletGenerateViewController`. Stale/misleading comment (and the doctrine it implements is wrong, see #3). | LOW | `View controllers/Wallet/WarrenMnemonicDisplayView.swift:8-13` | n/a |
| 13 | Split tunneling | Absent on iOS. Expected platform limitation (Mullvad iOS never shipped split tunneling); iOS uses `includeAllNetworks` instead. Noted for completeness, not a regression. | LOW (informational) | `Views/SplitMainButton.swift` is the connect-button split, unrelated | Android `feature/splittunneling`, desktop `views/split-tunneling` |

---

## Detail

### 1. Account model / login (HIGH)

The desktop redesign (`views/login/LoginView.tsx`) makes the **logged-out entry
screen itself** the wallet model: a "Create a new account" path that mints a
fresh identity and gates on a mandatory `backup-pending` 12-word backup, plus an
"I already have an account" path that restores from a pasted recovery phrase
(`MnemonicTextarea` + `setWarrenMnemonic`). There is no public-key/account-number
input.

iOS does not do this. `ApplicationCoordinator.evaluateNextRoutes()`
(lines 357-390) runs a one-time **Warren onboarding wizard** when
`hasCompletedWarrenOnboarding == false`, then falls back to the standard Mullvad
device-state machine: `.loggedOut -> .login`. `presentLogin` builds the legacy
`LoginViewController` (`LoginCoordinator.swift:45-72`), whose UI is the unmodified
Mullvad account-number text field + "Create account" button
(`LoginViewController.swift:153-190,238-289,422-439`: "Enter your account number",
"Valid account number", `accountInputGroup.textField`, `interactor.createAccount()`,
`interactor.setAccount(accountNumber:)`).

Net effect: the wallet (minted/imported in the wizard) and the actual auth
identity (Mullvad account number) are two parallel, unrelated systems on iOS. A
fresh user who completes the wizard then lands on a Mullvad account-number login
screen.

Recommended action: replace the iOS login screen with a Warren wallet login
mirroring desktop (Create / Restore-by-phrase, mandatory backup gate), and route
`.loggedOut` to it. Drive the Mullvad device-state from the wallet identity
rather than account numbers.

### 2. Logout (HIGH)

`AccountInteractor.logout()` (line 60-61) = `await tunnelManager.unsetAccount()`
only. It does not call `WarrenWalletInteractor.forgetWallet()` /
`WarrenWalletKeychain.delete()`. Wallet erasure is reachable only as a separate,
manual Settings row (`SettingsViewControllerFactory.swift:205`,
`WarrenWalletEraseViewController`). So "logout" leaves the wallet vault on the
device, diverging from the desktop "true sign-out wipes vault + history".

Recommended action: make logout wipe the wallet Keychain entry (and any cached
history) as part of the sign-out, or redefine the iOS logout to match desktop
semantics.

### 3. Mnemonic reveal + copy (HIGH)

iOS `WarrenMnemonicDisplayView`:
- Hides the phrase behind a `blurOverlay` (lines 146-158) revealed only by a
  `LongPressGesture` that re-hides after 1.5 s (lines 64-73).
- Has **no copy button**; the header comment (lines 8-13) explicitly states
  "no copy button by design" and "NOT yet wired into the Xcode project".
- Adds a screenshot-detection warning alert (lines 89-119).

Desktop (`OnboardingWalletView.tsx:99-103`, `LoginView.tsx:323-324`) does the
opposite, intentionally: `MnemonicGrid ... revealed` (shown directly, no blur)
plus `CopyMnemonicButton` (copy enabled). Per the project memory
(`warren_mnemonic_ux_decision`), this is the agreed doctrine and reverses the old
blur/no-copy approach; onboarding and the backup view must stay aligned.

Recommended action: replace blur+hold with a directly-shown grid and add a copy
button (matching desktop `MnemonicGrid revealed` + `CopyMnemonicButton`). Keep the
screenshot warning if desired, but it should not gate visibility.

### 4. Blocked-state / error surfacing (HIGH)

The Mullvad notification surface is intact:
`TunnelStatusNotificationProvider.swift` extracts `BlockedStateReason` (lines
130-164) and renders localized reasons; `InAppNotificationProvider` and the
account-expiry / new-device / new-version providers exist.

But the Warren tunnel actor never produces a real blocked state:
`WarrenQuinnActor.setErrorState(reason:)` (lines 326-330) is a logged no-op
("C.4.3 scaffold no-op, reason=..."), `reconnect` is a no-op (line 314-316), and
the observed-state mapping collapses every non-disconnect to `.initial`
(lines 131-134). Consequence: "unstable network", "blocking internet", offline,
and other blocking reasons are never surfaced from a Warren tunnel even though the
UI could render them. The offline/expired/revoked notifications that flow from
`TunnelManager` device state still work; the **tunnel-level blocked reasons do
not**.

Recommended action: implement `setErrorState` to publish a blocked state with a
real `BlockedStateReason`, and map Warren observed states (connected /
reconnecting / blocked) so the existing notification provider can render them.

### 5. NAT-PMP assigned port (HIGH)

`WarrenNatPmpSettingsView` is a scaffold:
- The toggle `onChange` body is `_ = newValue` with a TODO (lines 56-61): it does
  not persist or trigger a mapping request.
- `refreshFromAppGroup()` (lines 81-87) is empty with a TODO: it never reads the
  port, so `forwardedPort` stays `nil` and the row spins forever.

Meanwhile the extension DOES write the port:
`WarrenQuinnTunnelImplementation.swift:217-219` sets
`WarrenAppGroupKey.natPmpExternalPort` on `.natPmpMapped/.natPmpRenewed`. And
`WarrenQuinnActor.swift:216` shows NAT-PMP is currently `natPmpEnabled: false`
("settings-driven, deferred (needs V9 schema)"), so no port is requested at all.

So the headline NAT-PMP differentiator is non-functional end-to-end on iOS, and
the assigned port is never displayed.

Recommended action: wire `refreshFromAppGroup()` to read the App Group keys, make
the toggle persist + drive the extension's NAT-PMP request/release, and enable
`natPmpEnabled` from settings.

### 6-7. Onboarding wizard gaps (MED)

`OnboardingWizardView` privacy toggles `multiHopEnabled` / `daitaEnabled`
(state lines 33-35, UI lines 287-298) are never consumed by the coordinator or
written to tunnel settings; the HTTP/3 row is a hardcoded disabled `true`. The
wallet step exposes `onSkip` (lines 92-93, button 215) so a user can proceed with
no wallet, and there is no mandatory backup gate equivalent to desktop's
`backup-pending` (which blocks settings escape until the phrase is acknowledged).

Recommended action: persist the privacy-prefs selections to settings, and either
remove the skip path or add a mandatory backup gate aligned with desktop.

### 8. Feature indicators (MED)

`FeatureIndicatorsViewModel.chips` (lines 27-61) uses the Mullvad feature set
(`DaitaFeature`, `QuantumResistanceFeature`, `MultihopFeature`,
`ObfuscationFeature`, `DNSFeature`, `IPOverrideFeature`,
`IncludeAllNetworksFeature`, `LocalNetworkSharingFeature`, `IPVersionFeature`).
There is no QUIC chip in the indicator row (QUIC mimicry is only the static
`WarrenObfuscationIndicatorView` banner, `ConnectionView.swift:87`) and no
collapsed `DAITA_MULTIHOP` chip.

Android (`lib/repository/ConnectionProxy.kt:85-93`) emits Warren-specific
indicators: `DAITA`, `MULTIHOP`, a collapsed `DAITA_MULTIHOP` when both are on,
and an always-on `QUIC` chip for every Warren tunnel (tests at
`ConnectionProxyTest.kt:84-124`).

Recommended action: add a QUIC always-on chip and a DAITA_MULTIHOP collapse to
the iOS indicator set to match Android/the Warren model, and prune
Mullvad-only/WireGuard-only indicators (QuantumResistance) that don't apply.

### 9. Account view shape (MED)

`AccountContentView` (lines 40-60) shows `AccountDeviceRow`, `AccountNumberRow`
(Mullvad account number), `AccountExpiryRow`, the in-app `purchaseButton`, voucher
redemption (`RedeemVoucher*`), and `logoutButton`. The wallet/SS58 identity is not
shown here; it lives in a separate Settings item (`WarrenWalletIdentityView`).

Recommended action: surface the wallet identity (SS58 `wb…` address, copy) in the
account view and drop the account-number row once #1 lands.

### 10. allow-external-DNS (MED)

`grep allowExternalDns` over `ios/**.swift` returns nothing. iOS VPN settings only
have custom DNS + content blockers (`VPNSettingsViewModel.swift`). The Warren
advanced "allow external DNS" toggle present on desktop/CLI has no iOS equivalent.

Recommended action: add the advanced toggle to iOS DNS settings, wired to the same
tunnel/firewall setting.

### 11. Localized app name "Mullvad VPN" (MED branding)

16 `InfoPlist.strings` files still carry `CFBundleDisplayName = "Mullvad VPN"` and
`CFBundleName = "MullvadVPN"` (fi, ja, da, es, it, ko, tr, ru, pl, nl, th, pt, my,
nb, zh-Hans, zh-Hant). These drive the home-screen icon label and the iOS
Settings > General > VPN name for those locales. The en/Base variants were already
corrected; the main `Localizable.xcstrings` are clean (0 "Mullvad").

Recommended action: bulk-replace the display/bundle name strings in the 16
localized `InfoPlist.strings` to "Warren VPN" / "WarrenVPN".

### 12. Stale "NOT wired" comment (LOW)

`WarrenMnemonicDisplayView.swift:13` claims it is "NOT yet wired into the Xcode
project", but `WarrenWalletGenerateViewController.installDisplayView` instantiates
it (line 93). Comment is stale; fix or remove when addressing #3.

### 13. Split tunneling (LOW / informational)

No per-app split tunneling on iOS (`SplitMainButton.swift` is the connect-button
split UI, unrelated). This matches upstream Mullvad iOS (never shipped split
tunneling); iOS exposes `includeAllNetworks` instead. Android and desktop have
split tunneling. Not a Warren regression, listed for completeness.

---

## What is present and aligned (no action)

- Wallet engine + flows exist and are sound: `WarrenWalletInteractor` (generate /
  import / save / load-with-FaceID / forget), `WarrenWalletKeychain`,
  `WarrenWalletCoordinator` (generate / import / backup entry points),
  `WarrenMnemonicInputView` (restore grid).
- Onboarding wizard exists (5 steps) and is reachable on first launch.
- Connection card endpoint display: `DetailsView` shows in/out addresses
  (reused from Mullvad, functional).
- QUIC always-on mimicry is explained to the user (banner + read-only obfuscation
  settings screen `WarrenObfuscationSettingsReadOnlyView`).
- Settings: relay/location picker, DAITA toggle, multihop toggle + entry selection,
  custom DNS, kill switch (`includeAllNetworks`), language, notification settings,
  wallet backup/erase/identity, tunnel statistics, diagnostic info, about, problem
  report / support flow, FAQ.
- Subscription/billing: in-app purchase button, voucher redemption
  (`RedeemVoucher*`, `ProfileVoucherCoordinator`), account expiry display,
  out-of-time flow, subscription link to warrenbrowse.com in onboarding.
- Branding: main `Localizable.xcstrings` are clean of "Mullvad"; the LogoText
  asset is a real path-based "WARREN" wordmark SVG (not a text-element placeholder).
