# Parity Audit 05, Onboarding, First Launch, Privacy Consent, Initial Setup

Scope: privacy/consent on first launch, onboarding wizard/walkthrough, first-launch
flow ordering, wallet create/import + mnemonic backup, device naming, first-connect
guidance, navigation + completion-flag persistence, post-onboarding tips.

Reference = Electron (`desktop/packages/mullvad-vpn/src/renderer`). Targets = Android
(`android/`), iOS (`ios/WarrenVPN`).

## Verdict

- **Electron**: full 5-step Warren onboarding wizard (Welcome → Wallet → Subscription
  → Preferences → Done), gated on `onboardingCompletedUnix`, replayable from Settings.
  No separate TOS/data-consent screen (no Mullvad consent screen in this fork).
- **iOS**: **near-exact parity**, a SwiftUI 5-step wizard with the same steps, wired
  into `ApplicationCoordinator.evaluateNextRoutes()`, plus an extra **Terms-of-Service /
  privacy screen** ahead of it. Completion persisted via `hasCompletedWarrenOnboarding`.
- **Android**: **NO onboarding wizard at all.** Flow is Splash → PrivacyDisclaimer →
  Wallet (create/import + backup) → Connect. No welcome / value-props / subscription /
  privacy-preferences / done steps. This is the headline P0 gap. (Earlier audit note
  confirmed: still NOT implemented.)

Cross-cutting nuance: on **both** Electron and iOS the wizard's privacy-preference
toggles (multi-hop / DAITA) are **scaffold-only**, they do not write the real settings
on finish. So that step is informational on all platforms that have it.

## Parity Table

| Feature | Electron | Android | iOS | Severity | Notes (file:line) |
|---|---|---|---|---|---|
| First-launch flow ordering | login → **onboarding wizard** → main | Splash → PrivacyDisclaimer → Wallet → Connect (no wizard) | TOS → **onboarding wizard** → login/welcome/main | P0 (Android) | EL `lib/functions/navigation-base.ts:27`; AND `screen/splash/SplashViewModel.kt:44`; iOS `Coordinators/ApplicationCoordinator.swift:357` |
| Privacy / data-collection consent screen | None (no Mullvad consent in fork) | **PrivacyDisclaimer** screen (title + 2 paragraphs + privacy-policy link + "Agree and continue") | **TermsOfService** screen ("right to privacy… never store logs" + privacy-policy link) | OK / P2 | AND `screen/privacy/PrivacyDisclaimerScreen.kt:131`; iOS `View controllers/TermsOfService/TermsOfServiceView.swift:17`. EL has neither; minor divergence, both mobile add a consent gate EL lacks. |
| Onboarding wizard / walkthrough (multi-step) | **5 steps**: Welcome, Wallet, Subscription, Preferences, Done | **Absent** | **5 steps**: Welcome, Wallet, Subscription, Privacy prefs, Done | P0 (Android) | EL `components/views/onboarding/index.ts:1`; iOS `View controllers/Onboarding/OnboardingWizardView.swift:19` (enum OnboardingStep .welcome….done) |
| Step indicator / progress | Implicit via routes (back button per step) | n/a | Capsule step indicator "Step N of 5" + a11y label | OK | iOS `OnboardingWizardView.swift:120-137` |
| Welcome / value-prop step | "Welcome to Warren VPN, No logs. No accounts. No tracking." | Absent | "Welcome to Warren VPN, decentralized, non-custodial, HTTP/3 mimicry, multi-hop" | P0 (Android) | EL `OnboardingWelcomeView.tsx:20`; iOS `OnboardingWizardView.swift:142` |
| Wallet create-vs-import choice | Pick → Generate / Import (in-wizard) | **Yes** but outside wizard: `WarrenWalletLoginScreen` (Generate / Restore) | Step 2 delegates to `WarrenWalletCoordinator` (.generate / .importExisting) | OK | EL `OnboardingWalletView.tsx:123`; AND `feature/login/impl/WarrenWalletLoginScreen.kt:103`; iOS `OnboardingWizardView.swift:175` + `Coordinators/OnboardingWizardCoordinator.swift:69` |
| Mnemonic backup display (12 words) | MnemonicGrid + copy button | `WarrenWalletBackupScreen` (phrase + copy) | Via WarrenWalletCoordinator generate flow | OK | EL `OnboardingWalletView.tsx:175`; AND `feature/login/impl/WarrenWalletBackupScreen.kt:64` |
| Mnemonic backup confirmation gate | Checkbox "I have written it down" gates Continue | Button "I have written it down" (no checkbox gate, single confirm) | In WarrenWalletCoordinator | P2 (AND) | EL `OnboardingWalletView.tsx:181`; AND `WarrenWalletBackupScreen.kt:86` (button, not a gating checkbox) |
| Mnemonic import / restore | 12/24-word textarea, daemon-validated | `MnemonicInput` inline; importWallet | importExisting entry point | OK | EL `OnboardingWalletView.tsx:196`; AND `WarrenWalletLoginScreen.kt:84` |
| Subscription pointer step | Opens warrenvpn.com/pricing in browser + auto-poll for activation + "I already have one" verify | Absent in flow (subscription only in Settings later) | Opens warrenbrowse.com/pricing in SFSafari + "Maybe later" | P1 (AND) / P2 (iOS) | EL `OnboardingSubscriptionView.tsx:102` (10s poll/2min + `updateAccountData` verify); iOS `OnboardingWizardCoordinator.swift:111` (no poll/verify, just opens link) |
| Privacy-preferences step (multi-hop/DAITA/obfuscation) | Informational text only (toggles documented as TODO scaffold) | Absent | 3 real-looking toggles (multi-hop, DAITA, HTTP/3 forced-on) **but not persisted** | P1 (AND); P2 (EL+iOS) | EL `OnboardingPreferencesView.tsx:40` (comment: "Embed once wizard ships with IPC bindings"); iOS `OnboardingWizardView.swift:270`, bindings local to `OnboardingWizardState`, never written to `WarrenSettings.multiHopEnabled` (see Settings `WarrenMultiHopSettingsView.swift:21`) |
| Done / first-connect guidance step | "All set. Pick a country and connect." → main | Absent | "You're all set. Tap Launch Warren to connect." → finish | P0 (Android) | EL `OnboardingDoneView.tsx:27`; iOS `OnboardingWizardView.swift:319` |
| Device naming during setup | No | No (auto device naming, no setup prompt) | Welcome screen shows `deviceName` (Mullvad upstream account flow, not Warren wizard) | OK | iOS `View controllers/CreationAccount/Welcome/WelcomeContentView.swift:18,68`. Not a wizard step on any platform. |
| Skip / Get-started / Next navigation | "Get started", per-step "Continue", "Skip wizard (advanced)" link | n/a (linear forced wallet flow) | "Get started"/"Continue"/"Maybe later"/"Launch Warren"; no global skip link | P2 (iOS) | EL `components/OnboardingLayout.tsx:84-95` (skip link marks complete); iOS has per-step secondary buttons but no "skip wizard" affordance |
| Completion-flag persistence (show once) | `setOnboardingCompletedUnix` on finish/skip; gate in `getNavigationBase` | n/a (no wizard); wallet presence drives routing | `appPreferences.hasCompletedWarrenOnboarding = true` on finish | OK (EL/iOS); n/a (AND) | EL `OnboardingDoneView.tsx:21` + `OnboardingLayout.tsx:60` + `navigation-base.ts:27`; iOS `ApplicationCoordinator.swift:1294-1296` + gate `:367` |
| Replay onboarding (re-run from Settings) | "Replay onboarding" Settings item clears flag → wizard | n/a | No replay entry found (re-shown only after logout/reset) | P2 (iOS) | EL `components/replay-onboarding-list-item/ReplayOnboardingListItem.tsx:21`; iOS, no equivalent Settings item located |
| Post-onboarding tips / coachmarks | None | None | None | OK | No coachmark system on any platform. |

