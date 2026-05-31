# Parity Audit 06 — App-level Settings, Support & Informational Screens

Reference = Electron desktop (`desktop/packages/mullvad-vpn/`). Targets = Android (`android/`) + iOS (`ios/`).
Scope = main Settings menu **except** VPN/tunnel settings and account (covered by other agents): language/locale, notifications, beta/app-updates, problem report, FAQ/guides, changelog/version, privacy/terms, theme/appearance, DNS-blocker entry point, Warren informational screens.

Severity: OK = parity (or sensible mobile equivalent) · P0 = user-facing feature absent on a platform that should clearly have it · P1 = meaningful UX gap / partial · P2 = minor / cosmetic / platform-idiomatic divergence.

## Summary table

| Feature | Electron | Android | iOS | Severity | Notes (file:line) |
|---|---|---|---|---|---|
| Settings menu screen | Yes `SettingsView` | Yes `SettingsScreen` | Yes `SettingsDataSource` | OK | desktop SettingsView.tsx:34; android SettingsScreen.kt:136; ios SettingsDataSource.swift:260 |
| App language / locale picker | Yes, in-app full picker under User-interface settings | Yes, but **only Android 13+** and delegates to system per-app language; buried under Appearance | Yes, in-app full picker (`.language` item) | P1 (Android) | desktop UserInterfaceSettingsView.tsx:51 + SelectLanguageView.tsx; android AppearanceScreen.kt:36-43,62-70 (gated `SDK_INT >= TIRAMISU`); ios SettingsDataSource.swift:306, SettingsViewModel.swift:16 |
| Notifications setting | Yes — master in-app on/off switch (system notifications) | Partial — "location in notification" toggle + button to open OS notification settings; **no in-app master enable switch** | Yes — `NotificationSettingsView` | P1 (Android) | desktop NotificationsSetting.tsx:10-34; android NotificationSettingsScreen.kt:143-155 (only `enable_location_in_notification` + system button); ios NotificationSettingsView.swift |
| Beta program opt-in | Yes — `BetaSetting` switch in App info | **Missing** (no beta toggle in appinfo/settings) | **Missing** (no beta item in SettingsDataSource) | P2 | desktop beta-setting/BetaSetting.tsx:11; android: grep beta in appinfo/settings = none; ios: no `.beta` Item. Mobile stores (Play/TestFlight) handle beta enrollment externally → acceptable, but no in-app opt-in/notify. |
| App update available / in-app upgrade | Yes — `UpdateAvailableListItem` + app-upgrade flow + outdated-version dialog in problem report | No in-app updater (Play handles); shows unsupported-version warning only | No in-app updater (App Store handles); shows unsupported warning | OK | desktop AppInfoView.tsx:34, ProblemReportView.tsx:329; android AppInfoScreen.kt:158 (`unsupported_version_description`); ios SettingsCellFactory shows warning. Desktop-by-nature; mobile parity via store. |
| Problem report — email field | Yes (optional) | Yes | Yes | OK | desktop ProblemReportView.tsx:165; android ReportProblemScreen.kt; ios ProblemReportViewController.swift |
| Problem report — message field | Yes | Yes | Yes | OK | desktop ProblemReportView.tsx:172; android ReportProblemScreen.kt; ios ProblemReportViewController.swift |
| Problem report — "no email" confirm dialog | Yes `NoEmailDialog` | Yes `ReportProblemNoEmailDialog` | Yes `ReduceAnonymityWarningView` | OK | desktop ProblemReportView.tsx:294; android noemail/ReportProblemNoEmailDialog.kt; ios ProblemReport/ReduceAnonymityWarningView.swift |
| Problem report — view/attach app logs | Yes "View app logs" | Yes (ViewLogs screen) | Yes (ProblemReportReviewViewController) | OK | desktop ProblemReportView.tsx:183-194; android viewlogs/ViewLogsScreen.kt; ios ProblemReportReviewViewController.swift |
| Problem report — sending/success/failed states | Yes | Yes (`Sending/Success/Error` previews) | Yes (submission overlay) | OK | desktop ProblemReportView.tsx:208-292; android ReportProblemScreen.kt:68; ios ProblemReportSubmissionOverlayView.swift |
| Problem report — outdated-version warning before send | Yes `OutdatedVersionWarningDialog` | Not present | Not present | P2 | desktop ProblemReportView.tsx:329. Minor; mobile relies on store updates. |
| FAQ / Guides link | Yes (external) | Yes (external) — **hidden on Play builds** | Yes (external) | OK | desktop FaqButton.tsx:9; android SettingsScreen.kt:273-275 (`if (!state.isPlayBuild)`); ios SettingsCellFactory.swift:82, SettingsCoordinator.swift:122 |
| Changelog / "What's new" | Yes `ChangelogView` | Yes `ChangelogScreen` (under App info) | Yes `ChangeLogView` (`.changelog` item) | OK | desktop changelog/ChangelogView.tsx; android changelog/ChangelogScreen.kt; ios ChangeLog/ChangeLogView.swift |
| App version display | Yes `VersionListItem` (App info) | Yes — subtitle on App-info row + Version row | Yes — subtitle on "What's new" cell + WarrenAbout | OK | desktop app-info/VersionListItem; android SettingsScreen.kt:295 + AppInfoScreen.kt:147-156; ios SettingsCellFactory.swift:68, WarrenAboutView.swift:33 |
| Privacy policy link | **Not in settings** (Mullvad upstream omits) | Yes — `PrivacyPolicy` external link in Settings | Yes — in WarrenAbout + Terms screen | OK | android SettingsScreen.kt:324-338; ios WarrenAboutView.swift:65 |
| Terms of service link | **Not in settings** | **Missing** | Yes — WarrenAbout + `TermsOfServiceView` | P2 | ios WarrenAboutView.swift:70, TermsOfServiceView.swift; android has only privacy policy. |
| "About Warren" informational screen | No (no Warren about page) | No dedicated About screen (only PP link) | Yes — `WarrenAboutView` (version, build, marketing site, privacy, terms, source AGPL, send feedback) | P2 | ios WarrenAboutView.swift:29-80. Warren-specific addition; not yet mirrored on desktop/Android. |
| Source code / AGPL license link | No | No | Yes | P2 | ios WarrenAboutView.swift:75-76 |
| "Send feedback" (mail composer) | No (problem report is the channel) | No | Yes (extra, alongside problem report) | P2 | ios WarrenAboutView.swift:80 |
| App appearance / theme entry | No (Mullvad has only monochrome tray icon + animate map) | Yes — `Appearance` screen (currently only hosts Language) | No dedicated appearance screen | P2 | android SettingsScreen.kt:251-257 → AppearanceScreen.kt; desktop UserInterfaceSettingsView.tsx:50,59 (tray icon, animate map). No true light/dark theme picker on any platform. |
| API access methods entry | Yes `ApiAccessMethodsListItem` | **Removed** (no-op, intentionally dropped) | Present as `.apiAccess` item (still wired) | P1 (consistency) | desktop SettingsView.tsx:72; android SettingsScreen.kt:248-249 + Settings.kt:111 (no-op, comment "Warren has no per-user API access"); ios SettingsDataSource.swift:285. **iOS still surfaces API access; Android deliberately removed it** → inconsistent Warren product decision. |
| Manage default DNS content blockers (nav entry) | Under VPN settings (`dns-blocker-settings`) | Under VPN/Warren tunnel settings | Under VPN settings | OK (out of this domain) | desktop vpn-settings/components/dns-blocker-settings; deferred to VPN-settings agent. |
| Replay onboarding / wizard | Yes `ReplayOnboardingListItem` | **Missing** | **Missing** | P2 | desktop SettingsView.tsx:79. No "replay intro/onboarding" entry on mobile. |
| Debug menu (dev only) | Yes `DebugListItem` (gated) | Build-variant gated | Diagnostic info (`warrenDiagnosticInfo`) | OK | desktop SettingsView.tsx:81; ios SettingsDataSource.swift:305 (`warrenDiagnosticInfo`). |
| Quit app button | Yes `QuitButton` | N/A (mobile) | N/A | OK | desktop SettingsView.tsx:83; desktop-by-nature. |
| Wallet entry (Warren identity) | (account agent) | Yes (top of settings) | Yes (`warrenWallet*`) | n/a | android SettingsScreen.kt:208-214; ios SettingsDataSource.swift:291-299. Covered by account agent. |

