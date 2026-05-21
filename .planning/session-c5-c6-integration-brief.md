# Session C.5 / C.6 — Integration brief (ApplicationCoordinator + Settings)

Wire-in steps for the production-ready Warren wallet flow + Warren settings
views into the existing iOS app shell. The components themselves are already
delivered (cf. `.planning/session-c-report.md` C.5/C.6 sections) ; this brief
documents the small surgical edits needed in `ApplicationCoordinator.swift`
+ `SettingsViewControllerFactory.swift` + `AppPreferences.swift` to make the
flow actually invoked at runtime.

**Effort** : 1-2 days wall-clock.
**Risk** : Medium (touches the boot path of the app ; QA on simulator
required after each edit).

---

## 1. Add `hasCompletedWarrenOnboarding` to `AppPreferences`

`ios/WarrenVPN/Storage/AppPreferences.swift` (or wherever `AppPreferencesDataSource`
lives — search for `isShownOnboarding` to find the file).

Add the flag mirroring the existing `isShownOnboarding`/`isAgreedToTermsOfService`
pattern :

```swift
@AppStorageKey(key: "AppPreferences.hasCompletedWarrenOnboarding", defaultValue: false)
var hasCompletedWarrenOnboarding: Bool
```

Reset on logout (search for `isShownOnboarding = false` to find the reset
site) :

```swift
hasCompletedWarrenOnboarding = false
```

Rationale : a fresh wallet should not bypass the wizard.

---

## 2. Add `AppRoute.warrenOnboarding` case

`ios/WarrenVPN/Routing/AppRoute.swift` (search for `case tos` to find the enum).

```swift
case warrenOnboarding
```

Add to the route group switch (search `case .tos:` for the modal grouping
attribute — `warrenOnboarding` should be in the same `.primary` group as
`.tos`).

---

## 3. Wire `evaluateNextRoutes()` in `ApplicationCoordinator.swift`

`ios/WarrenVPN/Coordinators/ApplicationCoordinator.swift` — `evaluateNextRoutes()`
(line ~339 in the post-C.2 file).

**Before** (after C.2 rebrand) :

```swift
private func evaluateNextRoutes() -> [AppRoute] {
    guard appPreferences.isAgreedToTermsOfService else {
        return [.tos]
    }

    var routes = [AppRoute]()
    // ...
}
```

**After** :

```swift
private func evaluateNextRoutes() -> [AppRoute] {
    guard appPreferences.isAgreedToTermsOfService else {
        return [.tos]
    }
    // Warren onboarding takes precedence over Mullvad login UX so the
    // wallet exists before any tunnel attempt.
    guard appPreferences.hasCompletedWarrenOnboarding else {
        return [.warrenOnboarding]
    }

    var routes = [AppRoute]()
    // ... (unchanged below)
}
```

---

## 4. Add the presenter

`ApplicationCoordinator.swift` — add next to `presentTOS(animated:completion:)`
(line ~382) :

```swift
private func presentWarrenOnboarding(
    animated: Bool,
    completion: @escaping (Coordinator) -> Void
) {
    let coordinator = OnboardingWizardCoordinator()
    coordinator.delegate = self

    addChild(coordinator)
    coordinator.start(animated: false)

    presentChild(
        coordinator,
        animated: animated,
        configuration: ModalPresentationConfiguration(
            preferredContentSize: navigationContainer.preferredContentSize,
            modalPresentationStyle: .fullScreen
        )
    )

    completion(coordinator)
}
```

Register the route in the `router.handler` switch (search for `.tos` to find
the route-presenter dispatch) :

```swift
case .warrenOnboarding:
    self.presentWarrenOnboarding(animated: $0, completion: $1)
```

---

## 5. Conform `ApplicationCoordinator` to `OnboardingWizardCoordinatorDelegate`

Same file, add an extension at the bottom :

```swift
extension ApplicationCoordinator: @preconcurrency OnboardingWizardCoordinatorDelegate {
    func onboardingWizardDidFinish(_ coordinator: OnboardingWizardCoordinator) {
        appPreferences.hasCompletedWarrenOnboarding = true
        coordinator.removeFromParent()
        router.dismiss(.warrenOnboarding, animated: true)
        continueFlow(animated: true)
    }
}
```

---

## 6. Wire Settings entries for Warren features

`ios/WarrenVPN/Coordinators/Settings/SettingsViewControllerFactory.swift`
(or `SettingsDataSource.swift` depending on which owns the row model).

Add to the Settings table sections :

### Section : "Wallet"

