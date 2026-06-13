# Session C iOS fork, Follow-up brief outlines (C.3 deep + C.5 + C.6 + C.7)

Consolidated outline for the remaining sub-phases of Session C iOS fork.
Each section is intended to be expanded into a dedicated brief
(`.planning/session-c3deep-brief.md`, `.planning/session-c5-brief.md`, …)
when the corresponding sub-phase is scheduled.

**Pre-conditions for all follow-ups** :
- C.1 + C.2 + C.3 skeleton DONE (cf. `.planning/session-c-report.md`).
- C.4 design doc reviewed (cf. `.planning/c4-packet-tunnel-provider-quinn-design.md`).
- For follow-ups that touch warren-core : check pin
  (`Cargo.toml` workspace path-deps `../warren-core/crates/X`).

---

## Session C.3.deep, Replace mullvad-api with warren-api-client (3-5 days)

### Scope

Migrate the 16 source files in `warren-ios/src/api_client/` from the
`mullvad-api` crate to `warren-api-client` (warren-core).

### Files affected

| File | Mullvad APIs used | Warren replacement |
|------|-------------------|--------------------|
| `mod.rs` | `mullvad_api::access_method`, `EncryptedDnsProxyState` | drop access_method (Warren has M4.0 baseline), drop encrypted_dns_proxy (warren-api-client uses warren-tunnel transport) |
| `api.rs` | `mullvad_api::rest`, `proxy`, `Endpoint` | `warren_api_client::ApiClient` |
| `account.rs` | `mullvad_api::AccountsProxy`, `AccountNumber` | DROP (Warren auth = wallet, no account number) ; replace with `warren_identity::auth::sign_request` flow |
| `device.rs` | `mullvad_api::DevicesProxy` | DROP (Warren device model TBD) ; placeholder until Warren defines device-limit policy |
| `storekit.rs` | `mullvad_api::StoreKit` | KEEP or REPLACE with Warren subscription endpoint |
| `problem_report.rs` | `mullvad_api::ProblemReport` | OPTIONAL ; Warren telemetry minimal |
| `access_method_resolver.rs` | `mullvad_api::access_method`, `mullvad_encrypted_dns_proxy` | DROP |
| `access_method_settings.rs` | `mullvad_types::access_method` | DROP |
| `response.rs` | `mullvad_api::rest::Error` | `warren_api_client::Error` |
| `mock.rs` | `mockito` | KEEP (test infra) |
| `helpers.rs` | `shadowsocks::crypto` | DROP `available_ciphers` (Warren no Shadowsocks) |
| `shadowsocks_loader.rs` | `shadowsocks` | DROP entire file |
| `cancellation.rs` | std | KEEP |
| `completion.rs` | std | KEEP |
| `retry_strategy.rs` | std | KEEP |
| `swift_data.rs` | C ABI | KEEP, rename Mullvad-prefixed types |

### Tasks

1. Add `warren-api-client` + `warren-identity` path-deps to
   `warren-ios/Cargo.toml`
2. For each file, replace Mullvad imports with Warren equivalents (or DROP)
3. Sed `MullvadApiContext` -> `WarrenApiClient` cross codebase (Rust + Swift)
4. Drop `shadowsocks`, `mullvad-encrypted-dns-proxy`, `mullvad-types` deps from `warren-ios/Cargo.toml`
5. Drop `mullvad-api` dep
6. Update Swift wrappers in `ios/WarrenRustRuntime/Sources/WarrenRustRuntime/`
7. Run `cargo build --target aarch64-apple-ios` PASS
8. Regenerate `warren_rust_runtime.h` via cbindgen

### Effort estimate : 3-5 days

### Caveats

- Account number → wallet authentication is a **schema break**. Settings
  migration (in `MigrationManager`) must handle the conversion : detect
  legacy account number, prompt user to generate a Warren wallet, archive
  the account number for support purposes.
- StoreKit subscription IDs already aligned in C.1
  (`com.warrenbrowse.vpn.ios.subscription.storekit2.{30,90}days`).
- iOS device limit policy (max 5 devices in Mullvad) : Warren may not have
  this concept ; drop the DeviceChecker class entirely (cf. C.4 design doc).

