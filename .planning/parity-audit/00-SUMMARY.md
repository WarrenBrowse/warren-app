# Audit de parité Warren — Electron ↔ Android ↔ iOS

**Date** : 2026-05-31
**Référence** : l'app desktop Electron (`desktop/packages/mullvad-vpn/`) = source de vérité features/UX.
**Cibles** : Android (`android/`, Kotlin/Compose) et iOS (`ios/`, Swift, target WarrenVPN).
**Méthode** : 7 sous-audits parallèles, preuves file:line, dans ce dossier (`01-connection` … `07-branding-ux`).

> Les différences d'**architecture** sont attendues et ne sont pas des écarts : mobile n'a pas de daemon gRPC, utilise le tunnel QUIC warren-core + un compte wallet Warren (mnémonique BIP39) au lieu d'un numéro de compte Mullvad. On audite la **parité user-facing** (features + UX), pas l'implémentation. Les éléments intrinsèquement desktop (tray système, lancement au login, auto-updater) ou mobile (VPN always-on, biométrie) ne sont pas comptés comme écarts.

---

## Insight central

Les deux apps mobiles échouent la parité de **manière opposée et complémentaire** :

- **Android = la plomberie sans la présentation.** Le câblage backend est souvent là (JNI signés `/v1/subscription`, `/v1/register`, `/v1/devices` ; config tunnel complète atteint `WarrenTunnelConfigBuilder`), mais l'UI/UX est un échafaudage dev : pas de home compte réel, picker de localisation minimal, pas d'onboarding, et **le pipeline d'état de l'écran principal est lossy**. C'est la couche tunnel mobile **la plus propre** mais l'expérience la plus brute.

- **iOS = la présentation sans les données Warren.** L'UI Mullvad upstream a été conservée quasi-intégralement (panneau de connexion riche, SelectLocation complet, écrans compte), mais **branchée sur le mauvais modèle** : numéro de compte Mullvad + StoreKit IAP au lieu du wallet/`/v1/subscription`, et plusieurs réglages-clés Warren (DAITA, NAT-PMP) sont des **scaffolds morts**.

Conséquence stratégique : **Android a besoin d'UI/UX par-dessus son backend ; iOS a besoin de re-câbler son UI existante sur les données Warren + finir les scaffolds morts.**

---

## État final (mise à jour après implémentation)