## Top Gaps

1. **P0, Android has no onboarding wizard.** Electron and iOS both present a 5-step
   guided first-run (Welcome / Wallet / Subscription / Preferences / Done). Android jumps
   straight from PrivacyDisclaimer into the bare `WarrenWalletLoginScreen` then Connect,
   with no welcome/value-prop, no subscription pointer, no preferences intro, and no
   "you're all set" first-connect guidance. The wallet create/import + backup primitives
   already exist (`feature/login/impl`), so the missing work is the wizard chrome +
   subscription + welcome/done steps, not the wallet plumbing.

2. **P1, Subscription step divergence.** Electron's subscription step actively verifies
   enrollment: it opens pricing, auto-polls `updateAccountData()` every 10s for 2 min, and
   has an "I already have a subscription" check that errors if none is active
   (`OnboardingSubscriptionView.tsx:98-126`). iOS just opens the SFSafari link with a
   "Maybe later" button, no polling, no verification. Android has no subscription step in
   the flow at all.

3. **P1/P2, Privacy-preferences toggles are non-functional on every platform that has
   them.** Electron renders the prefs step as static text (toggles are an explicit TODO,
   `OnboardingPreferencesView.tsx:40`). iOS renders real toggles but never persists them, 
   `multiHopEnabled`/`daitaEnabled` live only in `OnboardingWizardState` and are dropped on
   finish; the real settings are written elsewhere (`WarrenMultiHopSettingsView.swift:21`).
   Net: the step is decorative everywhere; iOS is misleading because the toggles *look*
   functional.

4. **P2, Minor consent/replay divergences.** Electron has no first-launch privacy/TOS
   consent screen, while Android (PrivacyDisclaimer) and iOS (TermsOfService) both gate the
   flow on a consent screen, arguably mobile is *ahead* here, but the three platforms are
   inconsistent. Electron's "Replay onboarding" Settings entry has no iOS/Android
   equivalent. Android's mnemonic backup uses a confirm button rather than Electron's
   gating "I have written it down" checkbox.

## Evidence index (key files)

- Electron wizard: `desktop/packages/mullvad-vpn/src/renderer/components/views/onboarding/`
  (`OnboardingWelcomeView.tsx`, `OnboardingWalletView.tsx`, `OnboardingSubscriptionView.tsx`,
  `OnboardingPreferencesView.tsx`, `OnboardingDoneView.tsx`, `components/OnboardingLayout.tsx`);
  routing `renderer/lib/functions/navigation-base.ts`; replay `.../settings/components/replay-onboarding-list-item/ReplayOnboardingListItem.tsx`.
- Android flow: `android/app/src/main/kotlin/com/warrenbrowse/vpn/screen/splash/SplashViewModel.kt`,
  `.../screen/privacy/PrivacyDisclaimerScreen.kt`; wallet
  `android/lib/feature/login/impl/.../WarrenWalletLoginScreen.kt`, `WarrenWalletBackupScreen.kt`.
  No `Onboarding*`/`Welcome*`/`Wizard*` wizard NavKey exists in the app nav graph.
- iOS wizard: `ios/WarrenVPN/View controllers/Onboarding/OnboardingWizardView.swift`,
  `ios/WarrenVPN/Coordinators/OnboardingWizardCoordinator.swift`; wiring + TOS gate
  `ios/WarrenVPN/Coordinators/ApplicationCoordinator.swift:357-448,1287-1296`; consent
  `ios/WarrenVPN/View controllers/TermsOfService/TermsOfServiceView.swift`.