### Dependencies

- warren-core HEAD with warren-api-client + warren-identity available
  (currently `ba819cf+`, OK)

---

## Session C.5, UI Swift wallet Ed25519 mnemonic auth (5-7 days)

### Scope

Replace the Mullvad account-number login + signup with a Warren wallet
flow (BIP39 mnemonic generation + import + Face ID gated backup).

### Files to create

| File | Lines | Purpose |
|------|-------|---------|
| `WarrenVPN/View controllers/Wallet/WarrenMnemonicInputView.swift` | ~150 | UITextField grid 12 words, BIP39 wordlist autocomplete, paste support |
| `WarrenVPN/View controllers/Wallet/WarrenMnemonicDisplayView.swift` | ~100 | UILabel grid 12 words, blur+reveal pattern, no copy button (anti-clipboard-malware) |
| `WarrenVPN/View controllers/Wallet/WarrenWalletKeychain.swift` | ~80 | `kSecClassGenericPassword` wrapper with `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` |
| `WarrenVPN/Coordinators/OnboardingWizardCoordinator.swift` | ~200 | 5-step flow : Welcome → Wallet (generate/import) → Subscription link → Privacy prefs → Done |
| `WarrenVPN/View controllers/Onboarding/WelcomeViewController.swift` | ~80 | Step 1 |
| `WarrenVPN/View controllers/Onboarding/WalletGenerateViewController.swift` | ~150 | Step 2a: generate 12-word display |
| `WarrenVPN/View controllers/Onboarding/WalletImportViewController.swift` | ~100 | Step 2b: import existing 12-word |
| `WarrenVPN/View controllers/Onboarding/SubscriptionLinkViewController.swift` | ~50 | Step 3: WebView link to warrenbrowse.com pricing |
| `WarrenVPN/View controllers/Onboarding/PrivacyPrefsViewController.swift` | ~120 | Step 4: multi-hop toggle, DAITA toggle, obfuscation indicator |
| `WarrenVPN/View controllers/Onboarding/OnboardingDoneViewController.swift` | ~50 | Step 5 |
| `WarrenVPN/View controllers/Settings/WalletBackupViewController.swift` | ~120 | Settings → View mnemonic (Face ID gated) |

Total : ~1200 new Swift lines.

### Files to modify

- `WarrenVPN/Coordinators/LoginCoordinator.swift` : add branch for
  wallet-restore flow
- `WarrenVPN/View controllers/Login/LoginViewController.swift` : add
  "Restore from mnemonic" CTA
- `WarrenVPN/AppDelegate.swift` : check Keychain for existing wallet on
  launch ; if absent, present `OnboardingWizardCoordinator`
- `WarrenVPN/Resources/Localizable.xcstrings` : add ~30 strings for
  FR + EN

### FFI integration

- Calls into `WarrenWallet` Swift wrapper (planned in C.4 / C.5
  intersection) which wraps `warren_wallet_ffi`
  (warren-ios crate)
- For C.5 skeleton without FFI : use placeholder
  `WarrenWallet.generateMnemonic()` returning hardcoded test phrase
  ; replace with FFI call once C.3 deep + C.4 wired

### Decisions

- **Local Authentication framework** for Face ID / Touch ID :
  `LAContext.evaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, …)`
  with `.deviceOwnerAuthentication` fallback (passcode)
- **No iCloud sync** for mnemonic : strictly device-local. Keychain class
  must NOT include `kSecAttrSynchronizable: true`
- **No copy button** in display view : reduces clipboard malware attack
  surface. Users must hand-write the 12 words during backup
- **Subscription** : link out to `warrenbrowse.com/pricing` in WebView
  (not in-app purchase initially ; StoreKit migration is a follow-up)
- **Min iOS 16** (cf. brief §C.1 decision) : supports all required APIs

### Tests

- Unit tests for `WarrenWalletKeychain` (Keychain round-trip via mock)
- UI tests for `OnboardingWizardCoordinator` (5-step flow via XCUITest)
- Mock `LAContext` for Face ID gating tests

### Effort estimate : 5-7 days

