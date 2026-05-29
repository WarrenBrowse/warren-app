# Audit de parité Android — Warren VPN

**Date** : 2026-05-29

## Résumé exécutif

Cet audit compare 10 clusters de fonctionnalités entre Warren desktop (référence) et Warren Android. Le constat est sévère : **aucun cluster n'est à pleine parité**. La répartition est la suivante — **0 à parité complète**, **0 réduit (parité fonctionnelle moindre mais cohérente)**, **7 partiels** (dns, ipv6, port-forwarding, multihop, account-keys, subscription-payment, onboarding, relay-lists-recents-fine, branding-assets — soit 8 partiels en comptant branding-assets), et **2 absents** (killswitch, failover). Trois clusters P0 concentrent les risques de fuite vie privée les plus critiques :

- **Killswitch / lockdown (absent)** : Android ne pose aucune règle de pare-feu lors d'une chute du tunnel. Tout le trafic peut fuiter pendant la fenêtre où le tunnel est tombé sans que l'utilisateur le sache. La délégation totale au paramètre système « always-on VPN » de l'OS laisse l'app sans aucune garantie applicative.
- **IPv6 (partiel — fuite active)** : la route `::/0` est posée en dur quelle que soit la préférence utilisateur. Sur un réseau IPv6, tout le trafic IPv6 est tunnelé sans contrôle ; pire, il n'existe aucun toggle pour le désactiver côté client, et `warren-jni` ignore totalement `enableIpv6`.
- **DNS (partiel)** : les modèles existent mais ne sont câblés nulle part. `addDnsServer()` n'est jamais appelé, aucun écran ne pilote le DNS, et le blocage de contenu (pubs/trackers/malware) repose entièrement sur le côté exit qui doit être documenté et vérifié.

Les clusters P1 (port-forwarding réduit à un bool, multihop sans sélection de pays, gestion de compte/devices absente, paiement/abonnement totalement absent, onboarding réduit à 2 écrans, listes/recents/obfuscation fine partielles, failover absent) représentent un déficit fonctionnel majeur mais sans fuite directe. Le seul cluster P2 (branding-assets / « Reconnect now ») est mineur.

## Tableau de synthèse

| Feature | Priorité | Desktop (profondeur) | Android (état) | Sévérité écart | Estimation |
|---------|----------|----------------------|----------------|----------------|------------|
| killswitch (lockdown / always-on) | P0 | Riche — toggle lockdown, notif « BLOCKING INTERNET », état `lockedDown` | Absent | Critique | XL |
| dns (blocage contenu + DNS custom) | P0 | Riche — 6 toggles de blocage + DNS custom | Partiel (modèle seul, zéro câblage) | Critique | L |
| ipv6 (toggle activation) | P0 | Complet — toggle persistant, affichage IPv6 | Partiel (route `::/0` en dur, fuite) | Critique | M |
| port-forwarding (NAT-PMP avancé) | P1 | Riche — protocole, port préféré, statut + compte à rebours | Partiel (bool seul) | Majeur | L |
| multihop (pays entrée/sortie) | P1 | Riche — toggle + pays entrée/sortie ISO | Partiel (bool seul) | Majeur | XL |
| failover (bascule auto exit) | P1 | Complet — toggle + bannière « EXIT SWITCHED » | Absent | Majeur | L |
| account-keys (devices, clés) | P1 | Riche — mnémonique, devices, suppression | Partiel (wallet seul) | Critique | L |
| subscription-payment (abo, voucher) | P1 | Riche — voucher Crockford-32, expiry, achat | Absent | Critique | XL |
| onboarding (wizard 5 étapes) | P1 | Complet — 5 étapes + détection 1er lancement | Partiel (2 écrans wallet) | Majeur | L |
| relay-lists-recents-fine | P1 | Riche — listes custom, recents, obfuscation 6+ méthodes | Partiel (4 bools) | Majeur | XL |
| branding-assets (« Reconnect now ») | P2 | Bouton reconnect persistant | Partiel (affordance absente) | Mineur | M |

---

## Détail par feature

### killswitch (lockdown / always-on) — P0 — Critique

**Desktop (profondeur + preuves)** : implémentation complète d'un mode lockdown (toggle booléen dans `ISettings`) qui bloque tout trafic à la déconnexion. UI dédiée avec bouton d'info distinguant Kill Switch et Lockdown Mode ; système de notification « BLOCKING INTERNET » ; champ `lockedDown: boolean` dans `TunnelState` ; Kill Switch désactivé en dur ; RPC `setLockdownMode`.
- `desktop/packages/mullvad-vpn/src/shared/daemon-rpc-types.ts:253` (FeatureIndicator.lockdownMode)
- `desktop/packages/mullvad-vpn/src/shared/daemon-rpc-types.ts:269` (DisconnectedState.lockedDown)
- `desktop/packages/mullvad-vpn/src/shared/daemon-rpc-types.ts:500` (ISettings.lockdownMode)
- `desktop/packages/mullvad-vpn/src/renderer/features/tunnel/components/lockdown-mode-setting/LockdownModeSetting.tsx:10-39`
- `desktop/packages/mullvad-vpn/src/renderer/components/views/vpn-settings/VpnSettingsView.tsx:70`
- `desktop/packages/mullvad-vpn/src/shared/notifications/block-when-disconnected.tsx:24-90`
- `desktop/packages/mullvad-vpn/src/renderer/components/views/vpn-settings/components/kill-switch-setting/KillSwitchSetting.tsx:32`
- `desktop/packages/mullvad-vpn/src/main/daemon-rpc.ts:534-536`

**Android (état + preuves)** : **absent**. Aucun champ `lockedDown` dans `WarrenTunnelConfig`, aucun réglage dans le repository, aucun champ d'état, aucune pose de règle pare-feu à l'échec du tunnel (simple log + arrêt du polling). L'écran d'onboarding renvoie vers les réglages VPN système. `onRevoke()` déconnecte sans poser de route de blocage. Aucun `android:setAlwaysOn`. Délégation totale à l'OS.
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenTunnelConfig.kt:15-36` (pas de champ lockdownMode)
- `android/lib/repository/src/main/kotlin/com/warrenbrowse/vpn/lib/repository/WarrenLocalSettingsRepository.kt:26-96`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenTunnelState.kt:10-38`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenQuinnAdapter.kt:77-79, 84-86`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenQuinnAdapter.kt:200-205`
- `android/lib/feature/autoconnect/impl/src/main/kotlin/com/warrenbrowse/vpn/feature/autoconnect/impl/AutoConnectAndLockdownModeScreen.kt:113-129`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenVpnService.kt:276-280`
- `warren-jni/src/tunnel.rs:38-69`
- `warren-core/crates/warren-tunnel/src/android_tun.rs`
- `android/app/src/main/AndroidManifest.xml:87-108`

**Support warren-core / warren-jni** : **partiel**. La crate `warren-killswitch` est production-grade sur Linux (nftables), macOS (pf), Windows (WFP/PowerShell), avec trait `KillswitchBackend` et cycle `install_with()` / `uninstall_explicit()`. Mais `warren-tunnel` n'intègre pas `warren-killswitch` sur Android : `AndroidTun` lit/écrit les paquets sans interception pare-feu, le fd TUN est passé sans installation de règle. Les capacités existent mais ne sont pas câblées côté Android.
- `warren-core/crates/warren-killswitch/src/lib.rs:1-37`, `:108-123`
- `warren-core/crates/warren-tunnel/src/lib.rs:5`
- `warren-core/crates/warren-tunnel/src/android_tun.rs:48-72`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenQuinnAdapter.kt:159-174`

**Changements par couche** :
- **UI Compose** : ajouter un composant `LockdownModeSetting` (parallèle desktop) dans les réglages VPN + bouton d'info ; à l'état `Failed` + `lockedDown=true`, afficher une notification « Lockdown mode actif, connexion bloquée ».
- **Repository** : ajouter `lockdownMode: StateFlow<Boolean>` + `setLockdownMode()`, persisté en SharedPreferences (`KEY_LOCKDOWN_ENABLED`), amorcé à la construction.
- **WarrenTunnelConfig** : ajouter `@SerialName("lockdown_mode") val lockdownMode: Boolean = false` ; le builder lit depuis le repository.
- **JNI + warren-core** : `WarrenTunnelConfig` Rust ajoute le champ optionnel, propagé via `connectTunnel` → `ClientTunnel` ; câbler un hook d'installation pare-feu post-connect (ex. `AndroidKillswitchTun` enveloppant `AndroidTun`).
- **WarrenTunnelState** : ajouter `lockedDown: Boolean` à toutes les variantes ; `statusFromCode()` infère `lockedDown` selon tunnel down + lockdown actif.

