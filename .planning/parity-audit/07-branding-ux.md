# Parity Audit 07, Branding & Global UX

**Domain:** Branding, visual identity, naming, navigation/IA, localization, global UX consistency.
**Reference platform:** Electron desktop (`desktop/packages/mullvad-vpn/`).
**Targets:** Android (`android/`), iOS (`ios/`).
**Method:** Evidence-based grep + asset inspection. No source files modified.
**Date:** 2026-05-31.

---

## Executive summary

The **text rebrand to "Warren" is essentially complete on all three platforms**, app name is "Warren VPN" everywhere, support email is `support@warrenbrowse.com`, "WireGuard" is rebranded to "QUIC" in user-visible strings, and there are no residual user-facing "Mullvad" strings (the only ones left are intentional GPL fork attributions and internal code identifiers).

The **visual identity is where the gaps are**, almost entirely on **Android**:

- **P0, Android still ships the Mullvad marmot logo** (`logo_icon.png`, `launch_logo.png`, `small_logo_*.png`) on the splash screen, top bar of nearly every screen, quick-settings tile, and notifications. The launcher *adaptive icon* was rebranded to a Warren "W" but these in-app PNGs were never replaced.
- **P1, iOS app icon is a low-quality placeholder** (cropped "WARREN" wordmark on black, off-center).
- **P1/P2, Color scheme is the inherited Mullvad blue/green palette on all three platforms.** A Warren-yellow accent token (`#FFD524`) is defined on every platform but is barely wired (Android: only the `warning` color; iOS: only wallet/onboarding views). This is *consistent across platforms*, so it reads as a deliberate "dark-blue chrome + yellow accent" decision rather than a bug, flagged P2 except where it diverges.
- **P2, Leftover Mullvad "disguise icon" assets on Android** ("Whack-a-mole", "Ninja Mole" mole-themed launcher icons), dead assets, the feature is not wired into the manifest.

Localization coverage is near-identical across platforms (~21-23 languages).

---

## Findings table