---

## Session C.6, Multi-hop + DAITA + NAT-PMP UI parity (5-7 days)

### Scope

Surface the Warren-specific privacy / connectivity features in the iOS
app (parity with desktop M4.H.C + Session B).

### Files to create

| File | Lines | Purpose |
|------|-------|---------|
| `WarrenVPN/View controllers/Settings/WarrenMultiHopSettingsViewController.swift` | ~200 | Toggle multi-hop, entry country picker, exit country picker (UITableView with relay list) |
| `WarrenVPN/View controllers/Settings/WarrenDaitaSettingsViewController.swift` | ~100 | DAITA toggle + tooltip "Overhead ~10%, OFF by default" (cf. memory `warren_daita_doctrine_v1`) |
| `WarrenVPN/View controllers/Settings/WarrenNatPmpSettingsViewController.swift` | ~150 | Port-forwarding toggle + display forwarded port + lifetime countdown |
| `WarrenVPN/View controllers/Tunnel/WarrenObfuscationIndicatorView.swift` | ~50 | Always-on banner in connection-details view: "HTTP/3 mimicry active" |
| `WarrenVPN/View controllers/Tunnel/WarrenFailoverBannerView.swift` | ~80 | "Switched to <country>" banner, dismissible, consuming App Group `WarrenTunnel.lastFailoverExit` |

Plus modifications to existing files :
- `WarrenVPN/View controllers/SelectLocation/*` : adapt for entry/exit pickers
- `WarrenVPN/Resources/Localizable.xcstrings` : i18n FR + EN

Total : ~600 new lines + ~200 modified.

### Data flow

```
App Group UserDefaults (written by PacketTunnelProvider in C.4):
  WarrenTunnel.lastFailoverExit -> WarrenFailoverBannerView
  WarrenTunnel.failoverCount -> WarrenFailoverBannerView
  WarrenTunnel.natPmpExternalPort -> WarrenNatPmpSettingsViewController
  WarrenTunnel.natPmpLifetime -> WarrenNatPmpSettingsViewController

Settings UserDefaults (read by SettingsReader in C.4):
  WarrenSettings.multiHopEnabled -> WarrenMultiHopSettingsViewController toggle
  WarrenSettings.multiHopEntryRelay -> entry country picker
  WarrenSettings.multiHopExitRelay -> exit country picker
  WarrenSettings.daitaEnabled -> WarrenDaitaSettingsViewController toggle
  WarrenSettings.natPmpEnabled -> WarrenNatPmpSettingsViewController toggle
```

### FFI integration

- Country picker uses `warren-relay-selector::WarrenRelayList` via FFI
  (planned `warren_multihop_ffi` exports a `list_relays(countryCode) -> [Relay]`)
- DAITA + multi-hop toggles propagate to `WarrenTunnelConfig` (cf. C.4)
- NAT-PMP events from `warren_natpmp_ffi` callbacks update App Group
  `WarrenTunnel.natPmpExternalPort` -> UI

### Decisions

- DAITA default **OFF** (per memory `warren_daita_doctrine_v1` and the
  bench finding `pump_*_with_daita` instable cross-DC sustained from
  Session F, UI must clearly indicate DAITA is experimental)
- Multi-hop default **OFF** (UX overhead, opt-in)
- Obfuscation **always-on** (M4.0 baseline, no toggle)
- NAT-PMP **OFF** by default (avoids unexpected port exposure)

### Tests

- UI tests for multi-hop toggle apply + reconnect (XCUITest)
- Snapshot tests for failover banner appearance / dismissal
- Mock NAT-PMP events for lifetime display tests

### Effort estimate : 5-7 days

---

## Session C.7, Build TestFlight + smoke iOS simulator (3-5 days)

### Scope

Produce a Warren-branded iOS .ipa archive, smoke-test on iOS Simulator,
and upload to TestFlight (invite-only beta).

### Tasks

1. **Build Release configuration** for iOS device + simulator
   - `xcodebuild archive -scheme WarrenVPN -configuration Release
      -archivePath build/WarrenVPN.xcarchive -destination 'generic/platform=iOS'`
   - Requires `DEVELOPMENT_TEAM` set in `Configurations/Base.xcconfig`
     (poka must provide actual Team ID, currently placeholder `XXXXXXXXXX`)

