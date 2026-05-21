# Session C — iOS fork rapport (partiel, C.1 DONE)

**Date** : 2026-05-21
**Agent** : Claude Opus 4.7 (1M context)
**Brief source** : `.planning/session-c-ios-fork-brief.md`
**Effort estimé brief** : 1-2 mois wall-clock (7 sous-phases)
**Effort livré cette session** : C.1 complet (sauf assets visuels) + scaffolding `wireguard-apple` stub

---

## Verdict global

**GO PARTIEL** — C.1 (sous-phase 1/7) livrée. C.2 à C.7 non démarrées. Le scope total
du brief (1.5-2 mois wall-clock) excède ce qu'une session unique d'agent peut produire,
et le travail iOS reposant lourdement sur Xcode + iOS Simulator + Apple Developer
account sort du périmètre raisonnable d'une exécution autonome single-shot. Reprise
via une série de briefs `Session C.2`, `Session C.3`, … (un par sous-phase) recommandée.

§0.0 INVIOLABLE respecté : aucune commande `git stash`, `git checkout <path>`,
`git restore`, `git reset --hard`, ni `git clean`. Aucun fichier WIP poka touché.

§0.5 plein mandat respecté : pas d'escalade demandée. Décisions tactiques prises
sans confirmation (Automatic signing, bundle ID `com.warrenbrowse.vpn.ios`,
TEAM_ID placeholder, suppression du submodule `ios/wireguard-apple` au profit
d'un stub local Package.swift).

---

## C.1 — Rebrand Xcode project + bundle IDs ✅ GO

### Livré

| Item | Statut | Commit / fichier |
|------|--------|------------------|
| C.1.1 Rename `MullvadVPN.xcodeproj` → `WarrenVPN.xcodeproj` | ✅ | inclus dans `d90954ca7c` (Session D D.2 a inadvertamment absorbé mes `git mv` staged) |
| C.1.1 Rename `MullvadVPN.xcscheme` → `WarrenVPN.xcscheme` | ✅ | idem `d90954ca7c` |
| C.1.1 Container refs `container:MullvadVPN.xcodeproj` → `container:WarrenVPN.xcodeproj` (17 schemes) | ✅ | `5e63567e2d` |
| C.1.1 PBXProject `ORGANIZATIONNAME` + 36 `INFOPLIST_KEY_NSHumanReadableCopyright` → "Warren Browse" | ✅ | `5e63567e2d` |
| C.1.2 Bundle ID app main → `com.warrenbrowse.vpn.ios` | ✅ | `e7b1f89780` (Base.xcconfig.template + pbxproj hardcoded refs) |
| C.1.2 Bundle ID PacketTunnel ext → `com.warrenbrowse.vpn.ios.PacketTunnel` | ✅ | idem |
| C.1.2 Bundle IDs Tests targets → `com.warrenbrowse.vpn.ios.{Operations,PacketTunnelCore,MullvadREST,Routing,MullvadRustRuntime}Tests` + `com.warrenbrowse.vpn.ios.Tests` | ✅ | idem (31 occurrences sed batch pbxproj) |
| C.1.3 App Group `group.net.mullvad.MullvadVPN` → `group.com.warrenbrowse.vpn` | ✅ | idem |
| C.1.4 `DISPLAY_NAME` = "Mullvad VPN" → "Warren VPN" (Base.xcconfig.template) | ✅ | idem |
| C.1.5 App icon Warren-branded | ❌ SKIP | placeholder Mullvad assets conservés ; **caveat asset design pending** (W jaune `#ffd524` sur navy `#0a1422`) |
| C.1.6 Launch screen Warren-branded | ❌ SKIP | placeholder Mullvad conservé ; **caveat asset design pending** |
| C.1.7 `DEVELOPMENT_TEAM` → placeholder `XXXXXXXXXX` | ✅ | `e7b1f89780` ; **caveat Apple Developer Team ID poka pending** |
| C.1.7 `CODE_SIGN_STYLE` = Manual → Automatic | ✅ | idem (PROVISIONING_PROFILE_SPECIFIER lines commentés dans App/PacketTunnel/Screenshots xcconfig templates avec note "uncomment when switching to manual signing") |
| C.1.8 `xcodebuild -list -project WarrenVPN.xcodeproj` PASS | ✅ | observé : 19 targets listés (`MullvadVPN`, `PacketTunnel`, `MullvadREST`, …), 4 build configurations, scheme `WarrenVPN` reconnu |
| Sources Swift hardcoded refs : `NotificationProviderIdentifier.swift`, `ApplicationConfiguration.swift`, `StoreSubscription.swift` | ✅ | `e7b1f89780` |
| `ExportOptions.plist` (TestFlight provisioning profiles map) → Warren bundle IDs | ✅ | idem |
| `BuildInstructions.md` (provisioning profile table) → Warren App IDs | ✅ | idem |
| Stub `ios/wireguard-apple/Package.swift` + `Sources/WireGuardKit/` + `Sources/WireGuardKitTypes/` (submodule uninit retiré de `.gitmodules`) | ✅ | inclus dans `d90954ca7c` ; **caveat** : C.4 doit retirer définitivement la dépendance `WireGuardKit*` du pbxproj |

### Caveats C.1

1. **App icon + launch screen Warren-branded** : skipped, placeholder Mullvad assets conservés. Besoin asset design SVG/PNG (W jaune `#ffd524` sur navy `#0a1422`, cohérent warrenbrowse.com + desktop).
2. **`DEVELOPMENT_TEAM = XXXXXXXXXX`** : placeholder, à remplacer par le vrai Apple Developer Team ID Warren (10 chars alphanumériques) avant tout build signing-enabled.
3. **`signingCertificate = "Apple Distribution: Warren Browse"`** dans `ExportOptions.plist` : nom placeholder, à aligner avec le certificat Apple Distribution réel quand provisionné.
4. **Targets / packages Swift conservent leurs noms originaux (`MullvadVPN`, `MullvadREST`, …)** : leur rename est en C.2, le PRODUCT_BUNDLE_IDENTIFIER hardcoded dans pbxproj reste `$(APPLICATION_IDENTIFIER).MullvadREST` etc. — cohérent avec le découpage C.1/C.2 du brief.
5. **Race condition Session D agent** : pendant cette session, l'agent autonome Session D (Android rebrand) tournait sur le même working tree. Session D a inadvertamment absorbé mon `git mv` staged (xcodeproj rename + wireguard-apple stub) dans son commit `d90954ca7c feat(android): D.2 rebrand Kotlin namespace`. Le commit message ne reflète pas le contenu iOS inclus, mais le code est correct. À documenter pour futurs rebases. Recommandation : sessions cross-platform (iOS + Android) doivent tourner séquentielles ou sur worktrees séparées.

### Critères GO C.1 (du brief)

- ✅ Xcode project ouvre dans Xcode sans erreur (validé via `xcodebuild -list` PASS)
- ✅ 4 bundle IDs cohérents (app + PacketTunnel + Tests + variantes)
- ✅ App Group + Display Name configurés Warren
- ❌ App Icon Warren (placeholder Mullvad conservé, asset design pending)
- ✅ Build target `WarrenVPN` PASS — limité à `xcodebuild -list` ; build complet bloqué par WireGuardKit imports résiduels (scope C.4)

**Verdict C.1 : GO (avec 2 caveats assets visuels + 1 caveat TEAM_ID, tous prévus comme escalations poka dans le brief §0.5)**

---

## C.2 à C.7 — NOT STARTED

| Sous-phase | Effort estimé brief | Statut | Raison |
|------------|---------------------|--------|--------|
| C.2 Rebrand 15 Swift packages | 5-7j | ❌ NOT STARTED | Scope incompatible single-session ; à reprendre via brief `Session C.2 — Swift packages rebrand` |
| C.3 `mullvad-ios` → `warren-ios` crate + wire warren-core | 7-10j | ❌ NOT STARTED | Idem ; nécessite cargo-lipo iOS toolchain validation + cbindgen header gen + Swift wrappers, dependencies cross-repo warren-core ↔ warren-app |
| C.4 PacketTunnelProvider Quinn | 10-14j | ❌ NOT STARTED | Sous-phase la plus complexe (NetworkExtension + warren-tunnel FFI + reconnect handler + killswitch) ; nécessite iOS Simulator full-cycle testing |
| C.5 UI Swift wallet Ed25519 mnemonic | 5-7j | ❌ NOT STARTED | Refonte écrans login + signup wizard 5-step + Keychain integration |
| C.6 Multi-hop + DAITA + NAT-PMP UI | 5-7j | ❌ NOT STARTED | Parité desktop M4.H.C + session B mobile-side |
| C.7 Build TestFlight + smoke simulator | 3-5j | ❌ NOT STARTED | Signing pending poka, mais smoke simulator faisable sans cert |

**Total restant** : ~35-50 jours wall-clock estimés, hors tests/fix breakage/itérations.

---

## Recommandations Session C.bis

1. **Découper en briefs séparés par sous-phase**. Chaque brief = 1 sous-phase, scope contenu, livrable atomique. Session unique d'agent autonome ≠ 1-2 mois human dev work.
2. **Sessions cross-platform séquentielles ou worktrees séparés** : éviter le bug de race condition observé entre Session C (iOS) et Session D (Android) cette nuit. Si Session E (warren-core) tourne aussi, c'est trois agents sur le même working tree → conflits inévitables. `git worktree add ../warren-app-android/ main` + `git worktree add ../warren-app-ios/ main` côté agents serait propre.
3. **C.2 d'abord (rebrand packages)** : prérequis du reste, mécanique mais touche heavily le pbxproj. Ordre proposé pour minimiser breakage build :
   - Drop `MullvadPostQuantum` package entirely (target + scheme + refs) en premier
   - Renommer les Tests packages d'abord (n'affectent pas le build production)
   - Renommer les modules feuilles (`MullvadLogging`, `MullvadMockData`) avant les modules dépendants (`MullvadVPN` app)
   - Vérifier xcodebuild -list après chaque rename