## Top gaps

1. **Android language picker is Android-13-only and buried** (P1). Desktop and iOS both offer an unconditional in-app language picker. On Android <13 there is **no way to change app language** (`AppearanceScreen.kt:37` gates on `SDK_INT >= TIRAMISU`, and the entry lives under "Appearance" rather than a UI-settings group). Desktop/iOS users get it directly. Consider an in-app fallback locale picker for pre-13 devices, or at least surface it more discoverably.

2. **Android notifications screen lacks an in-app master enable/disable switch** (P1). Desktop (`NotificationsSetting.tsx`) and iOS both expose a toggle for system notifications. Android only offers a "show location in notification" toggle plus a deep-link button to OS settings (`NotificationSettingsScreen.kt:143-155`). Functional parity is partial.

3. **API access entry is inconsistent between mobile platforms** (P1). Android intentionally removed it (Warren API endpoint is hardcoded — `SettingsScreen.kt:248`, `Settings.kt:111`), but **iOS still renders the `.apiAccess` row** (`SettingsDataSource.swift:285`) and desktop still shows `ApiAccessMethodsListItem`. This is a Warren product decision that hasn't been applied uniformly — either keep it everywhere or remove it on iOS/desktop too.

4. **Beta program opt-in absent on both mobile platforms** (P2). Desktop `BetaSetting` lets users opt into beta-release notifications. Mobile relies on Play/TestFlight enrollment, which is defensible, but there is no in-app equivalent or "get notified about betas" affordance.

5. **iOS-only informational surface drift** (P2). iOS added a Warren-specific `WarrenAboutView` (marketing site, privacy, terms, AGPL source, send feedback) and a Terms-of-Service screen. Android exposes only a Privacy Policy link; desktop exposes neither Terms nor an About screen. For brand/legal consistency, consider mirroring privacy + terms + source links on desktop and Android.

6. **"Replay onboarding" entry missing on mobile** (P2). Desktop `ReplayOnboardingListItem` lets users re-run the intro wizard; neither Android nor iOS offers this.

## Notes / non-issues
- Problem report flow has **full feature parity across all three platforms** (email optional, message, view/attach logs, no-email confirmation, sending/success/failed states). Android's `sendProblemReport` JNI path matches desktop + iOS UX. Only the desktop-extra "outdated version" pre-send dialog is unique (P2, store-handled on mobile).
- Changelog / "What's new" and app-version display are at parity (mobile nests changelog under App info; desktop has dedicated App-info view — equivalent).
- FAQ/Guides is at parity; Android additionally hides it on Play builds (store-policy-driven, acceptable).
- DNS content-blocker navigation lives under VPN/tunnel settings on all platforms → deferred to the VPN-settings parity agent.
- Desktop-only-by-nature items (Quit button, tray icon, unpinned-window, start-minimized, animate-map) excluded per scope.