### Android — parité **essentiellement complète** (tout le périmètre vérifiable livré)
Livré + vérifié (tests unitaires verts, compile vérifié) : P0-1 état carte connexion · P0-6 logo W · fuites privacy (IPv6/DNS/killswitch) · allow-LAN (split-route CIDR anti-fuite) · MTU configurable · picker (recherche/groupement pays/recents toggle) · port-forwarding (config+status) · multi-hop pays · **compte complet** (SS58/mnémonique/subscription+expiry/voucher/devices — l'audit le qualifiait à tort de « scaffold ») · **cache expiry + statut proactif** · **bannière expiry sur écran Connect** · **onboarding welcome wizard** · obfuscation read-only · reconnect · consolidation surfaces d'état.
⚠️ À smoke-tester sur device : rendu nav onboarding (logique vérifiée par test).

### iOS — partiellement livré ; le reste nécessite **Xcode**
- **Livré + vérifié** : obfuscation read-only (parité desktop) ; **DAITA fonctionnel** (un-stub FFI warren-ios `with_daita`+`pump_bidirectional_with_daita`, **vérifié `cargo check --target aarch64-apple-ios`** + lecture du réglage persisté côté actor) ; **app-icon déjà complet** (set Warren light/dark/tinted — corrige P1-9).
- **Bloqué Xcode (gros refactors aveugles, non-vérifiables sans build)** :
  - **Compte/voucher/subscription (P0-2/P0-3)** : tout est encore sur le modèle Mullvad mort (numéro de compte, StoreKit IAP, `MullvadAPIProxy.submitVoucher` **stubbé**, expiry depuis `deviceState` pas `/v1/subscription`). **Aucun FFI subscription iOS n'existe** (contrairement à Android). Re-pointer = nouveau FFI signé warren-ios (vérifiable Rust) **+ gros refactor Swift des écrans compte** (invérifiable sans Xcode). Exposer le voucher seul = toggle non-fonctionnel (backend stubbé).
  - **NAT-PMP (P0-5)** : FFI `warren_natpmp_ffi` squelette ; nécessite impl Rust **+ acteur Swift d'orchestration** (request/renew/release sur cycle de vie tunnel + App Group + timers) — invérifiable sans device.
  - **Content-blockers (P1-6)** : nécessite un nouveau struct DNS dans le FFI tunnel + marshalling Swift + support warren-tunnel (incertain) → dépendance warren-core possible.
- **Note** : les autres `let _ = params.X` du FFI (multi_hop_relay, nat_pmp_enabled, bypass_cidrs) ne sont **pas des mirrors propres** — warren-jni (Android) les ignore AUSSI (dead-code des deux côtés). Seul DAITA était un vrai mirror (fait).

**Conclusion** : la parité Android est faite à hauteur du livrable+vérifiable. Le reliquat substantiel est **iOS** et requiert un environnement **Xcode buildable** pour être complété sans casser l'app (refactor compte/subscription, orchestration NAT-PMP, marshalling Swift des nouveaux FFI). Plan d'exécution précis disponible dans l'historique de cette mission.

---

## Backlog priorisé

### 🔴 P0 — bloquants (sécurité, données fausses, ou cœur cassé)

| # | Plateforme | Écart | Réf |
|---|-----------|-------|-----|
| ~~P0-1~~ ✅ | **Android** | **CORRIGÉ** (commit `bd6340c4b8`) : nouveau `WarrenConnectedInfo` typé + `connectedInfo` sur le provider ; `ConnectionProxy` mappe désormais `Failed`→`Error(non-blocking)` (« ERROR STATE »), `Blocking`→`Error(blocking)` (« BLOCKED CONNECTION »), et `Connected`→endpoint exit/entry réel + chips DAITA/MULTIHOP/DAITA_MULTIHOP/QUIC. Tests : `ConnectionProxyTest` 7/7 + `WarrenQuinnStateProxyTest` 6/6 + `ConnectViewModelTest` 10/10 (vérifiés en worktree isolé). | 01-connection |
| P0-2 | **iOS** | **Surface compte = Mullvad non modifié** : affiche le *numéro de compte*, « Add time » via **StoreKit IAP**, restore purchases, delete account ; lit l'expiry depuis `deviceState.accountData?.expiry` au lieu du backend Warren `/v1/subscription`. Le wallet existe en onboarding/settings mais les écrans compte/abonnement/voucher n'ont jamais été re-pointés dessus. | 02-account |
| P0-3 | **iOS** | **Voucher = DEBUG-only** : l'UI RedeemVoucher n'est atteignable que via la feuille Debug `#if DEBUG`. Aucune entrée en production. | 02-account |
| ~~P0-4~~ ✅ | **iOS** | **CORRIGÉ** (`c0319c7485` Rust vérifié ios-target + `e9deb5de85` Swift) : le FFI warren-ios ignorait `daita_spec` (`let _ = ...`) → un-stubbé (`with_daita` + `pump_bidirectional_with_daita`, mirror warren-jni). `WarrenQuinnActor` lit le réglage DAITA persisté → passe le spec. Note : `WarrenDaitaSettingsView` était un scaffold mort inutilisé ; la vraie vue `SettingsDAITAView` persiste déjà. Swift non-buildé (Xcode absent) mais API-vérifiée. | 03-vpn-settings |
| P0-5 | **iOS** | **NAT-PMP / port forwarding = scaffold mort** (`WarrenNatPmpSettingsView`) : pas de persistance, pas de FFI, statut placeholder. Différenciateur phare, fonctionnel sur desktop + Android. | 03-vpn-settings |
| ~~P0-6~~ ✅ | **Android** | **CORRIGÉ** (commit `5edc8a5254`) : les 20 PNG marmotte (`logo_icon`/`launch_logo`/`small_logo_{black,white}` × 5 densités) remplacés par 4 vector drawables « W » Warren (résolution-indépendants) couvrant splash, top bar, tile, notifications, manifest. Vérifié : `processProdDebugResources` vert (worktree). | 07-branding-ux |

### 🟠 P1 — features/UX importantes manquantes

| # | Plateforme | Écart | Réf |
|---|-----------|-------|-----|
| P1-1 | **Android** | **Pas de home compte réel** : abonnement/voucher/devices dans un scaffold dev (`WarrenWalletSettingsScreen`, EN hardcodé), pas de temps-restant, pas de lien achat-crédit. `AccountRepository` legacy = stub mort. | 02-account |
| P1-2 | **Android** | **Pas d'écran out-of-time / expiré, ni notification d'expiration** : un abonnement échu est invisible. `InAppNotification` n'a pas de variante AccountExpiry. Logout = no-op. | 02-account |
| P1-3 | **Android** | **Pas d'onboarding wizard** (desktop + iOS ont 5 étapes : Welcome → Wallet → Subscription → Preferences → Done). Splash va PrivacyDisclaimer → Wallet nu → Connect. Les primitives wallet existent ; il manque le chrome wizard + étapes welcome/subscription/done. | 05-onboarding |
| P1-4 | **Android** | **Picker de localisation très en-dessous de la parité** : liste plate des exits, **pas de hiérarchie pays→ville→serveur, pas de recherche, pas de sélection entrée/multihop**. (Correction : l'hypothèse « Android a déjà entrée/sortie » est fausse pour le picker actuel — seul l'exit est sélectionnable ; seul *recents* est présent.) | 04-location |
| P1-5 | **iOS** | **Pickers WireGuard legacy encore vivants** : sélecteur « QUIC port »/port custom dispatche de vrais updates. (L'obfuscation, elle, a été neutralisée par le commit `eb930cc9fd` — à confirmer au build.) | 03-vpn-settings |
| P1-6 | **iOS** | **Content blockers DNS absents** (l'UI DNS est custom-resolver seulement) ; **multi-hop se termine probablement en cul-de-sac** dans le proto `WarrenSettings` legacy que le tunnel QUIC ne lit pas. | 03-vpn-settings |
| P1-7 | **Android** | **Auto-connect / lockdown screen orphelin** (`AutoConnectNavKey` enregistré, jamais navigué) ; le boot receiver auto-connecte sans gate. **Pas de contrôle LAN / réseau local**. | 03-vpn-settings |
| P1-8 | **Android** | **Langue** gated Android 13+ et enterrée sous « Appearance » (pré-13 ne peut pas changer) ; **pas de switch master notifications** in-app. | 06-app-settings-support |
| P1-9 | **iOS** | **App icon = placeholder** (wordmark « WARREN » rogné, décentré). Warren-brandé mais non fini. | 07-branding-ux |
| P1-10 | **iOS** | **Étape Subscription onboarding sans vérification** (juste un lien Safari + « Maybe later »), vs desktop qui poll `updateAccountData()` 10s×2min. Toggles privacy onboarding **silencieusement jetés** au finish (bug UX). | 05-onboarding, 02 |
| P1-11 | **Incohérent** | **API access** : retiré sur Android (no-op), **encore affiché sur iOS** (`SettingsDataSource.swift:285`) et desktop. Décision produit appliquée inégalement. | 06-app-settings-support |

### 🟡 P2 — finition / cohérence

| # | Plateforme | Écart | Réf |
|---|-----------|-------|-----|
| P2-1 | Android | Picker localisation : pas de custom lists, pas de drapeaux, pas de toggle recents. | 04-location |
| P2-2 | Android | 3 surfaces d'état qui se chevauchent (snackbar + bannière + carte) ; lignes reconnect-count/age absentes. | 01-connection |
| P2-3 | iOS | 2 fuites « WireGuard » en locale fi/ru ; manque locales ar/ro/fa. | 07-branding-ux |
| P2-4 | Android | Assets « mole » disguise-icon morts (feature non câblée). | 07-branding-ux |
| P2-5 | Les deux | Beta opt-in absent ; « replay onboarding » seulement desktop ; iOS a `WarrenAboutView` (privacy/terms/AGPL) qu'Android n'a pas. | 06-app-settings-support |
| P2-6 | Android | Pas de MTU. | 03-vpn-settings |

---

## Décisions produit nécessaires (à trancher avant implémentation)

1. **Quantum-resistant tunnel** : reliquat Mullvad WireGuard PQ, gardé actif desktop (défaut `true`, relabellé « QUIC tunnel ») + iOS, **droppé Android**. Applicabilité réelle au transport QUIC Warren non vérifiée → décider explicitement (garder partout / retirer partout).
2. **Schéma de couleurs** : les 3 plateformes gardent le bleu/vert Mullvad ; le jaune Warren (#FFD524) est défini identiquement partout mais quasi non-câblé. Cohérent entre plateformes → ressemble à une décision « chrome bleu foncé + accent jaune ». **Confirmer l'intention** avant de traiter comme défaut.
3. **Filtres provider/ownership** (localisation) : présents EL+iOS, mais le schéma relay Warren n'a pas ces champs. Garder (et alimenter) ou retirer ?
4. **Filtres obfuscation/DAITA/QUIC/LWO** dans la sélection de localisation (EL + iOS) : concepts WireGuard non-fonctionnels pour Warren → candidats au retrait.
5. **API access** : décision Warren (endpoint hardcodé) à appliquer uniformément → retirer aussi sur iOS/desktop ?
6. **DNS content-blockers iOS** : les ré-implémenter (parité Android/desktop) ou les déléguer exit-side différemment ?

---

## Synthèse par plateforme

**Android** — Corriger en priorité : (a) pipeline d'état écran principal P0-1, (b) logo marmotte P0-6, (c) home compte + expiré + onboarding (P1-1/2/3), (d) picker localisation (P1-4). Le backend est prêt ; le travail est majoritairement **UI/UX + branchement présentation**.

**iOS** — Corriger en priorité : (a) re-pointer le compte sur le wallet + `/v1/subscription`, retirer StoreKit, sortir le voucher du DEBUG (P0-2/3), (b) finir DAITA + NAT-PMP scaffolds morts (P0-4/5), (c) retirer pickers WireGuard legacy restants + ajouter content-blockers (P1-5/6). Le travail est majoritairement **re-câblage de données + finition de scaffolds**. ⚠️ iOS non buildable dans cet environnement → toute modif iOS à smoke-tester sur simulateur.

**Commun** — Décisions produit ci-dessus, branding (couleurs/jaune), beta/replay-onboarding/about.