| Row | Action |
|-----|--------|
| "View recovery phrase" | Push `WarrenWalletBackupViewController(interactor:)` |

### Section : "Privacy" (or in the existing privacy/multi-hop section)

| Row | Action |
|-----|--------|
| "Multi-hop" | Push `UIHostingController(rootView: WarrenMultiHopSettingsView())` |
| "DAITA" | Push `UIHostingController(rootView: WarrenDaitaSettingsView())` |
| "Port forwarding" | Push `UIHostingController(rootView: WarrenNatPmpSettingsView())` |

Each row's tap handler instantiates the appropriate UIHostingController and
pushes onto the Settings navigation controller. Pattern (from
`SettingsViewControllerFactory.swift`) :

```swift
case .warrenWalletBackup:
    let controller = WarrenWalletBackupViewController(interactor: WarrenWalletInteractor())
    controller.delegate = self
    navigationController.pushViewController(controller, animated: animated)

case .warrenMultiHop:
    let controller = UIHostingController(rootView: WarrenMultiHopSettingsView())
    controller.title = String(localized: "Multi-hop", table: "Settings")
    navigationController.pushViewController(controller, animated: animated)

// (similar for .warrenDaita + .warrenPortForwarding)
```

Add the corresponding cases to the `SettingsRoute` (or equivalent) enum.

---

## 7. Wire the Failover banner

`ios/WarrenVPN/View controllers/Tunnel/TunnelViewController.swift` (or the
connection-details parent) — observe App Group UserDefaults keys written
by `PacketTunnelProvider.broadcastEvent` (cf.
`.planning/c4-packet-tunnel-provider-quinn-design.md` §2.3) :

```swift
private func subscribeToFailoverEvents() {
    let defaults = UserDefaults(suiteName: ApplicationConfiguration.securityGroupIdentifier)
    NotificationCenter.default.addObserver(
        forName: UserDefaults.didChangeNotification,
        object: defaults,
        queue: .main
    ) { [weak self] _ in
        guard let self else { return }
        if let country = defaults?.string(forKey: "WarrenTunnel.lastFailoverExit"),
           let date = defaults?.object(forKey: "WarrenTunnel.lastFailoverAt") as? Date,
           Date().timeIntervalSince(date) < 30 {
            self.showFailoverBanner(country: country, occurredAt: date)
        }
    }
}

private func showFailoverBanner(country: String, occurredAt: Date) {
    let banner = WarrenFailoverBannerView(
        info: WarrenFailoverBannerInfo(country: country, occurredAt: occurredAt),
        onDismiss: { [weak self] in self?.hideFailoverBanner() }
    )
    // Embed via UIHostingController + autolayout above the connection-details
    // panel. Auto-hide after 10 seconds.
}
```

---

## 8. Wire the Obfuscation indicator

`ios/WarrenVPN/View controllers/Tunnel/ConnectionView/ConnectionView.swift`
(SwiftUI Mullvad ConnectionView).

Add at the bottom of the relay info section :

```swift
WarrenObfuscationIndicatorView()
    .padding(.top, 12)
```

---

## 9. Verification checklist

After each edit :

1. `xcodebuild -list -project WarrenVPN.xcodeproj` PASS (project structure intact)
2. `xcodebuild build -target WarrenVPN -destination 'generic/platform=iOS Simulator' CODE_SIGNING_ALLOWED=NO` PASS (drop `WireGuardKit*` framework refs first if not done in C.4)
3. iOS Simulator manual test :
   - Fresh install → terms → onboarding wizard appears → walk through all 5 steps → wallet generated + stored in Keychain → main app shows
   - Settings → Wallet → View recovery phrase → Face ID prompt → backup view
   - Settings → Privacy → Multi-hop / DAITA / Port forwarding entries visible

---

## 10. Out of scope (still in follow-up briefs)

- C.3 deep step 2 : api_client mullvad → warren rewrite (cf. `.planning/session-c-followup-briefs.md`)
- C.4 implementation : FFI Rust bodies + NEPacketTunnelFlow bridge + drop WireGuardKit
- C.7 TestFlight upload : Apple Developer signing pending poka

---

## 11. References

- `.planning/session-c-report.md` — C.5/C.6 components inventory
- `.planning/c4-packet-tunnel-provider-quinn-design.md` — App Group event keys
- `.planning/session-c-followup-briefs.md` — remaining sub-phases
- Memory `warren_session_c_c1_delivered` — full session context
- Memory `warren_session_h_delivered` — analogous Electron WarrenPubKeyWarning wire-in pattern
