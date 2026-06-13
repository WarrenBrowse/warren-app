# Session iOS-Cleanup, 434 strings i18n + PacketTunnelActor + SVG logo

> Brief d'agent autonome warren-app iOS surface.
> Doctrine §0.0 INVIOLABLE + §0.5 full autonomy + §0.6 worktree séparé obligatoire.
> Session iOS finitions post-Session-C continuation : caveats restants pour TestFlight ready.

**Effort estimé** : wall-clock 5-7 jours.
**Coût Hetzner** : 0 EUR.
**Pré-conditions** :
- warren-app `main` HEAD `eced6c8613+`
- Session C continuation phase 7 livrée (build green + simulator smoke OK)
- macOS dev machine avec Xcode 16+

**Objectif** : finir les caveats iOS de Session C continuation (mentionnés memory `warren_session_c_continuation_phase7`) : 434 Mullvad strings dans Localizable.xcstrings (20+ langs) bulk-replace, full SVG Warren logo paths, C.4.5 Warren-native PacketTunnelActor (au-delà compile-green pour live tunnel).

Sous-phases (séquentielles autonomes) :

1. **iOS-Cleanup.1, Setup worktree** (~30 min)
2. **iOS-Cleanup.2, Localizable.xcstrings bulk-replace 434 strings (FR+EN focus, autres langs deferred)** (~2-3j)
3. **iOS-Cleanup.3, Full SVG Warren logo paths (header + launch + AppIcon)** (~1j)
4. **iOS-Cleanup.4, C.4.5 Warren-native PacketTunnelActor live tunnel** (~3-4j)
5. **iOS-Cleanup.5, Smoke iOS simulator E2E live tunnel** (~0.5-1j)
6. **iOS-Cleanup.6, Rapport + cleanup** (~0.5j)

---

## 0.0 INVIOLABLE, pas de commande git destructive

Cf. doctrine standard.

---

## 0.5 MANDAT D'AUTONOMIE

Plein mandat. Escalade SEULEMENT si :
1. Secret leak
2. Coût > 0.30 EUR (n/a)
3. Breaking change /v1 wire format (improbable iOS UI)
4. Signing key prod touchée (escalade si C.4.5 nécessite cert iOS Developer)
5. **Spécifique iOS-Cleanup** : si PacketTunnelActor live tunnel demande TestFlight signing pour test réel (vs simulator), escalader

---

## 0.6 WORKTREE SÉPARÉ OBLIGATOIRE

```bash
cd /Users/poka/dev/warrenBros/warren-app
git worktree add ../warren-app-ios-cleanup main
cd ../warren-app-ios-cleanup
```

Cleanup en fin :
```bash
git worktree remove ../warren-app-ios-cleanup
```

---

## 1. Setup initial

```bash
cd /Users/poka/dev/warrenBros/warren-app
git worktree add ../warren-app-ios-cleanup main
cd ../warren-app-ios-cleanup

# Lire memory C continuation
cat /Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-app/memory/warren_session_c_continuation_phase7.md
cat /Users/poka/.claude/projects/-Users-poka-dev-warrenBros-warren-app/memory/warren_session_c_wireup_continuation.md

# Inventaire xcstrings + assets
find ios -name 'Localizable.xcstrings' -exec wc -l {} \;
ls ios/MullvadVPN/Assets/LogoText.imageset/
ls ios/MullvadVPN/Assets/AppIcon.appiconset/
```

---

## 2. iOS-Cleanup.2, Localizable.xcstrings bulk-replace 434 strings (~2-3j)

### Scope

1. Xcode .xcstrings format = JSON sous-pattern key + localizations per-lang
2. Identifier les 434 keys avec value contenant `Mullvad` ou `mullvad` (lecture parse)
3. Décision tactique : remplacer dans FR+EN d'abord (priorité produit), autres langs (DE/ES/IT/JA/etc.) → automatique si pattern identique ou bulk script
4. Pattern remplacement :
   - `Mullvad VPN` → `Warren VPN`
   - `Mullvad account` → `Warren account` (ou wallet selon contexte)
   - `mullvad.net` → `warrenbrowse.com`
   - URLs `mullvad.net/.../...` → `warrenbrowse.com/.../...`
   - Account number references → mnemonic wallet references (cf. Session B.3 onboarding desktop pattern)
5. Validation post-bulk : `xcrun --find xcstool` ou script Python parse + assert no `Mullvad` residual FR+EN

### Critères GO

- 434 strings audited
- FR+EN bulk-replaced cohérents
- Autres langs : automatique si même structure clé OR deferred next phase i18n
- App relance Simulator + capture screenshot validates user-visible strings = "Warren"

### Décisions tactiques

- FR+EN priorité, autres langs (~18 langs Mullvad) → bulk script sed-like, validation sur sample (DE+ES+JA), reste accept "best-effort"
- Si strings ambiguous (Account number vs Mnemonic mapping) : adapter au contexte Warren (wallet flow) ou conserver Account si neutre

---

## 3. iOS-Cleanup.3, Full SVG Warren logo paths (~1j)

### Scope

1. **LogoText.imageset** : actuel = SVG text placeholder (Session C7 note "UIKit doesn't rasterize <text>", header montre icon only)
2. Remplacer par SVG vectoriel propre :
   - Logo Warren = "WARREN" wordmark + W badge yellow `#ffd524` sur navy `#0a1422`
   - Format SVG path-based (pas text element, compatible UIKit raster)
   - Sources : warrenbrowse.com `src/components/Logo.astro` (référence) ou regen via Adobe Illustrator / Inkscape