| Aspect | Electron | Android | iOS | Severity | Notes (file:line / grep) |
|---|---|---|---|---|---|
| **App name** | "Warren VPN" | "Warren VPN" | "Warren VPN" | OK | desktop `package.json:4` `productName`; android `values/strings_non_translatable.xml:2` `app_name`; ios `InfoPlist.xcstrings` CFBundleDisplayName/CFBundleName = "Warren VPN" |
| **Package / namespace** | `mullvad-vpn` (npm name only, not user-visible) | `com.warrenbrowse.vpn` | `WarrenVPN` host target | OK | Android pkg fully rebranded (commit d90954ca7c); iOS host renamed (commit 26b00faa4a). npm package name `mullvad-vpn` is internal only. |
| **Residual "Mullvad" in user strings** | 0 visible | 0 | 0 (1 intentional) | OK | Desktop: 23 hits all comments/code IDs (`Ownership.mullvadOwned`, `methods.mullvadBridges`). Android base `strings.xml`/`strings_non_translatable.xml`: **0**. iOS: only `WarrenAboutView.swift:88` legal fork credit ("Warren is a fork of Mullvad VPN…"), appropriate GPL attribution. |
| **Residual "WireGuard" in user strings** | 0 visible | 0 | 2 | P2 | `strings.wireguard = 'QUIC'` (desktop `shared/constants/strings.ts:2`) rebrands the placeholder. Desktop 154 raw hits are the `'wireguard-settings-view'` msgctxt namespace (not displayed). iOS: 2 leaks in `ios/Assets/Localizable.xcstrings` (Finnish "Wireguard-porttia" L60074, Russian "wireGuard порт" L60134), port-out-of-range msg not rebranded in those 2 locales only. |
| **Launcher / app icon** | Warren "W" (logo-icon.svg) | Warren "W" monogram (adaptive icon) | Warren wordmark, **placeholder quality** | P1 (iOS) | Android `drawable/icon_android.xml` = white "W" stroke path (commit 9234297d7c). iOS `AppIcon.appiconset/Icon-Light-1024x1024.png` shows cropped "WARREN" text on black, off-center / oversized, needs a polished icon. |
| **In-app logo (splash / top bar / tile / notif)** | Warren "W" | **MULLVAD MARMOT** | Warren "W" (gradient) | **P0 (Android)** | Android `drawable-*/logo_icon.png` + `launch_logo.png` + `small_logo_{black,white}.png` = **Mullvad marmot in hard-hat** (verified by image). Used in `TopBar.kt:121`, `SplashScreen.kt:82`, `NoDaemonScreen.kt:72`, `WarrenTileService.kt:43-44`, `TunnelStateNotificationAction.kt:24`, TV `NavigationDrawerTv.kt:189`. iOS `LogoIcon.imageset/logo-icon.svg` = proper Warren "W" with brand gradient. |
| **Disguise / alternate icons (mole feature)** | n/a | **Leftover Mullvad mole assets** | n/a | P2 | Android `app_name_game`="Whack-a-mole", `app_name_ninja`="Ninja Mole" (`strings_non_translatable.xml:22-23`); `mipmap/ic_launcher_{game,ninja,weather,notes,browser}.xml` + foreground vectors present. No `activity-alias` in `app/src/main/AndroidManifest.xml` → feature not shipped → dead branded assets. |
| **Brand color: accent (Warren yellow #FFD524)** | Defined + tokenized | Defined, barely used | Defined, scoped to new views | P2 | Desktop `color-tokens.ts:22` `yellow: rgb(255,213,36)`. Android `PaletteTokens.kt:17` `Yellow=0xFFFFD524` → only `ColorScheme.warning` (`Color.kt:33`); `colors.xml` also has unused `warrenYellow #F5C518`. iOS `UIColor+Warren.swift:23` yellow #FFD524 → only wallet/onboarding/Warren feature views. Values match across platforms = good. |
| **Primary chrome / color scheme** | Mullvad dark-blue + green secured | Mullvad dark-blue + green | Mullvad dark-blue (#294D73) + green | P2 (consistent) | Android `ColorDarkTokens.kt:29` `Primary = Blue`. iOS `UIColor+Palette.swift:152` `primaryColor = (0.16,0.30,0.45)` = Mullvad blue, `successColor` green. Desktop keeps `green: rgb(68,173,77)` for secured. Consistent across all 3 → reads as intentional. Brand-differentiation opportunity, not a defect. |
| **Token-name leakage (`MullvadWhite`, `UIFont.mullvad*`)** | - | `PaletteTokens.MullvadWhite` | `UIFont.mullvadSmall/Tiny` etc. | P2 | Internal identifiers, not user-visible. Android `PaletteTokens.kt:24`; iOS APIAccess cells. Cosmetic / rebase-noise only. |
| **Typography / fonts** | System + tokens | System (Material3, `Type.kt`) | System (`UIFont.systemFont`) | OK | No custom brand font on any platform; all use platform system fonts. Consistent. iOS internal typography enum still named `MullvadMaterial3Typography`-equivalent on Android (`Type.kt:18`), cosmetic. |
| **Navigation / IA (top-level screens)** | Main / Select location / Settings / Account | Same + TV variant | Same | OK | Equivalent IA across platforms (Connect view, location picker, settings tree, account). Warren-specific additions (wallet, multi-hop, warren-mode, pubkey-warning, obfuscation indicator) present on all. |
| **Localization coverage (languages)** | 23 `.po` locales | 23 `values-*` locales | 21 `.xcstrings` locales | OK | Desktop: 23 dirs under `locales/`. Android: 23 `values-*/strings.xml`. iOS: 21 (`da de es fi fr it ja ko my nb nl pl pt ru sv th tr uk zh-Hans zh-Hant` + en), 643 keys. iOS missing `ar`, `ro`, `fa` vs the other two, minor gap. |
| **Iconography (feature/status icons)** | Shared icon set | Material/vector icons | `Icon*.imageset` full set | OK | iOS has complete branded icon set (`Assets.xcassets`: IconState{Online,Offline,Issue}, multihop, daita illustrations, etc.). No marmot in feature icons. Consistent semantic iconography. |
| **Empty states / error copy tone** | Warren-branded | Warren-branded | Warren-branded | OK | e.g. Android `connecting_to_daemon`="Connecting to Warren system service…", always-on-VPN error references "Warren VPN" (`strings.xml:96,99`). Consistent tone. |
| **Accessibility (labels / dynamic type)** | aria-labels present | content descriptions | localized a11y labels | OK | iOS has `BlockedStateReason+Localization.swift`, contentDescription patterns; desktop uses `aria-label` (e.g. `WarrenPubKeyWarning.tsx`). No systematic gap found at audit altitude. |
| **Placeholder / unbranded UI** | none found | mole logos (above) | placeholder app icon (above) | see P0/P1 | Covered by icon rows above. |

---

## Top gaps (prioritized)

### P0, Android in-app logo is still the Mullvad marmot
The splash screen, top bar (nearly every screen), quick-settings tile and notification small-icon all render `logo_icon.png` / `launch_logo.png` / `small_logo_{black,white}.png`, which are the **Mullvad marmot mascot in a yellow hard-hat** (verified visually). Only the launcher adaptive icon was swapped to a Warren "W". This is the single most visible branding defect, a user sees the marmot on app launch (splash) and on every screen's header.
- Files: `android/lib/ui/resource/src/main/res/drawable-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/logo_icon.png`, `launch_logo.png`, `drawable-mdpi/small_logo_{black,white}.png`.
- Consumers: `TopBar.kt:121`, `SplashScreen.kt:82`, `NoDaemonScreen.kt:72`, `WarrenTileService.kt:43-44`, `TunnelStateNotificationAction.kt:24`, `NavigationDrawerTv.kt:189`, `AndroidManifest.xml:116`.
- Fix: replace these PNGs with the Warren "W" mark (iOS `LogoIcon.imageset/logo-icon.svg` or Android `drawable/icon_android.xml` are ready references).

### P1, iOS app icon is a placeholder
`AppIcon.appiconset/Icon-Light-1024x1024.png` (and Dark/Tinted) is an unpolished cropped "WARREN" wordmark on black, off-center, text clipped at edges. Warren-branded but ships-looking-unfinished. Needs a designed 1024² icon (the "W" mark, matching `LogoIcon.imageset`).

### P1, Android primary color is Mullvad blue, Warren yellow unused as brand accent
`ColorDarkTokens.kt:29` `Primary = PaletteTokens.Blue`. The `Yellow` token is wired only to `ColorScheme.warning`. If the product intent is a yellow-accented brand (per `UIColor+Warren.swift` doc comment: "in sync with warrenbrowse.com … desktop accent"), Android does not reflect it. *However* this matches iOS and desktop, so confirm with product whether the dark-blue chrome is intentional before treating as a defect.

### P2, Misc
- iOS 2 WireGuard string leaks (fi/ru locales), `Localizable.xcstrings` L60074, L60134.
- Android dead Mullvad "disguise icon" assets (`Whack-a-mole`/`Ninja Mole` + mole mipmaps), remove or rebrand.
- iOS localization missing `ar`, `ro`, `fa` (21 vs 23 on desktop/Android).
- Internal token names (`MullvadWhite`, `UIFont.mullvad*`, `MullvadMaterial3Typography`), rebase-noise, not user-visible.

---

## What's already solid (no action)

- App name "Warren VPN" on all 3 platforms.
- User-visible strings: zero "Mullvad"/"WireGuard" leaks except the 2 iOS locale strings and the intentional iOS fork-attribution credit.
- "WireGuard" → "QUIC" rebrand via `strings.wireguard='QUIC'` (desktop) and equivalent.
- Support email `support@warrenbrowse.com`.
- Android launcher adaptive icon = Warren "W"; Android `logo_text.xml` = "WARREN" wordmark.
- iOS `LogoIcon` = Warren "W" gradient mark.
- Warren yellow brand value (#FFD524) defined identically on all 3 platforms.
- IA / navigation structure equivalent across platforms.
- Localization coverage ~equal (21-23 languages).