**Plan ordonné** :
1. Ajouter `lockdownMode` à `WarrenTunnelConfig` (Kotlin + Rust) + sérialisation.
2. Étendre le repository (StateFlow + setter + persistance).
3. Builder lit `lockdownMode` et peuple la config.
4. Ajouter `lockedDown` à `WarrenTunnelState` + variantes.
5. Pose de pare-feu lockdown-aware dans `WarrenQuinnAdapter` à l'échec du tunnel (équivalent Android de warren-killswitch ; documenter le mécanisme : `addDisallowedApplication` + routes ou commandes netd).
6. Détection de chute réseau sans déconnexion explicite (`ConnectivityManager.NetworkCallback` + timeout).
7. Composant `LockdownModeSetting`.
8. `LockdownModeNotificationProvider` (« BLOCKING INTERNET »).
9. `android:setAlwaysOn` dans le manifeste si lockdown actif.
10. Tests : sérialisation, persistance, install/uninstall pare-feu, notification.

**Estimation** : XL.

**Risques** :
- **CRITIQUE** : sans installation atomique des règles avec `lockedDown=true`, fenêtre de fuite avant que l'utilisateur réalise la chute. Poser les règles **avant** de signaler `state=Failed`.
- **HAUT** : install pare-feu (netd/iptables) nécessite privilèges élevés et peut entrer en conflit avec d'autres apps VPN ; tester la coexistence + stratégie de rollback.
- **HAUT** : si `onRevoke()` survient avec lockdown actif, les règles peuvent persister après perte du fd TUN → risque de « bricking » réseau. Prévoir auto-nettoyage (règle liée à l'uid, flush à la terminaison).
- **MOYEN** : boucles de reconnexion (WiFi → cellulaire) peuvent échouer en lockdown → device coupé d'internet. Timeout + fallback (ex. après 3 échecs, erreur utilisateur).
- **MOYEN** : si Android active un lockdown applicatif, le desktop devrait activer le Kill Switch en parité (coordination daemon pour éviter un mismatch d'indicateur).
- **BAS** : livraison de notification retardée sur device chargé → masque l'état « BLOCKING INTERNET ». Notification foreground persistante + alertes système.

---

### dns (blocage contenu + DNS custom) — P0 — Critique

**Desktop (profondeur + preuves)** : système DNS à deux niveaux — 6 toggles de blocage de contenu (pubs, trackers, malware, contenu adulte, jeux d'argent, réseaux sociaux) via `DefaultDnsOptions`, plus adresses DNS custom via `CustomDnsOptions`. Bascule `default` / `custom`, UI complète.
- `desktop/packages/mullvad-vpn/src/shared/daemon-rpc-types.ts:418-431`
- `desktop/packages/mullvad-vpn/src/renderer/components/views/vpn-settings/VpnSettingsView.tsx:63-64`
- `desktop/packages/mullvad-vpn/src/renderer/features/dns/components/block-ads-switch/BlockAdsSwitch.tsx:1-27`
- `desktop/packages/mullvad-vpn/src/renderer/features/dns/hooks/use-dns.ts:1-25`

**Android (état + preuves)** : **partiel**. Couche modèle présente (`DnsOptions.kt`, `CustomDnsOptions`, `DefaultDnsOptions`), enums `FeatureIndicator` DNS_CONTENT_BLOCKERS / CUSTOM_DNS affichés, mais **zéro câblage** vers la config tunnel ou le builder VpnService. Aucun écran, aucune persistance, aucun champ config. `buildTunInterface` ne pose aucun `addDnsServer()`.
- `android/lib/model/src/main/kotlin/com/warrenbrowse/vpn/lib/model/DnsOptions.kt`
- `android/lib/model/src/main/kotlin/com/warrenbrowse/vpn/lib/model/DefaultDnsOptions.kt:6-42`
- `android/lib/model/src/main/kotlin/com/warrenbrowse/vpn/lib/model/CustomDnsOptions.kt:7`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenTunnelConfig.kt:15-36`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenQuinnAdapter.kt:158-173`
- `android/lib/repository/src/main/kotlin/com/warrenbrowse/vpn/lib/repository/WarrenLocalSettingsRepository.kt`

**Support warren-core / warren-jni** : le filtrage de contenu (pubs/trackers/malware) est déjà réalisé côté exit (`warren-exit`). Côté client, il suffit de : (a) sérialiser `DnsOptions` dans le JSON `connectTunnel`, (b) configurer optionnellement les serveurs DNS du fd TUN via `VpnService.Builder` si des adresses custom sont définies. La logique de filtrage vit majoritairement côté exit, pas dans le pump client.

**Changements par couche** :
- **UI Compose** : écran de réglages DNS dans `lib/feature/settings/impl/` avec un toggle par champ `DefaultDnsOptions` + champ d'adresses DNS custom, lié au repository et au `SettingsViewModel`.
- **Repository** : ajouter `dnsState`, `customDnsAddresses`, `defaultDnsOptions` (StateFlow + setters), persistés en SharedPreferences (`dns_state`, `custom_dns_addresses` JSON, `default_dns_options` JSON).
- **WarrenTunnelConfig** : ajouter `dns: DnsSpec? = null` (`@SerialName("dns")`) côté Kotlin et `pub dns: Option<serde_json::Value>` côté Rust (`warren-jni/src/tunnel.rs` ~ligne 69, `#[expect(dead_code)]` jusqu'au câblage D.4).
- **JNI + warren-core** : `ClientTunnel` ne gère pas le DNS aujourd'hui ; le client sérialise `DnsOptions` et configure éventuellement les serveurs DNS du TUN. Filtrage exit-side à documenter/vérifier (D.4).
- **WarrenTunnelState** : `buildTunInterface()` lit `config.dns` et appelle `builder.addDnsServer(addr)` pour chaque adresse custom si `state=='custom'` ; sinon laisse le résolveur système.

**Plan ordonné** :
1. Ajouter le champ `dns` au `WarrenTunnelConfig` Kotlin (forme miroir desktop `IDnsOptions`).
2. Ajouter le champ `dns` au struct serde Rust (`#[expect(dead_code)]` jusqu'à D.4).
3. Étendre le repository (StateFlow + setters + persistance JSON).
4. Intégration builder : lire les réglages DNS et peupler `config.dns`.
5. `buildTunInterface()` appelle `addDnsServer(InetAddress)` pour chaque adresse custom en mode `custom` ; documenter que le blocage de contenu est exit-side.
6. Écran de réglages DNS (toggles + saisie adresses custom).
7. Navigation vers l'écran DNS.
8. Chaînes i18n (`block_ads`, `block_trackers`, ... `custom_dns_label`).
9. Test : flux `DnsOptions` → JSON → `connectTunnel` → `tunnel.rs` (placeholder handoff D.4) ; vérifier `addDnsServer()` en mode custom.

**Estimation** : L.

**Risques** :
- DNS VpnService (`addDnsServer`) est de niveau système, modifiable par d'autres apps ; documenter que le blocage de contenu n'est pas garanti au niveau TUN — Warren s'appuie sur le filtrage exit-side.
- Validation des adresses DNS custom : rejeter formats IP/port invalides avant sérialisation (sinon crash `connectTunnel` ou tunnel dégradé).
- Persistance liste `InetAddress` : encodeur/décodeur JSON kotlinx-serialization ; risque de mismatch avec desktop (protobuf Go) pour l'import de config.
- **D.4 follow-up** : le filtrage DNS exit-side doit être documenté et câblé ; sinon les toggles s'affichent activés sans effet.
- DNS IPv6 : `addDnsServer` accepte v4/v6 mais `warren-exit` peut ne servir qu'une famille ; tester sur réseaux IPv6-only.
- Reconnexion : changement de réglages DNS pendant connexion → `reconnect()` doit relire `DnsOptions` et reconstruire la config (déjà en place via cache + relecture).

**Décisions produit** : voir section dédiée.

---

### ipv6 (toggle activation) — P0 — Critique

**Desktop (profondeur + preuves)** : contrôle complet via toggle persistant (`enableIpv6`, défaut `false`), backé Redux, RPC `setEnableIpv6()`, affichage des adresses IPv4 et IPv6 dans les détails de connexion.
- `desktop/packages/mullvad-vpn/src/renderer/features/tunnel/components/enable-ipv6-switch/EnableIpv6Switch.tsx:10-26`
- `desktop/packages/mullvad-vpn/src/renderer/features/tunnel/hooks/use-enable-ipv6.ts:8,23`
- `desktop/packages/mullvad-vpn/src/shared/daemon-rpc-types.ts:414`
- `desktop/packages/mullvad-vpn/src/renderer/redux/settings/reducers.ts:87,153`
- `desktop/packages/mullvad-vpn/src/main/daemon-rpc.ts:530-531`
- `desktop/packages/mullvad-vpn/src/main/default-settings.ts:37`
- `desktop/packages/mullvad-vpn/src/renderer/components/views/main/components/connection-panel/components/connection-details/ConnectionDetails.tsx:108-112`

**Android (état + preuves)** : **partiel — fuite active**. Couche modèle seule (`TunnelOptions.kt:11` définit `enableIpv6`). Aucune UI. Le builder VpnService route **toujours** IPv4 (`0.0.0.0/0`) ET IPv6 (`::/0`) en dur. `WarrenTunnelConfig` n'a aucun champ `enableIpv6`, le builder ne le lit/passe jamais.
- `android/lib/model/src/main/kotlin/com/warrenbrowse/vpn/lib/model/TunnelOptions.kt:11`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenQuinnAdapter.kt:162,164`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenTunnelConfig.kt:14-36`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/connect/WarrenTunnelConfigBuilder.kt:29-75`
- `android/lib/feature/settings/impl/src/main/kotlin/com/warrenbrowse/vpn/feature/settings/impl/WarrenTunnelSettingsScreen.kt:49-120`

**Support warren-core / warren-jni** : **absent**. `warren-jni` ne définit/attend pas `enableIpv6` ; `run_session` lit daita/nat_pmp/obfuscation mais pas IPv6. `warren-tunnel` accepte v4 et v6 sans chemin de désactivation.
- `warren-app/warren-jni/src/tunnel.rs:39-69`
- `warren-app/warren-jni/src/tunnel.rs:124-241`
- `warren-core/crates/warren-tunnel/src/android_tun.rs:1-148`

**Changements par couche** :
- **UI Compose** : toggle IPv6 dans `WarrenTunnelSettingsScreen` (frère DAITA/NAT-PMP), miroir UX desktop (Switch + label).
- **Repository** : `ipv6Enabled` StateFlow (analogue `daitaEnabled`), hot-reload.
- **WarrenTunnelConfig** : ajouter `enableIpv6: Boolean` (`@SerialName("enable_ipv6")`).
- **JNI + warren-core** : `WarrenTunnelConfig` Rust accepte `enable_ipv6` ; le pump filtre conditionnellement l'IPv6 ou `ClientTunnel` ignore l'assignation IPv6 quand `enableIpv6=false`.
- **WarrenTunnelState** : `buildTunInterface()` n'appelle `addRoute("::", 0)` que si `enableIpv6=true` ; sinon omettre la route (ou blackhole).

**Plan ordonné** :
1. Ajouter `enableIpv6` au `WarrenTunnelConfig.kt`.
2. Ajouter `ipv6Enabled` StateFlow au repository.
3. Builder lit `localSettings.ipv6Enabled` et passe à la config.
4. Toggle IPv6 dans `WarrenTunnelSettingsScreen`.
5. `buildTunInterface()` route `::/0` conditionnellement.
6. `warren-jni/tunnel.rs` accepte `enable_ipv6` (`#[expect(dead_code)]` si non utilisé).
7. (Optionnel D.5) `warren-tunnel` respecte la désactivation IPv6 client-side.
8. Tests Android : présence/absence de la route IPv6 selon le toggle.

**Estimation** : M.

**Risques** :
- **Fuite IPv6** : tant que `addRoute("::", 0)` est en dur, tout le trafic IPv6 est tunnelé quel que soit le champ modèle.
- Implémentation asymétrique desktop/Android jusqu'au câblage complet → échecs de tests de parité.
- Si `warren-exit` ne sait pas désactiver IPv6 côté serveur, le toggle client seul peut ne pas suffire (l'exit assigne quand même une IPv6 au Setup).
- Certains systèmes mobiles traitent l'absence de route IPv6 comme une panne et coupent le VPN ; tester sur device réel.
- Migration : défaut `false` (parité desktop) pour les utilisateurs sans valeur persistée.

**Décisions produit** : voir section dédiée.

---

### port-forwarding (NAT-PMP avancé) — P1 — Majeur

**Desktop (profondeur + preuves)** : riche. `PortForwardingSettingsView` superpose toggle on/off, panneau avancé (protocole TCP/UDP, port externe préféré avec auto-fallback, validation [49152,65535], reconfig live sans reconnexion) et statut (machine à états 5 branches : off, attente tunnel, requesting, mapped + compte à rebours mm:ss au renouvellement à lifetime/2, failed avec raison traduite). `lifetimeSecs` persisté, clampé [60,3600]s.
- `desktop/packages/mullvad-vpn/src/renderer/components/views/port-forwarding-settings/PortForwardingSettingsView.tsx:30-73`
- `desktop/packages/mullvad-vpn/src/renderer/features/port-forwarding/components/port-forwarding-advanced/PortForwardingAdvanced.tsx:71-72,85-91,180-187,189-206`
- `desktop/packages/mullvad-vpn/src/renderer/features/port-forwarding/components/port-forwarding-status/PortForwardingStatus.tsx:34-141,150-185`
- `desktop/packages/mullvad-vpn/src/shared/daemon-rpc-types.ts:542-587`

**Android (état + preuves)** : **partiel** — toggle booléen minimal seulement. Un Switch « NAT-PMP port forwarding » ; pas de sélecteur de protocole, ni saisie de port, ni statut. `assignedNatPmpPort: Int?` existe sur `Connected` mais n'apparaît que dans le texte d'état (`describe()`), pas de panneau dédié.
- `android/lib/feature/settings/impl/src/main/kotlin/com/warrenbrowse/vpn/feature/settings/impl/WarrenTunnelSettingsScreen.kt:93-98`
- `android/lib/repository/src/main/kotlin/com/warrenbrowse/vpn/lib/repository/WarrenLocalSettingsRepository.kt:34-35,57-60`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenTunnelConfig.kt:22`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenTunnelState.kt:13-19`
- `android/app/src/main/kotlin/com/warrenbrowse/vpn/app/service/WarrenQuinnStateProxy.kt:57`

**Support warren-core / warren-jni** : **partiel**. `warren-natpmp-client` est complet (MapProto TCP/UDP, `NatPmpMapping`, boucle de refresh avec `NatPmpEvent` Mapped/Renewed/Failed + `NatPmpFailureReason`). Mais le wrapper JNI **code en dur** `MapProtocol::Udp`, `internal_port=0`, `lifetime=3600s` — sans lire protocole/port/lifetime depuis la config.
- `warren-core/crates/warren-natpmp-client/src/lib.rs`
- `warren-core/crates/warren-natpmp-client/src/refresh.rs`
- `warren-app/warren-jni/src/tunnel.rs:62,270-304`

**Changements par couche** :
- **WarrenTunnelConfig** : ajouter `protocol: String`, `suggestedExternalPort: Int` (0=auto), `lifetimeSecs: Int` (défaut 3600), miroir `NatPmpSettings` desktop ; côté Rust étendre le désérialiseur (`tunnel.rs:38-69`).
- **Repository** : persister `natPmpProtocol` (défaut `udp`), `natPmpSuggestedPort` (0), `natPmpLifetimeSecs` (3600) avec StateFlow + setters.
- **UI Compose** : remplacer le ToggleRow par un panneau multi-contrôle (switch + dropdown protocole + saisie port [49152,65535] + sélecteur lifetime presets 1h/6h/24h).
- **JNI + warren-core** : `maybe_spawn_nat_pmp` lit protocol/suggestedExternalPort/lifetimeSecs et les passe à `spawn_refresh_loop_from_addr` ; router les `NatPmpEvent` vers Kotlin via callback/canal.
- **WarrenTunnelState** : enrichir le statut (`externalPort`, `lifetimeGrantedSecs`, `mappingState` requesting/mapped/failed, `errorReason`) ; idéalement un `StateFlow<NatPmpStatus>` séparé de `Connected` pour éviter le bloat.

**Plan ordonné** :
1. Étendre `WarrenTunnelConfig.kt` (3 champs + `@SerialName`).
2. Étendre le désérialiseur Rust `tunnel.rs:38-69`.
3. Étendre le repository (3 clés + StateFlow).
4. Remplacer le ToggleRow par le panneau multi-contrôle.
5. Étendre `WarrenTunnelState.Connected` (statusState, mappedExternalPort, lifetimeGrantedSecs, errorReason).
6. `WarrenQuinnStateProxy.describe()` surface les transitions (requesting / mapped + countdown / failed).
7. Câbler callback JNI `maybe_spawn_nat_pmp` → état Kotlin.
8. `maybe_spawn_nat_pmp` lit protocol/port/lifetime de la config.
9. Composable `PortForwardingStatus` (machine à états).
10. Tests : sérialisation, persistance, transitions.

**Estimation** : L.

**Risques** :
- Callback Rust → Kotlin : gérer le cycle de vie (teardown service pendant callback en vol). `NatPmpGuard` (tunnel.rs:253-268) drop la boucle + drain ; s'assurer que le callback n'est pas en vol au drop.
- `Connected` est créé une fois à l'établissement ; mettre à jour le port/statut nécessite de remplacer l'instance + publier via le proxy, ou un `StateFlow` séparé (recommandé).
- L'API VPN Android n'expose pas nativement le port assigné ; modèle Kotlin NAT-PMP propre recommandé (pas juste append au texte d'état).
- Compte à rebours de renouvellement (RFC 6886 §3.7 : renouveler à lifetime/2) : tracking wall-clock (`System.currentTimeMillis()`) capturé atomiquement avec le changement d'état.
- NAT-PMP n'est pas live-reconfigurable (config passée au spawn) ; un changement pendant connexion ne prend effet qu'à la reconnexion. Documenter ou implémenter une affordance reconnect-on-toggle (cf. `WarrenTunnelSettingsScreen.kt:45-46` « D.4 step 9 »).

**Décisions produit** : voir section dédiée.

---

### multihop (pays entrée/sortie) — P1 — Majeur

**Desktop (profondeur + preuves)** : panneau multihop riche — toggle (opt-in, OFF par défaut), `entryCountry` et `exitCountry` (ISO 3166 alpha-2, vide=auto), plus `hpkeEpochRotationMs` persisté mais non exposé. Pickers = saisies texte simples (M4.H.C.X follow-up). gRPC `setWarrenMultiHopSettings()` persiste, redémarrage daemon requis.
- `desktop/.../warren-multi-hop-settings/WarrenMultiHopSettingsView.tsx:1-96`
- `desktop/.../warren-multi-hop/components/warren-multi-hop-country-pickers/WarrenMultiHopCountryPickers.tsx:1-90`
- `desktop/.../warren-multi-hop/hooks/use-warren-multi-hop.ts:1-52`
- `desktop/.../shared/daemon-rpc-types.ts` (WarrenMultiHopSettings)

**Android (état + preuves)** : **partiel** — toggle bool seul. Pas de champ `entryCountry`/`exitCountry`. Picker de pays EXIT seulement (`selectedExitId`). `entryHop` contient relayPubkeyHex + endpoint (pas de code pays), construit par logique de fallback du catalogue. `hpkeEpochRotationMs` totalement absent.
- `android/.../WarrenTunnelSettingsScreen.kt:100-112`
- `android/.../WarrenLocalSettingsRepository.kt:37-38, 62-65`
- `android/.../WarrenTunnelConfig.kt:15-36`
- `android/.../WarrenTunnelConfigBuilder.kt:29-74`
- `android/.../WarrenLocationPickerScreen.kt:1-144`

**Support warren-core / warren-jni** : **partiel**. `warren-multihop` (HPKE RFC 9180) câblé. `warren-relay-selector` doit supporter le filtrage par pays (à vérifier). Le JNI accepte `entry_hop` mais ne l'utilise pas via codes pays ; la sélection se fait côté Kotlin (builder, fallback catalogue). Deux options : (A) pousser le filtrage pays côté Rust (relay-selector accepte contraintes), (B) garder le picker Kotlin et enrichir le builder. Option B plus simple pour phase 1.
- `warren-app/warren-jni/src/tunnel.rs:44-45` (entry_hop dead_code)
- `warren-app/warren-jni/src/tunnel.rs:154`
- `warren-core/crates/warren-multihop/`

**Changements par couche** :
- **WarrenTunnelConfig** : ajouter `entryCountry: String?` et `exitCountry: String?` (`entry_country`, `exit_country`), défaut null (auto).
- **Repository** : `_entryCountry`/`_exitCountry` StateFlow + setters, persistés.
- **WarrenTunnelState** : pas de couche dédiée ; contraintes passées via la config JSON, consommées au connect par le builder.
- **UI Compose** : deux saisies texte (ISO alpha-2, 2 car. max, majuscule) sous le toggle, visibles seulement si `multiHopEnabled=true` ; éventuel refactor de `WarrenLocationPickerScreen` (exit-only) pour l'entrée.
- **JNI + warren-core** : désérialiseur ajoute `entry_country`/`exit_country` (inutilisés au début, D.4) ; intégration `warren-relay-selector` pour le filtrage par pays.

**Plan ordonné** :
1. Ajouter `entryCountry`/`exitCountry` à `WarrenTunnelConfig.kt`.
2. Étendre le repository (StateFlow + persistance).
3. `WarrenTunnelSettingsScreen` : saisies conditionnelles entrée/sortie.
4. Enrichir le builder : lire les pays et les passer à la sélection de relais (fallback = auto si null).
5. Désérialiseur Rust : ajouter les champs (inutilisés, D.4).
6. Rust : intégration `warren-relay-selector` (phase 2 ou //) — vérifier le support du filtrage pays.
7. Tests : persistance, e2e sélection manuelle, fallback auto.
8. (Optionnel phase 2) UI picker modal par pays backé relay-list.

**Estimation** : XL.

**Risques** :
- Catalogue de relais peu diversifié (entrée unique en phase courante) ; filtre pays peut renvoyer vide ou le même relais → fallback auto.
- Intégration `warren-relay-selector` : support du filtrage pays inconnu, spike précoce requis (Kotlin-side vs Rust-side).
- Validation code pays ISO alpha-2 (case-insensitive, normaliser).
- Pas de redémarrage daemon côté Android ; clarifier l'UX « prend effet à la reconnexion ».
- `hpkeEpochRotationMs` non exposé ; défaut 4h sûr en phase 1, décision à venir.

**Décisions produit** : voir section dédiée.

---

### failover (bascule auto exit) — P1 — Majeur

**Desktop (profondeur + preuves)** : implémentation complète. Toggle `WarrenFailoverSwitch` (effet immédiat, sans redémarrage daemon, défaut `true`), bannière in-app « EXIT SWITCHED » quand `failoverCount > acknowledgedCount`. Réglage GUI-only ; le daemon gère le failover via `select_failover_alternative` (préférence même-pays).
- `desktop/.../warren-mode/components/WarrenFailoverSwitch.tsx:1-25`
- `desktop/.../warren-mode/components/warren-failover-setting/WarrenFailoverSetting.tsx:1-44`
- `desktop/.../shared/daemon-rpc-types.ts` (WarrenFailoverSettings)
- `desktop/.../renderer/lib/notifications/warren-failover.ts:1-39`
- `desktop/.../main/settings.ts` (handleSetWarrenFailover)
- `desktop/.../main/gui-settings.ts`
- `desktop/.../main/default-settings.ts:104-106`

**Android (état + preuves)** : **absent**. Pas de toggle (seuls DAITA, NAT-PMP, multi-hop, M4.0). Pas de clé `failover_enabled` dans le repository, pas de champ config, pas de suivi d'événement « exit switched ». `warren-relay-selector` est en dépendance mais aucun `set_failover_enabled` n'est surfacé.
- `android/.../WarrenTunnelSettingsScreen.kt:49-112`
- `android/.../WarrenLocalSettingsRepository.kt:26-96`
- `android/.../WarrenTunnelConfig.kt:15-36`
- `android/.../WarrenTunnelConfigBuilder.kt:29-75`
- `android/.../WarrenTunnelState.kt:10-38`

**Support warren-core / warren-jni** : **complet** côté logique. `warren-relay-selector` exporte `select_failover_alternative` et `select_failover_alternative_for_attempt` (préférence même-pays, exclusion du relais cassé, tests dédiés). Mais le JNI surface `listRelays()` sans `set_failover_enabled` ni `get_failover_notification_count`. Commentaire `android_jni.rs` : schéma single-endpoint « until multi-endpoint failover lands ».
- `warren-core/crates/warren-relay-selector/src/selector.rs`
- `warren-core/crates/warren-relay-selector/tests/failover.rs:1-40`
- `warren-app/warren-jni/Cargo.toml:52`
- `warren-app/warren-jni/src/android_jni.rs`

**Changements par couche** :
- **Repository** : `failover_enabled: StateFlow<Boolean>` + `setFailoverEnabled()`, persisté (`failover_enabled`, défaut `true`).
- **WarrenTunnelConfig** : `failover_enabled: Boolean` (`@SerialName("failover_enabled")`) ; builder lit `localSettings.failoverEnabled`.
- **WarrenTunnelState** : ajouter `failoverCount: Int?` + `previousExitId: String?` à `Connected` (ou un sous-type `ExitSwitched`).
- **UI Compose** : ToggleRow « Automatic failover » lié au repo + bannière d'événement failover (miroir `WarrenFailoverNotificationProvider`).
- **JNI + warren-core** : surface l'état du toggle + un compteur d'événements failover (dans le status ou un `get_failover_count`) ; s'assurer que la config `failover_enabled` est lue côté Rust.

**Plan ordonné** :
1. Vérifier que `warren-tunnel`/`warren-client` accepte `failover_enabled` dans `ClientConfig` ; tracer la désérialisation, ajouter si manquant.
2. `failover_enabled` StateFlow (défaut true) + persistance + setter.
3. Champ config + builder.
4. Étendre `WarrenTunnelState.Connected` (suivi failover) + wiring du compteur.
5. ToggleRow « Automatic failover ».
6. Bannière de notification failover (afficher/dismiss, bump `acknowledgedFailoverCount`).
7. Surface JNI du compteur d'événements.
8. Test e2e : persistance, JSON, honneur du flag côté Rust, failover simulé + bannière.

**Estimation** : L.

**Risques** :
- `ClientConfig` Rust peut ne pas encore accepter `failover_enabled` ; audit désérialisation + call-site relay-selector, possible PR Rust.
- Schéma multi-endpoint non finalisé côté Android (`listRelays()` single-endpoint) ; failover limité au fallback pays (déjà implémenté).
- Notification failover nécessite la surface JNI du compteur ; sinon la bannière ne se déclenche pas. Coordination warren-tunnel.
- Pas d'infra de bannière transitoire côté Android (contrairement au sous-système desktop) ; possible nouveau flux d'événements `WarrenQuinnAdapter` → UI.

**Décisions produit** : voir section dédiée.

---

### account-keys (devices, clés) — P1 — Critique

**Desktop (profondeur + preuves)** : système de compte multi-vues — `KeysView` (révélation mnémonique destructive + confirmation + copie + restauration 12/24 mots), `AccountView` (nom de device, clé publique, expiry, boutons Buy/Redeem/Backup/Logout), `ManageDevicesView` (liste devices, device courant surligné, date de création, suppression non-courants), dialogues de confirmation/erreur. RPC : `getWarrenMnemonic`, `setWarrenMnemonic`, `listDevices`, `removeDevice`.
- `desktop/.../views/keys/KeysView.tsx:30-158`
- `desktop/.../views/keys/RestoreMnemonicView.tsx:27-147`
- `desktop/.../views/account/AccountView.tsx:26-131`
- `desktop/.../views/manage-devices/ManageDevicesView.tsx:15-61`
- `desktop/.../device-list-item/DeviceListItem.tsx:59-65`
- `desktop/.../shared/daemon-rpc-types.ts:459-468`

**Android (état + preuves)** : **partiel** — wallet-only, sans info compte ni gestion devices. Onboarding wallet (création/restauration, backup blur+reveal sans CTA copie) + révélation en réglages (biométrie → mnémonique). Pas de section compte (expiry, crédit, devices, suppression). `WarrenJni` n'expose que des primitives wallet (generateMnemonic, importMnemonic, mnemonicPubkeyHex, signCanonicalRequest). `AccountRepository`/`DeviceRepository` morts (null/no-op).
- `android/.../login/impl/WarrenWalletLoginScreen.kt:42-123`
- `android/.../login/impl/WarrenWalletBackupScreen.kt:40-90`
- `android/.../settings/impl/WarrenWalletSettingsSection.kt:49-190`
- `android/app/.../jni/WarrenJni.kt:32-79`
- `android/.../repository/AccountRepository.kt` (mort)
- `android/.../repository/DeviceRepository.kt` (mort)

**Support warren-core / warren-jni** : **partiel**. `warren-identity` (BIP39) complet, `warren-api-client` existe, `warren-api/src/devices.rs` exporte `DeviceStore` (register, list_for_owner, remove_for_owner). Mais `warren-jni` n'exporte pas les RPC devices (signing seul), `warren-api-client` n'est pas surfacé au JNI, et la suppression exige une signature owner (mnémonique fournie à chaque appel).
- `warren-core/crates/warren-identity/src/lib.rs:1-150`
- `warren-core/crates/warren-api/src/devices.rs:1-150`
- `android/app/.../jni/WarrenJni.kt`

**Changements par couche** :
- **JNI + warren-core** : ajouter `getWalletPubkey`, `listDevices(mnemonic, pubkey_hex) -> JSON`, `removeDevice(mnemonic, pubkey_hex, device_id)` appelant `warren-api-client` (GET/POST `/v1/devices`).
- **Repository** : créer `WarrenDeviceRepository` (Flow<List<Device>>, `removeDevice()` suspend) injectant le JNI.
- **WarrenTunnelConfig / WarrenTunnelState** : aucun changement (orthogonal au tunnel).
- **UI Compose** : `AccountSettingsSection` (nom device, clé publique tap-to-copy, lien devices) + `ManageDevicesScreen` (liste + date + suppression + confirmation) ; gate biométrique pour révélation/suppression.

**Plan ordonné** :
1. Auditer `warren-api` GET/DELETE `/v1/devices` (test `devices.rs` couvre list+create ; vérifier remove).
2. Ajouter `listDevices`/`removeDevice`/`getWalletPubkey` au JNI (`src/lib.rs` + `wallet.rs`/`devices.rs`), via `tokio::block_on` ou async.
3. Créer `WarrenDeviceRepository` (StateFlow + removeDevice) injectant le JNI + mnémonique du wallet.
4. `ManageDevicesScreen` (fetch à l'entrée, liste + suppression + confirmation).
5. `AccountSettingsRow` dans `WarrenWalletSettingsSection` (ou refactor tabbed Wallet/Account/Devices).
6. Gate biométrique (`BiometricPromptAuthorizer`) à la suppression.
7. Test : erreurs gRPC (not found, unauthorized), cleanup état navigation.
8. Test e2e : créer device sur desktop, supprimer depuis Android, vérifier cross-platform.

**Estimation** : L.

**Risques** :
- Durée de vie du token : la mnémonique n'est tenue que transitoirement ; chaque list/remove exige une fraîche (biométrie). Liste devient stale → refresh à l'entrée + TTL court (évict 5 min idle).
- Versioning API : endpoints `/v1/devices` peut-être incomplets (vérifier remove).
- Friction biométrique : re-auth à chaque suppression ; mettre en section dédiée pour rendre la re-auth attendue.
- Threading JNI : `warren-api-client` + tokio peut bloquer le thread Kotlin ; si > 100 ms, rendre async.
- Nommage device : Android crée des devices nommés `warren-device` en dur (devices.rs:141) ; pas de rename (P2 acceptable).

**Décisions produit** : voir section dédiée.

---

### subscription-payment (abo, voucher) — P1 — Critique

**Desktop (profondeur + preuves)** : 3 surfaces — `OnboardingSubscriptionView` (lien externe pricing + vérification manuelle via `updateAccountData()`/`submitVoucher`, auto-poll 10s/2min), `AccountView` (expiry, Buy more credit, Redeem voucher Crockford-32 `XXXX-XXXX-XXXX-XXXX`), vue compte expiré avec compte à rebours. RPC `submitVoucher -> VoucherResponse`. Pas de Play Billing.
- `desktop/.../views/onboarding/OnboardingSubscriptionView.tsx:14-190`
- `desktop/.../components/RedeemVoucher.tsx:24-363`
- `desktop/.../views/account/AccountView.tsx:26-131`
- `desktop/.../shared/daemon-rpc-types.ts` (VoucherResponse)
- `desktop/.../main/daemon-rpc.ts:287-310`

**Android (état + preuves)** : **absent**. Zéro UI abonnement/paiement/voucher. L'écran voucher Mullvad retiré du graphe de nav (D.4 step 16). Notifications d'expiry abandonnées (D.4 step 38). Pas d'appels `/v1/subscription`, `/v1/register`, ni paiements mobiles. Pas de bridge JNI paiement. Pas d'affichage d'expiry.
- `android/app/.../WarrenApp.kt:110`
- `android/app/.../di/AppModule.kt:54-150` (`:121`)
- `android/.../settings/impl/WarrenWalletSettingsScreen.kt:35-80`

**Support warren-core / warren-jni** : le backend `warren-api` implémente le flux complet (sessions de paiement éphémères 1h TTL anti-corrélation, vérification Apple JWS / Google purchase_token, idempotence via `mobile_purchase_links`, `/v1/register` voucher, `/v1/subscription`, suppression compte RGPD). Côté JNI rien n'est surfacé.

**Changements par couche** :
- **Repository** : `AccountRepository` réel (GET `/v1/subscription`, cache `expiry_at`, StateFlow observable).
- **WarrenTunnelState** : nouveau `SubscriptionStateProxy` (StateFlow<Long?> `expiry_at`).
- **WarrenTunnelConfig** : aucun lien (orthogonal).
- **JNI + warren-core** : exposer (a) init/check Apple, (b) init/acknowledge Google, (c) `/v1/register` voucher, (d) `/v1/subscription`, avec payloads `warren-api-types`.
- **UI Compose** : `AccountScreen` (expiry + countdown, Buy/Redeem/Logout) + `OnboardingSubscriptionScreen` (lien web + vérif + auto-poll 10s/2min).

**Plan ordonné** :
1. **DÉCISION** : voucher in-app (parité desktop) ou paiements mobiles uniquement ?
2. Si voucher in-app : JNI `/v1/register`.
3. Si paiements mobiles : StoreKit 2 + Play Billing v6+, endpoints init/verify, `MobilePurchaseLinkStore`.
4. `SubscriptionStateProxy` (StateFlow expiry).
5. `AccountRepository` réel (cache + observable + cleanup au logout).
6. `AccountScreen` (expiry, boutons).
7. `OnboardingSubscriptionScreen` (lien web, check, polling).
8. Bridges JNI paiement.
9. UI erreurs (voucher invalide/used, paiement échoué, session expirée).
10. Re-ajouter notifications de compte à rebours d'expiry (D.4 step 38).
11. Tests : voucher happy path, paiements mocks, countdown, vérif onboarding.

**Estimation** : XL.

**Risques** :
- **Secretisation** : ne jamais logger voucher_secret/purchase_token/JWS en clair ; impl `Debug` redactée côté Kotlin/JNI.
- **Idempotence** : retries de `/check`/`/acknowledge` → soumission dupliquée ; gate idempotence `mobile_purchase_links` (mobile_payments.rs:787).
- **Play Billing acknowledgement** : acquittement Google sous 3 jours sinon remboursement auto.
- **Anti-corrélation** : init() renvoie un UUID ; Apple/Google ne voient jamais la pubkey.
- **Format voucher** : Crockford-32 strict ([0-9A-HJKM-NP-TV-Z], 16 car.), même regex que desktop.
- **Suppression compte (RGPD)** : purge `subscriptions` + `mobile_purchase_links`.
- **Flux compte expiré** : auto-navigation à l'activation ; sinon erreur « No active subscription found ».
- **Pas de pricing en dur** : bouton « Buy credit » ouvre l'URL web (pas de billing in-app par défaut). Décision Play Billing obligatoire/optionnel.
- **Fraîcheur cache** : refresh agressif (~5 min) si expiry < 7 jours, sinon quotidien ; refresh au foreground.

**Décisions produit** : voir section dédiée.

---

### onboarding (wizard 5 étapes) — P1 — Majeur

**Desktop (profondeur + preuves)** : wizard 5 étapes avec détection 1er lancement persistante et replay. (1) Welcome, (2) Wallet (generate/import BIP39, blur+reveal, pas de copie), (3) Subscription (lien externe + auto-poll), (4) Preferences (Multi-hop OFF, DAITA OFF, obfuscation ON), (5) Done (persiste `onboardingCompletedUnix`). Détection : `onboardingCompletedUnix === undefined` → onboarding ; défini → main. Replay depuis Settings.
- `desktop/.../views/onboarding/OnboardingWelcomeView.tsx:9-23`
- `desktop/.../views/onboarding/OnboardingWalletView.tsx:13-27`
- `desktop/.../views/onboarding/OnboardingSubscriptionView.tsx:14-23`
- `desktop/.../views/onboarding/OnboardingPreferencesView.tsx:10-23`
- `desktop/.../views/onboarding/OnboardingDoneView.tsx:10-15`
- `desktop/.../shared/routes.ts:44-53`

**Android (état + preuves)** : **partiel** — 2 écrans wallet seulement. Splash route : Privacy → Wallet absent → Wallet (login/generate/import) → backup → Connect. Manque : Welcome, Subscription (aucun gate avant Connect), Preferences (réglages post-onboarding seulement), Done (aucun marqueur de complétion, détection 1er lancement faible). Splash route directement Wallet→Connect.
- `android/app/.../screen/splash/SplashViewModel.kt:14-53`
- `android/.../login/impl/navigation/WarrenWalletEntryProvider.kt:36-63`
- `android/.../login/impl/WarrenWalletLoginScreen.kt:25-123`
- `android/.../login/impl/WarrenWalletBackupScreen.kt:39-90`
- `android/.../home/impl/navigation/ConnectEntryProvider.kt:12-25`

**Support warren-core / warren-jni** : **complet** pour le wallet (Ed25519 + BIP39, marshalling JNI). Aucune logique de gating abonnement / validation préférences en Rust : ce sont des règles métier UI à implémenter côté app Android.

**Changements par couche** :
- **WarrenTunnelState** : `WalletRepository.state` gate déjà la navigation ; aucun suivi abonnement/préférences.
- **WarrenTunnelConfig** : config construite au connect ; pas de capture onboarding-time.
- **Repository** : pas de persistance d'état onboarding (pas d'équivalent `onboardingCompletedUnix`), pas de suivi abonnement.
- **UI Compose** : 5 nouveaux écrans (Welcome, Preferences, Subscription, Done, Replay) + modifier `SplashViewModel` pour enchaîner les étapes et appliquer le gate abonnement.
- **JNI + warren-core** : aucun changement (wallet OK) ; éventuelle requête d'état abonnement si le gate passe au niveau daemon.

**Plan ordonné** :
1. Audit : le check abonnement appartient-il au daemon ou est-il UI pure ?
2. `OnboardingWelcomeScreen`.
3. `SplashViewModel` : enchaîner Wallet→Welcome→Subscription→Preferences→Done→Connect.
4. `OnboardingSubscriptionScreen` (lien web + « I already have » + « Check again » + poll).
5. `OnboardingPreferencesScreen` (4 toggles, défauts desktop).
6. `OnboardingCompletionScreen` (Done + persistance timestamp).
7. `WarrenWalletEntryProvider` : router vers Welcome après WalletReady.
8. Entrée Settings Replay (clear flag → Splash).
9. Test flux complet 1er lancement + 2e lancement + replay.

**Estimation** : L.

**Risques** :
- Vérification abonnement non implémentée ; orchestration polling à décider (deep-link retour de warrenvpn.com ?).
- Perte d'état auto-poll si process tué après ouverture navigateur ; timeout + reprise à décider.
- Toggles préférences accessibles seulement post-connect aujourd'hui ; UX « apply on next connect » à clarifier.
- Détection 1er lancement par état wallet seul est plus faible que le timestamp desktop ; persister un flag de complétion séparé recommandé.

**Décisions produit** : voir section dédiée.

---

### relay-lists-recents-fine — P1 — Majeur

**Desktop (profondeur + preuves)** : riche et complet. Listes custom (create/edit/delete/add/remove), recents (singlehop + multihop, toggle enable/disable), DAITA 2 toggles (global + « direct only » avec modale d'avertissement), obfuscation 6+ méthodes (auto/off/udp2tcp/shadowsocks/quic/lwo/wireGuardPort) avec réglages fins ; M4.0 HTTP/3 mimicry auto en mode Warren ; relay overrides (IP-in/IPv6-in par hostname).
- `desktop/.../custom-lists/components/custom-list-menu/CustomListMenu.tsx:35-83`
- `desktop/.../locations/components/geographical-location-menu/GeographicalLocationMenu.tsx:28-62`
- `desktop/.../locations/types.ts:54-56`
- `desktop/.../locations/hooks/use-recents.ts:1-14`
- `desktop/.../daita/components/daita-setting/DaitaSetting.tsx:14-27`
- `desktop/.../daita/components/daita-direct-only-setting/DaitaDirectOnlySetting.tsx:12-47`
- `desktop/.../shared/daemon-rpc-types.ts:690-706`
- `desktop/.../views/anti-censorship/AntiCensorshipView.tsx:15-22, 42-43, 83-99`
- `desktop/.../shared/daemon-rpc-types.ts:509, 779-783`

**Android (état + preuves)** : **partiel** — contrôles toggle de base, lacunes sévères. 4 toggles bool (daita, natPmp, multiHop, obfuscationM40, sans options fines). Picker exit unique. Pas de listes custom (supprimées D.4 step 45). Composants recents définis mais orphelins. Obfuscation booléenne (M4.0), pas de picker multi-méthodes.
- `android/.../WarrenTunnelSettingsScreen.kt:49-120`
- `android/.../WarrenLocationPickerScreen.kt:46-96`
- `android/.../WarrenLocalSettingsRepository.kt:31-50`
- `android/.../WarrenTunnelConfig.kt:15-36`
- `android/.../di/UiModule.kt:86-88`
- `android/.../lib/ui/component/relaylist/RelayListItem.kt`

**Support warren-core / warren-jni** : à étendre. `warren-jni`/`warren-tunnel` `ClientConfig` doit accepter customLists, recentsEnabled, selectedObfuscationType (enum), daitaDirectOnly, relayOverrides. `warren-relay-selector`/`warren-tunnel` doivent câbler la sélection du type d'obfuscation (aujourd'hui DAITA + bool M4.0 seulement) et appliquer les relay overrides à la résolution d'endpoint.

**Changements par couche** :
- **UI Compose** : (1) section Listes custom (create/edit/delete/add/remove), (2) toggle recents + liste (réutiliser composants orphelins), (3) picker multi-méthodes d'obfuscation + panneaux par méthode, (4) toggle « Direct only » imbriqué sous DAITA (désactivé si DAITA off ou multihop off).
- **WarrenTunnelState** : `Connected` suit déjà daita + obfuscationM40 ; aucun changement.
- **WarrenTunnelConfig** : ajouter customLists, recentsEnabled, selectedObfuscationType (enum) + configs par méthode, daitaDirectOnly, relayOverrides.
- **Repository** : ajouter customListsRepository (CUD), recentsEnabled, selectedObfuscationType + réglages par méthode, daitaDirectOnly, relayOverrides.
- **JNI + warren-core** : `ClientConfig` accepte les nouveaux champs ; câbler la sélection d'obfuscation + l'application des relay overrides.

**Plan ordonné** :
1. [UI] Écran Listes custom (CUD + add/remove location).
2. [UI] Toggle recents (réutiliser composants orphelins).
3. [UI] Picker obfuscation + panneaux par méthode.
4. [UI] « Direct only » imbriqué sous DAITA (logique d'avertissement desktop).
5. [Repository] Étendre avec les nouveaux StateFlow + persistance.
6. [Repository] `CustomListsRepository` (CUD + add/remove location).
7. [Config] Étendre `WarrenTunnelConfig` (noms JSON = serde Rust).
8. [Builder] Lire et peupler tous les champs.
9. [Rust] Désérialisation `connectTunnel` + câbler la sélection d'obfuscation.
10. [Rust] Appliquer relay overrides à la résolution d'endpoint.

**Estimation** : XL.

**Risques** :
- Listes custom : sync avec `/relays` si server-provisioned ; vérifier la propriété des données.
- Obfuscation : les crates Rust peuvent ne pas supporter les 6+ méthodes ; vérifier avant le picker.
- DAITA direct-only : `warren-relay-selector` doit respecter un filtre « relais DAITA seulement ».
- Relay overrides IPv4/IPv6 : routage TUN doit respecter les overrides ; vérifier la résolution d'endpoint.
- Recents : ownership local (SharedPreferences) vs daemon-synced à décider.
- Breaking change si désérialisation Rust strict-valide les champs inconnus ; `#[serde(default)]`.

**Décisions produit** : voir section dédiée.

---

### branding-assets (« Reconnect now ») — P2 — Mineur

**Desktop (profondeur + preuves)** : `ReconnectButton` toujours visible quand connecté dans le panneau de sélection ; pas de dialogue « Reconnect now » — action directe.
- `desktop/.../select-location-buttons/components/reconnect-button/ReconnectButton.tsx`

**Android (état + preuves)** : **partiel**. 4 toggles mais aucune affordance « Reconnect now ». Commentaire ligne 46-47 : « D.4 step 9 will add a "Reconnect now" affordance ». Affiche seulement « Changes apply on next connect ». Le bouton reconnect existe dans `ConnectScreen` mais n'est pas déclenché par un changement de réglage.
- `android/.../WarrenTunnelSettingsScreen.kt:46-47`
- `android/.../WarrenTunnelSettingsScreen.kt:82-84`
- `android/.../home/impl/connect/ConnectScreen.kt:815-832`

**Support warren-core / warren-jni** : **complet**. Le JNI désérialise déjà les 4 champs ; le reconnect est un flux IPC Android pur (intent → `WarrenVpnService` → Quinn adapter). Aucun changement Rust.

**Changements par couche** :
- **UI Compose** : `WarrenTunnelSettingsScreen` suit l'état tunnel (déjà collecté ligne 58) et compare les valeurs avant/après ; si changement pendant `Connected`, afficher conditionnellement l'affordance reconnect.
- **WarrenTunnelState** : `WarrenTunnelStateProvider` alimente déjà l'écran ; aucun changement.
- **WarrenTunnelConfig** : aucun changement.
- **Repository** : setters existants (`setDaitaEnabled`, etc.) ; éventuellement émettre un side-effect si connecté.
- **JNI + warren-core** : aucun changement.

**Plan ordonné** :
1. Vérifier les patterns desktop (un réglage déclenche-t-il une offre de reconnexion ?).
2. Définir l'UX Android (toast+bouton / modale / texte inline).
3. Suivre l'état précédent du flag + détecter le changement pendant connexion.
4. Implémenter l'affordance (side-effect, Snackbar, ou bouton conditionnel).
5. Câbler vers `WarrenReconnectUseCase`.
6. Test : toggle pendant connexion → affordance → reconnexion avec nouvelle config.
7. Vérifier icône launcher + chaînes Mullvad résiduelles (déjà confirmé propre).

**Estimation** : M.

**Risques** :
- Sans garde, un toggle accidentel pendant connexion peut déclencher des reconnexions répétées.
- `WarrenReconnectUseCase` doit garantir un teardown/rebuild propre du tunnel avec la nouvelle config.
- Aucun test sur ce flux ; ajouter de la couverture d'intégration.

**Décisions produit** : voir section dédiée.

---

## Plan d'implémentation ordonné

### P0 — Sécurité / fuites vie privée (à traiter en premier)

1. **ipv6 (M)** — Le plus rapide à corriger et fuite active immédiate. Le routage `::/0` en dur tunnele tout l'IPv6 sans contrôle. Câbler le toggle de bout en bout (config → repo → builder → route conditionnelle → JNI). Aucune dépendance bloquante.
2. **killswitch / lockdown (XL)** — Risque de fuite le plus grave (fenêtre de fuite à la chute du tunnel). Dépend de la pose de pare-feu Android (mécanisme à valider) ; capacités `warren-killswitch` existantes mais non câblées sur Android. À mener en parallèle de l'ipv6 mais avec une attention forte sur l'atomicité install-avant-Failed.
3. **dns (L)** — Modèle déjà présent ; câbler config/repo/UI + `addDnsServer`. Dépend de la confirmation/câblage du filtrage exit-side (D.4) pour le blocage de contenu.

Ces trois clusters partagent tous une extension du **schéma `WarrenTunnelConfig` (Kotlin ↔ Rust)** : grouper les ajouts de champs (lockdown_mode, dns, enable_ipv6) dans une même passe pour limiter les allers-retours de synchronisation serde.

### P1 — Parité fonctionnelle

4. **failover (L)** — Logique Rust complète ; vérifier d'abord que `ClientConfig` accepte `failover_enabled`, puis câbler repo/config/UI + compteur d'événements JNI. Dépendance : surface JNI du compteur (coordination warren-tunnel).
5. **account-keys (L)** — Préalable à subscription-payment (devices + clés). Dépend de l'audit `/v1/devices` (remove) + exposition JNI de `warren-api-client`.
6. **subscription-payment (XL)** — **Décision produit bloquante** (voucher in-app vs paiements mobiles) avant tout code. Dépend de la surface JNI compte (partage avec account-keys) et du backend `warren-api` (déjà mature).
7. **onboarding (L)** — Dépend de subscription-payment (étape Subscription du wizard) et réutilise les écrans Preferences existants. À séquencer après le choix du gate abonnement.
8. **port-forwarding (L)** — Indépendant ; étend NAT-PMP (config + repo + UI + callback JNI). Réutilise le pattern d'affordance reconnect de branding-assets.
9. **multihop (XL)** — Dépend d'un spike `warren-relay-selector` (support filtrage pays). Option B (Kotlin-side) pour phase 1, refactor Rust phase 2.
10. **relay-lists-recents-fine (XL)** — Le plus large ; dépend du support Rust des 6+ méthodes d'obfuscation et du filtre DAITA direct-only. À fractionner en sous-livraisons (listes custom / recents / obfuscation fine / relay overrides).

### P2 — Confort

11. **branding-assets / « Reconnect now » (M)** — Aucune dépendance Rust ; pur UI Android. À livrer en même temps que le premier cluster P1 qui modifie un toggle pendant connexion (port-forwarding) pour mutualiser l'affordance reconnect.

**Dépendances clés** : extension partagée du schéma `WarrenTunnelConfig` (P0) → account-keys précède subscription-payment → subscription-payment précède onboarding → spike relay-selector débloque multihop et le filtre DAITA direct-only de relay-lists.

---

## Décisions produit en attente

### killswitch
- Sur Android, le lockdown doit-il s'appuyer sur le « always-on VPN » système (`android:setAlwaysOn` + consentement) ou implémenter des règles pare-feu in-app + empêcher la déconnexion manuelle ?
- Au changement de réseau (WiFi ↔ cellulaire), faut-il auto-reconnecter en lockdown (préserver la garantie) ou mettre en pause avec une notification « en attente de reconnexion » (éviter les boucles de route) ?
- Le lockdown Android doit-il bloquer aussi les CIDR de bypass (réseau local) aussi strictement que le desktop, ou autoriser l'accès LAN (RFC1918) comme l'option `allow_lan` de warren-killswitch ?
- Si le fd TUN est révoqué (`onRevoke`), peut-on garder une route de blocage de secours jusqu'à l'arrêt complet de l'app, ou Android tue-t-il l'app avant qu'on puisse poser les règles ?

### dns
- Si l'utilisateur choisit « DNS custom » mais ne fournit aucune adresse : rejeter (au moins une adresse requise) ou retomber sur le résolveur système ? (recommandation : rejeter avec erreur de validation UI).
- Confirmer que les bloqueurs de contenu (blockAds, blockTrackers...) s'appliquent **exit-side uniquement** et le documenter clairement dans l'UI.
- Multi-hop : le DNS s'applique-t-il à l'entrée et à la sortie, ou à la sortie seulement ? (à confirmer avec l'équipe warren-exit).

### ipv6
- Android doit-il défaut à `enableIpv6=false` (parité desktop) ou `true` (connectivité max) ?
- Si l'exit refuse d'honorer la désactivation IPv6 (assigne quand même une IPv6), le client doit-il rejeter le SetupAck ou ignorer silencieusement l'assignation ?
- Le toggle IPv6 va-t-il dans « Warren tunnel » (avec DAITA/NAT-PMP) ou dans une section « Vie privée & fuites » (avec DNS/bypass) ?
- `warren-exit` a-t-il déjà une logique de désactivation IPv6 par session, ou faut-il l'ajouter côté serveur ?

### port-forwarding
- Lifetime : sélecteur de presets 1h/6h/24h ou saisie libre en secondes ? (desktop clampe [60,3600], presets acceptables).
- Port externe préféré : optionnel (null=auto) ou toujours présent ? (desktop traite 0 = auto ; recommandation : même convention).
- Compte à rebours de renouvellement : mise à jour chaque seconde (parité desktop) ou statique « renouvellement dans 30m » ?

### multihop
- Le picker de pays d'entrée doit-il permettre la sélection par ID de relais (dropdown relay-selector) ou seulement la saisie de code ISO (parité desktop minimal) ?
- Faut-il exposer `hpkeEpochRotationMs` dans l'UI Android, ou différer en phase 2 ?
- Stratégie de fallback quand multihop activé mais pays vides (auto) : aligner sur le desktop (n'importe quel entry actif distinct de l'exit) ?
- Picker pays = saisie texte (rapide) ou picker modal relay-list (plus convivial mais nécessite un fetch) ?

### failover
- UX de notification : bannière/toast à chaque failover, ou log silencieux + compteur persisté ? (desktop : bannière « EXIT SWITCHED » comme différenciateur).
- Défaut `failover_enabled` : `true` (parité desktop) ou opt-in ? (recommandation : true).
- Suivi des tentatives de failover par session ou persisté entre sessions (impacte l'état « acknowledged ») ?

### account-keys
- Android doit-il afficher l'expiry/crédit d'abonnement comme le desktop `AccountView`, ou hors scope D.5 ? (recommandation : clé publique + devices seulement, modéliser l'abonnement plus tard).
- Le nom de device doit-il être éditable, ou en lecture seule ? (recommandation : lecture seule, aligné sur les deux plateformes).
- « Manage Devices » : écran dédié ou section repliable dans `WarrenWalletSettingsSection` ? (recommandation : sous-écran dédié).

### subscription-payment
- **Voucher in-app** (saisie Crockford-32, parité desktop) ou achat web uniquement ? (in-app nécessite JNI GET `/v1/subscription` + POST `/v1/register`).
- **Plateforme de paiements mobiles** : implémenter Apple StoreKit 2 + Google Play Billing v6+, ou « travaux futurs » (redirection web) ? (impacte JNI, repository, complexité UI, conformité légale/fiscale, commission 30%).
- **Visibilité du paiement** : afficher « Abonnement actif, expire le 2026-06-15 » sur l'écran compte, ou masquer l'expiry ? (recommandation : toujours afficher, avec countdown si < 7 jours).
- **Timing de refresh** : quand fetch GET `/v1/subscription` (launch / resume / polling) ? (recommandation : agressif ~5 min si < 7 jours, sinon quotidien).
- **Comportement compte expiré** : bloquer la connexion VPN si l'abonnement expire, ou laisser l'exit rejeter la pubkey silencieusement ? Afficher un avertissement si expiry < 24h ?

### onboarding
- Android applique-t-il un gate d'abonnement (bloque Connect sans abo actif) ou seulement une incitation (lien + « check again ») ?
- La vérification d'abonnement est-elle daemon-side (nouvel endpoint RPC JNI) ou app-side (expiry en cache) ?
- La complétion d'onboarding est-elle suivie explicitement (nouveau champ `UserPreferencesRepository`) ou inférée de l'état wallet + settings ? (recommandation : flag explicite).
- Le Replay d'onboarding doit-il être disponible dans les Settings Android (parité desktop) ? Si oui, quel libellé ?

### relay-lists-recents-fine
- Les listes custom sont-elles créées par l'utilisateur (style Mullvad) ou provisionnées serveur ? (si serveur, fetch + merge avec le catalogue).
- Les recents sont-ils locaux (SharedPreferences) ou synchronisés au daemon ? (desktop : daemon ; Android possible offline-first).
- Pour l'obfuscation, supporter les 6 méthodes Mullvad ou seulement M4.0 + un sous-ensemble ? (M4.0 always-on en mode Warren ; clarifier l'applicabilité des méthodes Mullvad si `warrenMode=false`).
- Les « relay overrides » sont-ils user-facing (UI de saisie d'overrides IP) ou cachés (admin/debug) ? (la surface desktop suggère interne).

### branding-assets
- UX du prompt de reconnexion : (a) toast+bouton en bas de `WarrenTunnelSettingsScreen`, (b) modale de confirmation, (c) texte inline « Reconnect now » apparaissant seulement au changement d'un toggle pendant connexion ? (desktop = bouton persistant ; Android devrait suivre le paradigme mobile à surface réduite).

---

## Risques transverses

- **Synchronisation de schéma Kotlin ↔ Rust** : la majorité des clusters P0/P1 ajoutent des champs à `WarrenTunnelConfig` (Kotlin `@SerialName`) qui doivent correspondre exactement au struct serde de `warren-jni/src/tunnel.rs`. Tout désalignement de nom JSON ou de type provoque des échecs silencieux de désérialisation ou de connexion. **Mitigation** : grouper les ajouts de champs P0 dans une seule passe ; utiliser systématiquement `#[serde(default)]` côté Rust pour tolérer les champs absents et éviter les breaking changes ; tests de roundtrip de sérialisation par cluster.

- **Ne pas casser le desktop / les autres consommateurs de warren-core** : `warren-tunnel`, `warren-killswitch`, `warren-relay-selector`, `warren-natpmp-client`, `warren-api` sont partagés avec le daemon desktop. Toute modification de signature (ex. ajout de `failover_enabled` à `ClientConfig`, filtrage pays dans le relay-selector, sélection d'obfuscation) doit rester rétro-compatible et ne pas régresser les chemins desktop/iOS. **Mitigation** : champs optionnels avec défauts, garder les call-sites existants intacts, audit des consommateurs avant toute modification de trait/struct partagé.

- **Mécanisme pare-feu Android non éprouvé (killswitch)** : l'équivalent Android de warren-killswitch (netd/iptables/per-UID routing ou `addDisallowedApplication`) requiert des privilèges et peut entrer en conflit avec d'autres apps VPN ou laisser le device en état bloqué (« bricking »). C'est le risque technique le plus élevé du backlog. **Mitigation** : spike isolé sur le mécanisme + stratégie d'auto-nettoyage liée à l'uid avant tout câblage.

- **Garanties exit-side non vérifiées** : DNS (blocage de contenu), IPv6 (désactivation par session), multihop (filtrage pays) reposent partiellement sur `warren-exit`. Si l'exit n'implémente pas ces fonctions, les toggles client s'afficheront actifs sans effet réel (fausse impression de protection). **Mitigation** : confirmer/documenter les capacités exit-side (D.4) avant d'exposer les toggles comme « actifs ».

- **Surface JNI / threading** : plusieurs clusters (account-keys, subscription-payment, port-forwarding, failover) nécessitent de nouveaux appels JNI synchrones potentiellement bloquants (`warren-api-client` + tokio) ou des callbacks Rust→Kotlin avec gestion de cycle de vie (teardown service pendant callback en vol). **Mitigation** : mesurer la latence, basculer en async si > 100 ms, garantir l'abort des callbacks au drop des guards.

- **Build / CI** : iOS est désactivé par défaut (commit récent `4b01da87`) ; les modifications partagées de warren-core doivent rester buildables pour desktop + Android sans casser la matrice CI. Les ajouts de dépendances (Play Billing, StoreKit) impactent les pipelines de release Android/iOS et la conformité store (commission 30%, vérification de reçu). **Mitigation** : compiler les nouveaux champs Rust avec `#[expect(dead_code)]` jusqu'au câblage complet, fractionner les XL (multihop, relay-lists, subscription) en sous-livraisons CI-vertes.

- **Anti-corrélation / secrétisation (paiement)** : transverse au cluster paiement mais avec impact sécurité global — ne jamais transmettre la pubkey à Apple/Google, ne jamais logger voucher_secret/purchase_token/JWS. **Mitigation** : impls `Debug` redactées côté Kotlin/JNI, sessions de paiement éphémères UUID-keyed, gate d'idempotence backend.