3. **AppIcon.appiconset** : tous les size variants (20pt @2x/@3x, 29@2x/@3x, 40@2x/@3x, 60@2x/@3x, 76@1x/@2x, 83.5@2x, 1024 marketing)
4. **Launch screen** : Warren wordmark + tagline "Privacy without compromise"
5. Test simulator launch : header rendu correct + AppIcon visible Springboard

### Critères GO

- LogoText.imageset PNG/SVG vectoriel propre
- AppIcon variants tous renseignés
- Launch screen Warren-branded
- Simulator screenshot validates UI

### Décisions tactiques

- Format final PNG vs SVG : SVG vectoriel préféré (scaling), fallback PNG si UIKit raster issue
- 1024 marketing pour App Store : exporté SVG → PNG haute résolution

---

## 4. iOS-Cleanup.4, C.4.5 Warren-native PacketTunnelActor live tunnel (~3-4j)

### Scope

Session C continuation phase 6 a livré PacketTunnelProvider stub (compile-green), pas le tunnel live. Cette phase finit le wiring.

1. Étudier `ios/PacketTunnel/PacketTunnelProvider/` :
   - WireGuardAdapter pattern Mullvad (référence)
   - PacketTunnelActor protocol (Mullvad split actor / coordinator)
2. Implémenter `WarrenQuinnPacketTunnelActor` :
   - Spawn warren-tunnel Quinn endpoint via `WarrenRustRuntime` FFI
   - NEPacketTunnelFlow read/write loop
   - Network change observer (NWPathMonitor) → trigger reconnect Backoff::HANDSHAKE 15s
   - Killswitch via NetworkExtension auto-block
3. Configure NEVPNProtocolWarren custom subclass :
   - serverAddress = exit IP:port
   - providerConfiguration JSON = WarrenTunnelConfig (exit pubkey + relay config + wallet signing key from Keychain + multi-hop + DAITA + NAT-PMP)
4. Wire activation depuis App main :
   - User tap "Connect" → load WarrenTunnelConfig from settings + Keychain → `NEVPNManager.shared().setEnabled(true)` → `connection.startVPNTunnel()`
5. Tests Swift `PacketTunnelTests` :
   - Init/start/stop sans crash
   - Network change handler triggers reconnect
6. Tests Rust FFI warren-ios : echo packet round-trip via Quinn

### Critères GO

- WarrenQuinnPacketTunnelActor Swift implémenté
- Live tunnel iOS Simulator vers warren-exit-1 prod : connect + ping ifconfig.me retourne IP exit
- Disconnect propre
- Network change handover

### Décisions tactiques

- Multi-hop iOS : utiliser MultiHopClient via warren-multihop FFI
- DAITA iOS : si Session G fix wire activé client warren-ios, OK
- NAT-PMP iOS : optional v1 (peut être feature-flagged off pour bêta initiale)

---

## 5. iOS-Cleanup.5, Smoke iOS simulator E2E live tunnel (~0.5-1j)

### Scope

1. Build Release iOS simulator
2. Smoke tests :
   - Connect warren-exit-1 prod via UI
   - `curl ifconfig.me` retourne IP exit (depuis app WebView ou Safari simulator)
   - Multi-hop toggle apply
   - DAITA toggle apply (visible logs DAITA actif)
   - Disconnect propre
   - Re-connect after network change (simulate Wi-Fi → cellular via Settings simulator)
3. Capture screenshots final pour App Store metadata + RELEASE notes

### Critères GO

- 5+ smoke tests PASS
- Screenshots iPhone 6.7" + 5.5" capturés

---

## 6. iOS-Cleanup.6, Rapport + cleanup (~0.5j)

### Scope

- Rapport `.planning/session-ios-cleanup-report.md`
- Memory `warren_session_ios_cleanup_delivered.md` warren-app
- Update MEMORY.md
- Cleanup worktree

---

## 7. Sources cross-repo à lire (PARALLÈLE)

- `ios/MullvadVPN/Localizable.xcstrings` (target bulk-replace)
- `ios/MullvadVPN/Assets/LogoText.imageset/` + `AppIcon.appiconset/`
- `ios/PacketTunnel/PacketTunnelProvider/` (target C.4.5)
- `warren-ios/src/` (FFI exports Session C.3 deep)
- `desktop/packages/mullvad-vpn/locales/{en,fr}/messages.po` (FR+EN strings reference Warren desktop)
- `desktop/packages/mullvad-vpn/src/renderer/components/Logo.tsx` (logo SVG reference Warren)
- warrenbrowse-site `src/components/Logo.astro` (SVG canonical Warren)
- Memory `warren_session_c_continuation_phase7` + `warren_session_c_wireup_continuation`

---

## 8. Critères GO ULTIMATE

- ✅ iOS-Cleanup.2-iOS-Cleanup.6 critères GO PASS
- ✅ FR+EN 100% Warren strings (autres langs best-effort)
- ✅ Logo SVG vectoriel + AppIcon variants
- ✅ Live tunnel iOS Simulator vers warren-exit-1 prod OK
- ✅ Smoke 5+ tests PASS
- ✅ `xcodebuild build -scheme WarrenVPN` PASS
- ✅ Rapport rédigé
- ✅ Worktree cleaned

Verdict GO PARTIEL acceptable si :
- C.4.5 PacketTunnelActor build PASS mais live tunnel skipped (TestFlight signing pending poka)

---

## 9. Doctrine

- §0.0 INVIOLABLE git
- §0.5 autonomy
- §0.6 worktree séparé
- English-only code comments (Swift `//`)
- Pas em-dash
- Pas secrets in commits (Apple Developer keys)

---

## 10. Memory updates

- `warren_session_ios_cleanup_delivered.md`
- Update MEMORY.md

---

## 11. Commencer maintenant

Worktree §0.6, sources §7 en parallèle, attaque iOS-Cleanup.2 strings audit. Push au fil de l'eau.

Bonne route.