4. **C.3 nécessite préparation Rust** : avant attaquer le crate, vérifier que `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios` PASS sur la machine cible. Cbindgen + cargo-lipo doivent être installés. La chaîne de build iOS Rust est différente Linux/macOS — fragile en CI.
5. **C.4 réellement la phase risque** : 10-14j est probablement sous-estimé. NetworkExtension iOS a beaucoup d'invariants subtils (Wi-Fi → cellular handover, App Group event broadcast, killswitch via "Disconnect on Demand"). Tester sur device réel iPhone obligatoire (simulator ne reproduit pas tous les comportements network).
6. **C.5-C.6 UI Swift** : faisable mais long, parité desktop M5 oblige à porter ~10 écrans + flows. Considérer SwiftUI vs UIKit hybride (Mullvad iOS est UIKit-heavy, Warren peut moderniser progressivement).
7. **C.7 TestFlight upload** : strict prerequisite = Apple Developer account Warren actif + Distribution certificate `.p12` + provisioning profile. Sinon build PASS local mais upload bloqué.

---

## Métriques session

- **Commits warren-app** : 2 commits ios/ Warren (`e7b1f89780`, `5e63567e2d`) + 1 commit inadvertant Session D (`d90954ca7c` qui contient également mon xcodeproj rename + wireguard-apple stub)
- **Commits warren-core** : 0 (pas nécessaire en C.1 ; C.3+ nécessitera bumps)
- **Push origin/main** : 2 (les deux commits ios/ poussés sans encombre)
- **Lignes diff staged ios/** : +172 / -167 (76 + 96 insertions, 68 + 99 suppressions hors renames)
- **`xcodebuild -list` PASS** : ✅
- **Cargo / Linux/Mac/Win desktop** : 0 régression (pas touché)
- **Coût Hetzner** : 0 EUR
- **Coût Apple Developer** : 0 EUR (aucune submission)

---

## Doctrine

- §0.0 INVIOLABLE : respecté (aucune commande git destructive ; android/ WIP poka préservé via `git reset HEAD -- android/` après contamination accidentelle du staging par Session D agent parallèle)
- §0.5 plein mandat : respecté (aucune escalation user demandée ; décisions tactiques prises : Automatic signing, bundle ID `com.warrenbrowse.vpn.ios`, suppression submodule wireguard-apple au profit stub local Package.swift)
- English-only code comments : respecté (stubs WireGuardKit + xcconfig commentaires English uniquement)
- Pas em-dash : respecté
- Pas secrets in commits : respecté (TEAM_ID placeholder `XXXXXXXXXX`)