2. **iOS Simulator smoke tests** :
   - Launch app on iPhone 15 Simulator + iPad Pro Simulator
   - Onboarding wizard completes (generate wallet end-to-end)
   - Connect to warren-exit-1 prod
   - DNS leak test (curl ifconfig.me + dig ifconfig.me)
   - Multi-hop toggle + reconnect
   - DAITA toggle apply
   - NAT-PMP (skip if qBittorrent simulator unavailable)
   - Disconnect + reconnect cycle
   - Force-kill app -> verify killswitch blocks traffic

3. **App Store metadata** (App Store Connect API or fastlane) :
   - Title : "Warren VPN"
   - Subtitle FR : "VPN décentralisé Warren"
   - Subtitle EN : "Warren decentralized VPN"
   - Description (~1500 chars FR + EN, mirror warrenbrowse.com landing)
   - Keywords (~100 chars per locale)
   - Screenshots : iPhone 6.7" + 5.5", iPad 12.9" (10 screens each)
     - Generate via fastlane snapshot, automated XCUITest navigation
   - Privacy nutrition label :
     - Data collected : IP address (transient, tunnel routing only)
     - Not collected : usage analytics, identifiers, contacts, location

4. **TestFlight invite-only group setup** (App Store Connect console)
   - Beta App Description
   - Test Information (what to test)
   - Internal testers : poka + Warren team
   - External testers : invite via email (later, post stable)

5. **Upload .ipa** :
   - `xcrun altool --upload-app -f WarrenVPN.ipa
      --apiKey <APIKEY> --apiIssuer <ISSUER>` (requires App Store Connect API key, escalade poka)
   - Wait for Apple processing (~15-30 min)
   - Verify in TestFlight that build is "Ready to Test"

### Decisions

- **Without Apple Developer signing** (current state) : build PASS suffit
  pour GO PARTIEL C.7 ; TestFlight upload skip with caveat for poka to
  generate signing cert + provisioning profile.
- **Screenshots automation** : use fastlane snapshot from start ; one-time
  setup ~2h saves recurring work on every release.
- **Apple Developer account** required : escalate to poka if not already
  provisioned. Cost : $99 USD/year.

### Effort estimate : 3-5 days

### Caveats

- TestFlight upload skip if signing pending poka → not a blocker for GO
  PARTIEL C.7
- iOS device smoke skip if no iPhone available → simulator-only smoke
  acceptable

---

## Dependency graph (suggested sequencing)

```
C.3.deep  ─┬─> C.4 ──┬─> C.5  ─> C.6  ─> C.7
           │         │
           └─────────┘  (C.4 calls warren_api_client via Swift wrappers built
                         in C.3.deep ; can also be done concurrently if
                         WarrenApiClient Swift wrapper is stubbed)
```

**Critical path** : C.4 → C.5 → C.6 → C.7. C.3.deep can run in parallel
with C.4 since it touches the Rust side (api_client/) while C.4 touches
mostly Swift + new FFI modules.

**Total wall-clock estimate** : 16-29 days (C.3.deep 3-5 + C.4 10-14 +
C.5 5-7 + C.6 5-7 + C.7 3-5, with C.3.deep parallel to C.4 ⇒ effectively
13-26 days serial).

---

## References

- `.planning/session-c-ios-fork-brief.md`, original Session C brief
- `.planning/session-c-report.md`, Session C C.1+C.2+C.3-skeleton report
- `.planning/c4-packet-tunnel-provider-quinn-design.md`, C.4 detailed design
- `.planning/session-c2-modules-migration-plan.md`, C.2 module rebrand plan
- Memory `warren_session_c_c1_c2_c3_skeleton_delivered`, context of this session
- Memory `warren_session_b_delivered`, desktop wizard pattern (5-step)
- Memory `warren_daita_doctrine_v1`, DAITA OFF by default
- Memory `warren_session_f_delivered`, `pump_*_with_daita` instability finding (warren-core M5.B.1.X)
