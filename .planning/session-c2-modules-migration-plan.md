# Session C.2, Swift packages rebrand migration plan

Plan d'exécution pour la sous-phase C.2 du brief Session C iOS fork.
Référence : `.planning/session-c-ios-fork-brief.md` §C.2.

**Pré-conditions** :
- Session C.1 livrée (cf. `.planning/session-c-report.md`, commits `e7b1f89780` + `5e63567e2d` + bonus `d90954ca7c` Session D agent).
- `xcodebuild -list -project WarrenVPN.xcodeproj` PASS.
- Submodule `ios/wireguard-apple` retiré + stub local Package.swift.
- 2 schemes orphans `MullvadPostQuantum.xcscheme` + `MullvadPostQuantumTests.xcscheme` supprimés (commit cette session).

**Effort estimé** : 5-7 jours wall-clock.

---

## Inventaire modules iOS

| Module Mullvad | Target Warren | Importers | Files | Action C.2 |
|----------------|---------------|-----------|-------|------------|
| MullvadVPN | WarrenVPN | host app | ~hundreds | RENAME (target principal app) |
| MullvadREST | WarrenREST | 94 | 66 | RENAME |
| MullvadRESTTests | WarrenRESTTests | ~5 | ~15 | RENAME |
| MullvadSettings | WarrenSettings | 172 | 47 | RENAME |
| MullvadTypes | WarrenTypes | 305 | 52 | RENAME |
| MullvadLogging | WarrenLogging | 68 | 8 | RENAME |
| MullvadMockData | WarrenMockData | 24 | 14 | RENAME |
| MullvadRustRuntime | WarrenRustRuntime | 33 | 19 | RENAME (lien C.3 vers warren-ios crate) |
| MullvadRustRuntimeTests | WarrenRustRuntimeTests | ~3 | ~10 | RENAME |
| MullvadVPNTests | WarrenVPNTests | host tests | ~hundreds | RENAME |
| MullvadVPNUITests | WarrenVPNUITests | host UI tests | ~50 | RENAME |
| MullvadPostQuantum | DROP | 0 imports | n/a | DROP (couplé à C.4 PacketTunnelProvider Quinn replace WireGuardAdapter ; les 5 .swift PostQuantum-related restent in-place jusqu'à C.4) |
| Operations | (keep) | x | y | KEEP (nom générique, pas Mullvad-branded) |
| OperationsTests | (keep) | x | y | KEEP |
| PacketTunnel | (keep, rebrand contenu) | x | y | KEEP target name, rebrand content via C.4 |
| PacketTunnelCore | (keep, rebrand contenu) | x | y | KEEP target name, rebrand content via C.4 |
| PacketTunnelCoreTests | (keep) | x | y | KEEP |
| Routing | (keep) | x | y | KEEP (nom générique) |
| RoutingTests | (keep) | x | y | KEEP |

**Total à rebrand** : 11 modules. **Drop** : 1 module (PostQuantum, scheduled C.4). **Keep** : 7 modules.

---

## Ordre recommandé (leaf → app)

Le rebrand d'un module modifie les `import` chez tous ses importateurs. En commençant par les modules feuilles (peu d'importateurs, dépend de peu de choses), on minimise les cascades.

