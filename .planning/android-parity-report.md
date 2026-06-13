# Rapport de parité Android, Warren VPN

**Date** : 2026-05-29
**Statut** : en cours, Phase A (audit) + Phase B (P0 privacy) livrées et vérifiées ; Phase C (P1) partielle (port forwarding, multi-hop pays) ; Phase D (P2) livrée (icône, Reconnect now).

Ce rapport accompagne l'audit [`android-parity-audit.md`](android-parity-audit.md) et suit l'avancement de la mise à parité.

## Décisions produit (validées par l'utilisateur, 2026-05-29)

| Sujet | Décision |
|-------|----------|
| Kill switch / lockdown | **Always-on système (OS) + blackhole TUN applicatif** (pas de root, pas d'iptables/netd) |
| Abonnement / paiement | **Voucher in-app + statut/expiry seulement**, pas de Play Billing |
| DNS | **DNS custom + toggles de blocage de contenu côté exit, documentés** |
| Défaut IPv6 | **`false`** (parité desktop : IPv6 bloqué sauf activation explicite) |

## Livré et vérifié

### Phase B, P0 : fuites vie privée (CRITIQUE), ✅ FAIT

Commits : `25be6c737f`, `514a617e4b`.

| Fuite | Avant | Après |
|-------|-------|-------|
| **IPv6** | route `::/0` posée en dur, IPv6 tunnelé sans contrôle, aucun toggle | Toggle IPv6 (défaut off). La route `::/0` est **toujours** posée pour capturer l'IPv6 ; quand off, **aucune adresse v6 n'est assignée** → l'IPv6 est blackholé (pas de fuite), quand on → IPv6 porté par le tunnel. |
| **DNS** | aucun `addDnsServer()`, DNS pouvait fuiter vers le résolveur LAN | DNS **toujours** routé dans le tunnel : résolveur exit `10.66.0.1` par défaut, ou serveurs custom validés. Mode custom + 6 toggles de blocage (exit-side) persistés et envoyés dans la config. |
| **Kill switch** | aucune protection à la chute du tunnel, trafic repartait en clair | Mode lockdown : à une chute **inattendue**, l'adapter établit une interface **blackhole** (capture tout, ne pompe rien) au lieu de rendre la main au réseau, puis retente la connexion. Nouvel état `WarrenTunnelState.Blocking`. Le handover réseau (changement Wi-Fi/cellulaire) ne déclenche pas le kill switch (téléchargement volontaire). |

**Architecture** : nouveau module pur et testable `WarrenTunInterfacePlan.kt` qui calcule adresses/routes/serveurs DNS à partir de la config (avec variante `blocking`). `WarrenQuinnAdapter` applique le plan et porte la machine à états du kill switch (`blockingFd`, `userInitiatedDisconnect`, `scheduleLockdownReconnect`).

**Chaîne complète câblée** : UI (`WarrenTunnelSettingsScreen` sections Privacy + DNS) → `WarrenLocalSettingsRepository` (persistance) → `WarrenTunnelConfigBuilder` → `WarrenTunnelConfig` (Kotlin) → `WarrenJni.connectTunnel` → `warren-jni/src/tunnel.rs` (champs serde synchronisés) ; enforcement IPv6/DNS au niveau `VpnService.Builder`, kill switch dans l'adapter, état remonté via `WarrenQuinnStateProxy`.

**Tests** : `WarrenTunInterfacePlanTest` (11 cas : routage v4/v6, blackhole IPv6, DNS in-tunnel par défaut/custom/filtrage invalide, plan blocking), `WarrenTunnelConfigBuilderTest` (+ipv6/lockdown/DNS), `WarrenLocalSettingsRepositoryTest` (+persistance). Build local `:lib:repository` + `:app` vert.

**Note Always-on** : l'app étant un `VpnService` avec `stopWithTask="false"`, l'Always-on/Lockdown système de l'OS est déjà disponible (l'utilisateur l'active dans les réglages VPN Android). L'auto-connect *headless* sous always-on reste un suivi (le mnémonique est gated par biométrie ; nécessite une décision produit sur un cache de clé non-biométrique).

### Phase C, P1 (entamée)

#### Port forwarding (profondeur), ✅ FAIT

Commit : `db93d2edd9`.

- `WarrenTunnelConfig` étendu : `nat_pmp_protocol` (udp/tcp), `nat_pmp_external_port` (0=auto), `nat_pmp_lifetime_secs`.
- `warren-jni` : `maybe_spawn_nat_pmp` lit désormais protocole/port/lifetime depuis la config (avant : `Udp`/`0`/`3600` codés en dur) et les passe à `warren-natpmp-client`.
- Repository : persistance + clamps (port ∈ [49152,65535] ou 0 ; lifetime ∈ [60s, 24h]).
- UI : panneau avancé sous le toggle NAT-PMP (chips protocole UDP/TCP, champ port, presets lifetime 1h/6h/24h).
- Tests : builder + repository (protocole/port/lifetime).

#### Port forwarding, status live, ✅ FAIT

Commit : `1c0801276b`.

- Rust (`warren-jni`) : la boucle NAT-PMP publie son état dans un `static NATPMP_STATUS` (architecture polée, comme `getTunnelStatus`, pas de callback JVM), mappant `NatPmpEvent` → JSON (`requesting`/`mapped{external_port,lifetime}`/`rate_limited{retry}`/`failed{reason}`). `Failed` ne surface que la *catégorie* `reason`, jamais l'erreur brute (anti-fuite). Reset à la fin du mapping et au `disconnectTunnel`.
- JNI : nouvel export `getNatPmpStatus(): String`.
- Kotlin : nouveau provider `WarrenNatPmpStatusProvider` (lib/repository) implémenté par `WarrenQuinnStateProxy` ; l'adapter poll `getNatPmpStatus()` dans sa boucle de status ; le service forwarde ; l'UI parse (sans dépendance JSON) et affiche une ligne de statut live dans le panneau port-forwarding.
- Vérifié : `cargo check` android (Rust) + app/repository/settings compile + test `NatPmpStatusLabelTest` (parseur).

#### Multi-hop, sélection de pays entrée/sortie, ✅ FAIT

Commit : `3d0ee334cf`.

- Repository : `entryCountry`/`exitCountry` (ISO alpha-2 normalisé, persisté ; null = auto).
- Builder : la sélection de relais filtre par pays (Option B audit, Kotlin-side, sans changement de schéma Rust), précédence sortie : picker explicite > pays préféré > premier actif ; entrée : pays préféré distinct > tout actif distinct > repli. Repli auto gracieux si le catalogue ne contient pas le pays.
- UI : deux champs ISO sous le toggle multi-hop (miroir `WarrenMultiHopCountryPickers` desktop).
- Tests : builder (sélection/filtre/repli par pays) + repository (normalisation ISO-2).

### Phase D, P2, ✅ FAIT

- **Icône launcher / splash / banner TV** (commit `9234297d7c`) : remplacement du logo Mullvad (marmotte) par la marque Warren (« W » blanc sur fond darkBlue), miroir du desktop `logo-icon.svg`. Dessiné en path stroké (Android Vector ne rasterise pas `<text>`). Audit ressources : 0 string « Mullvad » résiduelle user-facing. Validé via aapt2 (`:lib:ui:resource:assembleDebug`).
- **Affordance « Reconnect now »** (commit `5e29f84ba4`) : bouton affiché dans les réglages tunnel quand le tunnel est connecté, déclenchant `WarrenQuinnReconnectInvoker` (réutilise le mnémonique caché, pas de re-prompt biométrique) pour appliquer un changement de réglage sans déconnexion manuelle.

#### Recents (exits récemment utilisés), ✅ FAIT

Commit : `90a929024a`. Item du cluster relay-lists, Kotlin-only. Repository : `recentExitIds` (5 max, most-recent-first, dédupliqué, persisté ; auto-enregistré à chaque sélection d'exit). UI : section « Recents » en tête du location picker, résolue contre le catalogue courant. Tests repository (dedup/cap/auto-record).

#### Compte / abonnement / devices / voucher, ✅ FAIT

Découverte clé : les commits concurrents sur `main` touchent **uniquement desktop/daemon/CLI** (`mullvad-cli`, `mullvad-daemon`, `mullvad-types`), **pas `android/` ni `warren-jni/`** → pas de collision sur la zone compte Android. `warren-api-client` expose déjà tout le nécessaire. Pattern JNI authentifié = `sendProblemReport` (dérive la clé du mnémonique → `WarrenApiClient::new(PROD_API_URL, key)` → `RUNTIME.block_on`).

- **Statut d'abonnement** (commit `dbe362546b`) : JNI `getSubscription(mnemonic)` → `GET /v1/subscription` signé → `{expires_at}`. Surface `WarrenSubscriptionInvoker` + use case biométrique + bouton « Check subscription » (actif/expiré + date). Test `SubscriptionLabelTest`.
- **Gestion devices** (commit `75730ae7ae`) : JNI `listDevices`/`removeDevice` → `GET`/`DELETE /v1/devices` signés (projette id/name/created_at, pas la wg key). Surface `WarrenDeviceInvoker` + use case + UI « Manage devices » (liste + remove biométrique par device). Test `DeviceLabelTest`.
- **Voucher in-app** (commit `dedd6155c3`) : JNI `redeemVoucher(mnemonic, voucher)` → `POST /v1/register` (non-signé, pubkey dérivée du mnémonique) → `{expires_at}`. Champ Crockford-32 + bouton « Redeem voucher ». Décision produit respectée (pas de Play Billing). Test voucher.

Tout vérifié : `cargo check` android (chaque JNI) + app/repo/settings compile + tests label verts.

### Faisabilité dé-risquée (suite P1)

`warren-api-client` expose **déjà** : `get_subscription()`, `list_devices()`, `get_device()`, `delete_device()`, `register_device()`, `delete_account()`. Le pont JNI a le pattern d'appel API authentifié (`sendProblemReport` : dérive la clé du mnémonique → `WarrenApiClient::new(url, key)` → `RUNTIME.block_on`). Donc **compte/devices + statut d'abonnement sont prêts à câbler** : il reste à ajouter les exports JNI (`getSubscription`, `listDevices`, `removeDevice`), le déclencheur biométrique (réutiliser `BiometricPromptAuthorizer`), un repository et l'UI. Le **voucher** (`/v1/register`) reste à confirmer côté warren-api-client (non vu dans la surface publique). Le **failover** reste bloqué par le schéma single-endpoint côté JNI (`listRelays` projette un seul endpoint « until multi-endpoint failover lands »).

### Obfuscation fine (6 méthodes), ✅ FAIT (décision révisée : parité réelle)

Commits : `d468d62a21` (Android), `eb930cc9fd` (iOS).

**Finding vérifié** (preuves file:line, traçage daemon desktop + warren-core) : les 6 méthodes d'obfuscation desktop (`SelectedObfuscation` : Auto/Off/Udp2Tcp/Shadowsocks/Quic/Lwo) sont du **legacy WireGuard/Mullvad** et **ne s'appliquent PAS au tunnel Warren** :
- L'Electron **masque** le picker 6-méthodes en mode Warren et affiche un bandeau **read-only « M4.0 HTTP/3 mimicry always-on »** (`AntiCensorshipView.tsx` L15-22, L42-67).
- Le daemon Warren (`produce_warren_tunnel_params`) ne lit **jamais** `SelectedObfuscation` ; `talpid-warren-tunnel` n'accepte qu'un `bool use_warren_obfuscation` (issu d'un JSON opérateur, pas des réglages utilisateur).
- **warren-core n'implémente aucune** des 6 méthodes (grep : Shadowsocks/UDP2TCP/LWO = 0 ligne). Le tunnel QUIC a uniquement le mimicry M4.0 **hardcodé always-on** (single-hop) + DAITA.

**Conséquence** : Android/iOS tournant sur warren-core, un picker 6-méthodes serait composé de **toggles no-op trompeurs**. La **parité réelle** = reproduire l'indicateur read-only « mimicry M4.0 always-on » (ce que l'Electron montre réellement). Décision utilisateur validée : **parité réelle, indicateur read-only**.

- **Android** : le toggle « M4.0 obfuscation » (qui écrivait le champ JNI mort `obfuscation_m40`) est remplacé par une section read-only « Anti-censorship » expliquant le mimicry always-on. `WarrenQuinnStateProxy.describe()` surface désormais `mimicry` **inconditionnellement** (always-on) au lieu de le gater sur le flag mort. Tests `WarrenQuinnStateProxyTest` mis à jour + **correction de l'échec env `Dispatchers.Main`** (setMain/UnconfinedTestDispatcher) → **6/6 verts**. `:lib:feature:settings:impl` + `:app` compilent.
- **iOS** : le picker interactif (`wireGuardObfuscation` dans `VPNSettingsDataSource`) est neutralisé **sans toucher aux enums** (zéro cascade) : section exclue du tableau principal, et l'écran obfuscation dédié (route + filter pill + connection chip) affiche `WarrenObfuscationSettingsReadOnlyView` (réutilise l'indicateur existant). **Non buildé** (env Xcode indisponible ici), relu, additif (3 fichiers), les tests UI compilent encore (échec runtime attendu, accepté). **À smoke-tester sur simulateur.** Reliquat cosmétique mineur : le « filter pill » obfuscation peut encore apparaître dans le sélecteur de localisation et mène désormais à l'indicateur read-only.

## Restant (feuille de route P1 → P2)

Ordre recommandé, dépendances notées. Estimations issues de l'audit.

| # | Item | Prio | Est. | Dépendances / risques |
|---|------|------|------|------------------------|
| 1 | **Onboarding wizard (5 étapes)** | P1 | L | Pur UI Android, mais **touche le splash + la navigation** (5 NavKeys/EntryProviders + gating + flag de complétion persisté). Sensible (flux de lancement). L'étape Subscription peut réutiliser `WarrenSubscriptionInvoker` (déjà livré). |
| 2 | **Failover (toggle + bannière « EXIT SWITCHED »)** | P1 | L | **Bloqué** : schéma single-endpoint JNI (`listRelays`) « until multi-endpoint failover lands ». Logique relay-selector présente mais non exposée. Nécessite warren-core/JNI multi-endpoint. |
| 3 | **Relay lists custom / DAITA direct-only / overrides** | P1 | XL | Support Rust à confirmer (sinon toggles non-fonctionnels). À fractionner. **L'obfuscation fine est résolue** (voir ci-dessus : parité réelle read-only, les 6 méthodes ne s'appliquent pas au tunnel Warren). |

