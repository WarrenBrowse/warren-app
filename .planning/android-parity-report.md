# Rapport de parité Android — Warren VPN

**Date** : 2026-05-29
**Statut** : en cours — Phase A (audit) + Phase B (P0 privacy) livrées et vérifiées ; Phase C (P1) entamée ; Phase D (P2) à venir.

Ce rapport accompagne l'audit [`android-parity-audit.md`](android-parity-audit.md) et suit l'avancement de la mise à parité.

## Décisions produit (validées par l'utilisateur, 2026-05-29)

| Sujet | Décision |
|-------|----------|
| Kill switch / lockdown | **Always-on système (OS) + blackhole TUN applicatif** (pas de root, pas d'iptables/netd) |
| Abonnement / paiement | **Voucher in-app + statut/expiry seulement** — pas de Play Billing |
| DNS | **DNS custom + toggles de blocage de contenu côté exit, documentés** |
| Défaut IPv6 | **`false`** (parité desktop : IPv6 bloqué sauf activation explicite) |

## Livré et vérifié

### Phase B — P0 : fuites vie privée (CRITIQUE) — ✅ FAIT

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

### Phase C — P1 (entamée)

#### Port forwarding (profondeur) — ✅ FAIT

Commit : `db93d2edd9`.

- `WarrenTunnelConfig` étendu : `nat_pmp_protocol` (udp/tcp), `nat_pmp_external_port` (0=auto), `nat_pmp_lifetime_secs`.
- `warren-jni` : `maybe_spawn_nat_pmp` lit désormais protocole/port/lifetime depuis la config (avant : `Udp`/`0`/`3600` codés en dur) et les passe à `warren-natpmp-client`.
- Repository : persistance + clamps (port ∈ [49152,65535] ou 0 ; lifetime ∈ [60s, 24h]).
- UI : panneau avancé sous le toggle NAT-PMP (chips protocole UDP/TCP, champ port, presets lifetime 1h/6h/24h).
- Tests : builder + repository (protocole/port/lifetime).
- **Reste (status live)** : remonter `NatPmpStatus` live (requesting / mapped(port,countdown) / failed(reason)) nécessite un canal de callback JNI Rust→Kotlin (nouveaux exports + état) — non livré, voir « Restant ».

## Restant (feuille de route P1 → P2)

Ordre recommandé, dépendances notées. Estimations issues de l'audit.

| # | Item | Prio | Est. | Dépendances / risques |
|---|------|------|------|------------------------|
| 1 | **Port forwarding — status live** | P1 | M | Canal callback JNI Rust→Kotlin (cycle de vie au teardown). |
| 2 | **Failover (toggle + bannière « EXIT SWITCHED »)** | P1 | L | Vérifier que `ClientConfig` (warren-tunnel) accepte `failover_enabled` ; surface JNI du compteur d'événements. Logique relay-selector déjà présente. |
| 3 | **Compte / keys / devices** | P1 | L | Nouveaux exports JNI `listDevices`/`removeDevice`/`getWalletPubkey` (warren-api-client + tokio) ; gate biométrique. |
| 4 | **Abonnement (voucher + statut)** | P1 | L→XL | JNI `/v1/register` (voucher Crockford-32) + `/v1/subscription` (expiry). Pas de Play Billing (décidé). Partage la surface compte avec #3. |
| 5 | **Onboarding wizard (5 étapes)** | P1 | L | Pur UI Android ; l'étape Subscription dépend de #4 ; flag de complétion persisté. |
| 6 | **Multi-hop — country pickers entrée/sortie** | P1 | XL | Spike `warren-relay-selector` (filtrage par pays). Option Kotlin-side d'abord. |
| 7 | **Relay lists custom / recents / obfuscation fine / DAITA direct-only / overrides** | P1 | XL | Support Rust des 6+ méthodes d'obfuscation + filtre DAITA direct-only. À fractionner. |
| 8 | **Icône launcher Warren** (remplacer le placeholder Mullvad) | P2 | M | Assets `mipmap/ic_launcher*` — nécessite l'asset graphique Warren. |
| 9 | **Affordance « Reconnect now »** au changement de flag pendant connexion | P2 | M | `WarrenReconnectUseCase` existe ; détecter le changement pendant `Connected`. |

### Risques transverses (rappel)

- **Schéma Kotlin ↔ Rust** : tout nouveau champ `WarrenTunnelConfig` doit matcher le struct serde de `warren-jni/src/tunnel.rs`. Mitigation appliquée : `#[serde(default)]` systématique côté Rust + tests de roundtrip côté Kotlin.
- **Ne pas casser desktop / warren-core** : crates partagées (`warren-killswitch`, `warren-relay-selector`, `warren-natpmp-client`, `warren-api`) — garder les changements additifs et rétro-compatibles.
- **Vérification de build** : les tests unitaires JVM tournent en local (`./gradlew :app:testProdDebugUnitTest`, `:lib:repository:testDebugUnitTest`). Le pont `warren-jni` (Rust) ne compile que pour la cible android → vérification via CI `build-android`. 10 échecs de tests préexistants et **environnementaux** (`Intent.setAction not mocked` dans `WarrenDisconnect/ReconnectUseCaseTest` ; `Dispatchers.Main` non initialisé dans `WarrenQuinnStateProxyTest`) — fichiers non modifiés, sans rapport avec ces changements.

## Points produit encore en attente

- **Always-on auto-connect headless** : autoriser un cache de clé non-biométrique pour reconnecter sans interaction sous always-on lockdown ? (sinon l'always-on OS protège mais l'app ne peut pas rétablir le tunnel seule).
- **Port forwarding** : compte à rebours de renouvellement rafraîchi chaque seconde (parité desktop) ou statique ?
- **Multi-hop** : picker pays = saisie ISO (parité desktop minimal) ou picker modal relay-list ?
- (Voir l'audit pour la liste complète des décisions par cluster.)