1. **MullvadRustRuntimeTests** (~3 importers, isolé), proof of pattern
2. **MullvadRESTTests** (~5 importers)
3. **MullvadVPNUITests** (~50 importers, UI tests isolated)
4. **MullvadMockData** (24 importers, dépend de MullvadTypes uniquement)
5. **MullvadLogging** (68 importers, dépend de MullvadTypes + swift-log)
6. **MullvadRustRuntime** (33 importers, lien C.3 vers warren-ios crate, peut être différé jusqu'à C.3 pour éviter double migration)
7. **MullvadREST** (94 importers)
8. **MullvadSettings** (172 importers)
9. **MullvadTypes** (305 importers, racine, en dernier)
10. **MullvadVPN** (host app, en dernier, après tous ses dependencies)
11. **MullvadVPNTests** (host tests, avec MullvadVPN)

---

## Checklist par module (template)

Pour chaque module `MullvadX` à rebrand en `WarrenX` :

### Préparation
- [ ] Lire `ios/MullvadX/*` pour cataloguer les types publics (renommage `MullvadFoo` → `WarrenFoo` si Warren-specific)
- [ ] Lister importateurs : `rg "^import MullvadX\b" ios --type swift`
- [ ] Identifier le PBXNativeTarget UUID + PBXGroup UUID + productReference UUID dans pbxproj

### Renommage fichiers
- [ ] `git mv ios/MullvadX ios/WarrenX`
- [ ] Si des `Sources/MullvadX/` sub-dirs : `git mv ios/WarrenX/Sources/MullvadX ios/WarrenX/Sources/WarrenX`

### pbxproj edits
- [ ] `name = MullvadX;` → `name = WarrenX;` (PBXNativeTarget)
- [ ] `productName = MullvadX;` → `productName = WarrenX;`
- [ ] `MullvadX.framework` → `WarrenX.framework` (productReference)
- [ ] `MullvadX.xctest` → `WarrenX.xctest` (pour Tests targets)
- [ ] `PRODUCT_BUNDLE_IDENTIFIER` paths : `$(APPLICATION_IDENTIFIER).MullvadX` → `$(APPLICATION_IDENTIFIER).WarrenX` ; hardcoded tests `com.warrenbrowse.vpn.ios.MullvadXTests` → `com.warrenbrowse.vpn.ios.WarrenXTests`
- [ ] PBXGroup `name = MullvadX` + `path = MullvadX` → `name = WarrenX` + `path = WarrenX`
- [ ] PBXFileSystemSynchronizedRootGroup si utilisé : idem
- [ ] Dependencies du target dans `dependencies = ( … )` listes des autres targets

### xcscheme edits
- [ ] `git mv ios/WarrenVPN.xcodeproj/xcshareddata/xcschemes/MullvadX.xcscheme WarrenX.xcscheme`
- [ ] Dans le scheme renommé : `BuildableName = "MullvadX.framework"` → `WarrenX.framework`
- [ ] `BlueprintName = "MullvadX"` → `WarrenX`
- [ ] `ReferencedContainer = "container:WarrenVPN.xcodeproj"` (déjà fait C.1)

### Source code edits
- [ ] sed batch : `import MullvadX` → `import WarrenX` dans tous les `.swift` files de tous les targets
- [ ] Types `MullvadFoo` Warren-specific : sed `MullvadFoo` → `WarrenFoo` (exception : types upstream Mullvad WG-legacy à garder)

### Validation
- [ ] `xcodebuild -list -project WarrenVPN.xcodeproj` PASS
- [ ] `xcodebuild build -scheme WarrenVPN -destination 'platform=iOS Simulator,name=iPhone 15' -configuration Debug CODE_SIGNING_ALLOWED=NO` PASS (au minimum compile errors absent)
- [ ] `grep -rn "MullvadX\|MullvadFoo" ios --type swift` = 0 résiduel

### Commit
- [ ] `git commit -m "refactor(ios): rename MullvadX → WarrenX (C.2)"` (un commit par module)

---

## Risques + mitigation

### Risque 1 : pbxproj corruption
**Cause** : pbxproj est un fichier semi-binaire avec UUIDs internes. sed batch peut casser la structure si un id partiel collisionne.

**Mitigation** :
- Tester chaque modification avec `xcodebuild -list` avant commit.
- Si Xcode est ouvert sur la machine dev, faire `File → Project Settings → Validate` après rename.

### Risque 2 : MullvadRustRuntime ↔ warren-ios crate (C.3 coupling)
**Cause** : Le rebrand de MullvadRustRuntime nécessite aussi le rebrand du header C (`MullvadIOS.h` → `WarrenIOS.h`) qui est généré par cbindgen depuis le crate `mullvad-ios`. Ce dernier est en scope C.3.

**Mitigation** :
- Différer le rebrand de MullvadRustRuntime jusqu'à C.3, pour migrer en une fois module Swift + crate Rust + header C.

### Risque 3 : Tests targets brisés temporairement
**Cause** : Pendant le rebrand inter-modules, les imports `import MullvadX` peuvent rester partiellement résolus.

**Mitigation** :
- Commits atomiques par module (rename + tous les imports updated en un commit).
- Vérifier xcodebuild après chaque commit.

### Risque 4 : Race condition avec autres agents (Session D Android, etc.)
**Cause** : cf. memory `feedback_parallel_agents_same_worktree`, `git add` capture les modifs d'autres agents sur le même working tree.

**Mitigation** :
- Lancer Session C.2 sur un worktree séparé : `git worktree add ../warren-app-ios-c2 main`.
- Ou exécuter séquentiellement (Session D terminée avant Session C.2 démarre).

---

## Hors scope C.2 (différé)

- **Drop MullvadPostQuantum 5 .swift files** (`PacketTunnelActor+PostQuantum.swift`, `MullvadPostQuantum+Stubs.swift`, `EphemeralPeerExchangingPipeline.swift`, `MultiHopEphemeralPeerExchanger.swift`, `SingleHopEphemeralPeerExchanger.swift`) → C.4 retirera ces fichiers naturellement lors du remplacement de WireGuardAdapter par WarrenQuinnAdapter, car ces classes orchestrent du PostQuantum WG-key-exchange qui n'a pas d'équivalent dans la stack Quinn + HPKE Warren.
- **WireGuardKit + WireGuardKitTypes dépendances** : stub local Package.swift maintenant ; suppression définitive des `WireGuardKitTypes in Frameworks` (~20 PBXBuildFile entries) dans pbxproj → C.4 également.
- **Rename `mullvad-ios` Rust crate → `warren-ios`** + génération `WarrenIOS.h` → C.3.

---

## Commencement

```bash
# Setup worktree dédié (recommandé pour éviter race conditions cross-sessions)
git worktree add ../warren-app-ios-c2 main
cd ../warren-app-ios-c2

# Suivre le checklist module-par-module dans l'ordre proposé.
# Module 1 : MullvadRustRuntimeTests (proof of pattern, ~30 min)
# Module 2 : MullvadRESTTests
# ...
```