> **Note** : le cluster compte / abonnement / voucher / devices est désormais **livré côté Android** (commits ci-dessus). Les commits concurrents sur cette thématique ne touchent que desktop/daemon/CLI, aucune collision Android constatée.

### Risques transverses (rappel)

- **Schéma Kotlin ↔ Rust** : tout nouveau champ `WarrenTunnelConfig` doit matcher le struct serde de `warren-jni/src/tunnel.rs`. Mitigation appliquée : `#[serde(default)]` systématique côté Rust + tests de roundtrip côté Kotlin.
- **Ne pas casser desktop / warren-core** : crates partagées (`warren-killswitch`, `warren-relay-selector`, `warren-natpmp-client`, `warren-api`), garder les changements additifs et rétro-compatibles.
- **Vérification de build** : les tests unitaires JVM tournent en local (`./gradlew :app:testProdDebugUnitTest`, `:lib:repository:testDebugUnitTest`). Le pont `warren-jni` (Rust) ne compile que pour la cible android → vérification via CI `build-android`. Échecs de tests préexistants et **environnementaux** : `Intent.setAction not mocked` dans `WarrenDisconnect/ReconnectUseCaseTest` (non modifiés). **`WarrenQuinnStateProxyTest` (`Dispatchers.Main`) est désormais corrigé** (setMain/UnconfinedTestDispatcher) → 6/6 verts.
- **iOS non buildable ici** : le target Xcode (`WarrenVPN.xcodeproj`) ne se compile/lance pas dans cet environnement (pas de `swift build` rapide, signing/SPM lourds). Les changements iOS sont relus + additifs mais **non vérifiés au build/runtime** → smoke-test simulateur requis. SourceKit/LSP local ne résout pas les modules cross-target (faux positifs sur `import WarrenSettings`, `Color.Warren`, `.mullvadSmall`).

## Points produit encore en attente

- **Always-on auto-connect headless** : autoriser un cache de clé non-biométrique pour reconnecter sans interaction sous always-on lockdown ? (sinon l'always-on OS protège mais l'app ne peut pas rétablir le tunnel seule).
- **Port forwarding** : compte à rebours de renouvellement rafraîchi chaque seconde (parité desktop) ou statique ?
- **Multi-hop** : picker pays = saisie ISO (parité desktop minimal) ou picker modal relay-list ?
- (Voir l'audit pour la liste complète des décisions par cluster.)
