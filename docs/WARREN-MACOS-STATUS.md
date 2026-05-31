# Warren macOS — état, diagnostic, et chantiers prod

> Mis à jour : 2026-05-31 (v1.0.4). Document de référence sur l'état réel
> du client Warren sur macOS, la cascade de bugs diagnostiquée pendant la
> mise au point des releases v1.0.x, et ce qu'il reste à faire avant
> qu'un utilisateur lambda puisse l'utiliser.

## TL;DR

- ✅ **Le tunnel Warren macOS fonctionne de bout en bout** (prouvé sur vrai
  matériel le 2026-05-30) : `Connected`, route par défaut via `utunX`,
  trafic qui circule, IP publique = l'exit (`204.168.207.130`, Allemagne/Kassel).
  C'était la première validation réelle du data-path macOS (jusque-là
  «pas encore exercé en daemon réel Mac»).
- ✅ Les builds desktop (macOS/Linux/Windows) sont verts et publiables.
- ⚠️ **Un utilisateur lambda ne peut PAS encore connecter sur une install
  propre** : l'obtention d'une identité + l'enrôlement/abonnement ne sont
  pas câblés en prod (cf. § Chantiers). On a fait marcher le tunnel via des
  contournements manuels (identité dev posée à la main + mode local-account).

## La cascade « ça connecte mais plus d'internet » (résolue)

Ce n'était NI un bug de routage, NI de packaging. C'était une chaîne :

1. **App lancée contre un vieux daemon dev** (`1.0.0-dev`) resté en mémoire
   → message « application désynchronisée » (GUI 1.0.x ≠ daemon dev).
2. **Daemon v1.0.2 incapable de lire son identité dans le Keychain**
   → `OSStatus -25308 errSecInteractionNotAllowed`. Le Keychain legacy lie
   un item à la signature de code du binaire créateur ; nos builds sont
   non signés / ad-hoc (pas de Developer ID stable), donc la signature
   change à chaque build et le daemon ne peut pas relire un item écrit par
   un binaire précédent → pas de clé de signature.
3. **Sélection d'exit en échec** quand on choisit un serveur précis :
   bug de casse (`city kassel` côté requête vs `Kassel` côté relay,
   comparaison sensible à la casse). Contournable en choisissant le *pays*.
4. **Handshake refusé par l'exit** : une identité fraîche aléatoire (générée
   quand le Keychain est vidé) n'est **pas enrôlée** sur l'exit → l'exit
   ferme la connexion (`read SetupAck: connection lost`).
5. Tant que le tunnel ne monte pas, **le killswitch bloque tout le trafic**
   (par design, anti-fuite) = « plus d'internet ». Ce n'est pas un bug
   séparé, c'est la conséquence directe du handshake en échec.

**Déblocage** : charger l'**identité dev déjà enrôlée** dans le daemon prod
(via le stockage plaintext) + sélectionner le pays. → handshake OK → tunnel
up → internet via l'exit.

## Corrigé dans le code (v1.0.3)

| Fix | Repo / commit | Effet |
|---|---|---|
| Sélecteur **city-case** | warren-core `936c208` | Choisir un exit *précis* matche (plus seulement le pays). `eq_ignore_ascii_case` sur la ville. TDD RED→GREEN. |
| **Identité macOS persistante** | warren-app `5114907` (`dist-assets/pkg-scripts/postinstall`) | Le daemon stocke l'identité dans un fichier `0600` root (`WARREN_USE_PLAINTEXT_STORAGE=1` dans le LaunchDaemon livré). Les builds non signés gardent leur identité d'une mise à jour à l'autre au lieu du `-25308`. À retirer quand on aura une signature Developer ID + l'entitlement `keychain-access-groups`. |

Le fix city-case arrive dans le build via `.warren-core-version` (déjà à
`f36a814`, qui inclut `936c208`).

### v1.0.4 - le champ « bon » de la GUI ne ment plus

| Fix | Repo / fichier | Effet |
|---|---|---|
| **Redemption de voucher réelle en mode local** | warren-app `mullvad-daemon/src/device/account_backend.rs` | En mode local-account, `LocalAccountBackend::submit_voucher` faisait un **stub bidon** : il ignorait le code (`_voucher`), ne contactait aucun backend, et renvoyait toujours « OK +100 ans » pour n'importe quelle saisie, à l'infini. La GUI affichait donc « compte approvisionné » alors que **rien n'était enrôlé** → l'exit refusait le handshake → internet bloqué. Désormais `submit_voucher` fait le **vrai** `POST /v1/register` (non-signé, pubkey en clair) comme le backend remote, et **fail-closed** : un bon invalide / déjà utilisé remonte une vraie erreur au lieu d'un mensonge. Test de non-régression `local_submit_voucher_fails_closed_instead_of_fabricating_success`. |

### v1.0.4 - défaut en mode remote (fin des stubs trompeurs)

| Fix | Repo / fichier | Effet |
|---|---|---|
| **B1 - URL warren-api par défaut** | warren-app `mullvad-daemon/src/warren_remote_config.rs` | `resolve()` utilise désormais `https://api.warrenbrowse.com` (const `DEFAULT_WARREN_API_URL`) quand ni l'env `WARREN_API_URL` ni `Settings::warren_api_url` ne fournit de valeur non-vide. Avant : pas d'URL → fallback silencieux vers `api.mullvad.net` (qu'on n'opère pas). Plus besoin de `warren warren api-url set …` à la main. Tests réécrits (`no_url_anywhere_uses_compiled_default`, `empty_url_uses_compiled_default`, `empty_env_url_falls_through_to_settings_url`). |
| **B2 - défaut local-account = false** | warren-app `mullvad-types/src/settings/mod.rs` | Toute install fraîche démarre désormais sur le **vrai** backend warren-api (abonnement + voucher + enrôlement device réels), plus sur le stub POC. Élimine le « 99 ans » trompeur (#2) et route « pas d'abonnement » vers l'état compte honnête (404 → arrêt de la boucle, pas « no relay »), ce qui adresse le **cas courant de #4**. Test `default_is_remote_backend_not_local_account_stub`. ⚠️ À valider E2E (connexion fresh-install en remote) avant release. |

#### Les 4 incohérences révélées (même cause racine)

Le mode local-account est un raccourci POC qui **simule trop de choses**. Il
est en plus **actif par défaut** sur toute install fraîche
(`mullvad-types/src/settings/mod.rs` : `warren_local_account: true` dans le
`Default`, alors que le doc-comment du champ dit « Default false » - à
réconcilier). Conséquences observées par l'utilisateur :

1. ✅ **CORRIGÉ v1.0.4** - Champ « bon » = stub bidon (n'enrôlait rien,
   « validé » pour tout code). `submit_voucher` fait le vrai `/v1/register`.
2. ✅ **CORRIGÉ v1.0.4 (B2)** - « 99 ans » affiché. Le défaut passe en mode
   remote → `get_data` renvoie la vraie expiration. Le stub local
   (`Utc::now() + 100 ans`) n'est plus actif que si on l'active
   explicitement (`local-account set on` / `WARREN_LOCAL_ACCOUNT=1`).
3. ⚠️ **La désinstallation détruit l'identité** - `uninstall_macos.sh` fait
   `rm -rf /etc/warren-vpn` → la pubkey enrôlée est perdue → chaque réinstall
   génère une **nouvelle** pubkey non enrôlée, et l'ancien abonnement reste
   orphelin sur une clé morte. À corriger : sauvegarder/exporter l'identité
   hors du dossier wipé, ou proposer export/import de mnemonic.
4. ✅ **CORRIGÉ** - Message d'erreur trompeur, sur deux fronts :
   - Le **cas courant** (pas d'abonnement) est résolu par B2 : en remote,
     le 404 warren-api remonte comme un état compte honnête, plus comme
     `NoMatchingRelay`.
   - Le **rejet au handshake** : l'exit fermait déjà la connexion avec le
     code applicatif `WARREN_AUTH_FAILED` (0x57415252). Le **client** le
     reconnaît maintenant via `Connection::close_reason()` et renvoie
     `TunnelError::AuthRejected` (warren-core `client.rs`), que
     `talpid-warren-tunnel` mappe en `Error::BackendFatal`
     (**non-retryable**) au lieu de `Error::Handshake` (retry →
     `NoMatchingRelay`). Le state machine entre donc en ErrorState clair
     au lieu de boucler sur un message trompeur. Tests RED→GREEN :
     `d2_allowlist::non_allowlisted_client_handshake_returns_auth_rejected`
     (warren-core) + `auth_rejected_maps_to_fatal_non_recoverable_error`
     (warren-app).
   - **Pas de bench Hetzner requis** : changement purement client sur le
     chemin de handshake *échoué* (aucun tunnel monté), zéro impact wire
     format / pump / data-plane. Le code de close existait déjà côté exit.

#### Effets de bord traités dans la même passe

- **Dérive natpmp `rate_limit`** : le bump `.warren-core-version` → f36a814
  (fait pour le city-case) avait tiré l'ajout du champ
  `Response::Map.rate_limit` dans warren-core **sans** mettre à jour les 5
  call-sites de `talpid-warren-tunnel` → warren-app ne compilait plus (le
  build v1.0.4 annulé aurait échoué ici). Corrigé : `rate_limit: None` sur
  les 5 réponses Map.
- **Cohérence warren-core** : `.warren-core-version` bumpé à `a8c2fde`
  (inclut `AuthRejected`), warren-core poussé, garde-fous Cargo.lock
  (pin quinn-fork + `cargo metadata --locked`) vérifiés OK.

### Connectivité macOS : « connecte mais pas d'internet » + crash split-tunnel (déclenché par le split tunneling)

Signalé par **2 utilisateurs**, exactement à l'**activation du split tunneling** macOS
(donc pas un hasard). Diagnostic (sur vrai Mac) :

- Symptôme : tunnel `Connected`, routes posées, mais `pump_metrics` montre
  `uplink>0 / downlink=0` → le trafic sort, rien ne revient → pas d'internet.
- Cause routing : la route /32 de l'exit était pinnée sur **utun4 (le tunnel)**
  au lieu de l'interface physique → l'exit était routé *dans* le tunnel →
  boucle → downlink=0. `route -n get default` renvoie le tun une fois le
  split-default posé, donc la détection `default_iface` prenait `utun4`.
- Déclencheur : le **split tunneling macOS** (Endpoint Security + BPF + utun
  dédié) ne peut pas fonctionner sur un **build non signé** (ES exige
  Developer ID + entitlement + Full Disk Access). Il s'initialise à moitié
  → routage corrompu + crash de la GUI au quit.

**Corrigés :**

| Fix | Repo / fichier | Effet |
|---|---|---|
| **Détection d'interface résistante au tunnel** | warren-core `crates/warren-client/src/default_route_split_macos.rs` (`1d9d950`) | `discover_default_iface` pinne la route de l'exit sur la NIC **physique** via `scutil` PrimaryInterface quand `route get default` renvoie un tun. Refuse de pinner sur un tunnel (erreur claire plutôt que tunnel-sur-lui-même). 6 tests TDD (`resolve_falls_back_to_scutil_when_route_is_a_tunnel`, …). |
| **Gating du split tunneling macOS sur build non signé** | warren-app `mullvad-daemon` (`lib.rs` + `Cargo.toml`) | Le daemon **refuse** d'activer le ST (`MacosSplitTunnelUnsupported`) AVANT tout init ES/BPF, sauf si build signé (feature `macos-split-tunnel`, OFF par défaut). Désactiver/clear restent permis. Élimine crash + breakage à la source. Tests `unsigned_macos_build_reports_split_tunnel_unsupported`, `enabling_split_tunnel_is_refused_on_unsigned_macos`. |

Workaround manuel sur un build non corrigé (v1.0.3) : après connexion,
`sudo route -n add -host <exit_ip> <gateway_physique>` (auto-détectable via
`scutil` + le log daemon). À relancer à chaque repaire reconnexion ; le fix
`1d9d950` le rend automatique.

### Chantiers restants

- **#3 - Persistance d'identité** : 🟡 largement adressé.
  - Le **flow GUI existe déjà** et est routé (`AppRouter.tsx`) :
    `KeysView` (révéler/sauvegarder la phrase), `RestoreMnemonicView`
    (importer/restaurer), `OnboardingWalletView` (générer/importer). Le user
    ne le voyait pas car le local-mode (faux compte) ne déclenchait pas
    l'onboarding ; **B2 corrige ça** (fresh install → remote → onboarding).
  - **CLI ajouté** (`warren warren mnemonic export` / `import <mots…>`,
    `mullvad-cli/src/cmds/warren.rs` + wrappers `client.rs`) : permet de
    sauvegarder la phrase AVANT une désinstallation et de la réimporter
    après, sans dépendre de la GUI. Normalisation testée en TDD.
  - **Reste** : l'uninstaller (`rm -rf /etc/warren-vpn`) n'oblige/rappelle
    pas la sauvegarde. Amélioration UX possible (prompt de backup au
    désinstall), mais l'identité est désormais récupérable via export/import.
- **#4 (protocole)** : code de close `WARREN_AUTH_FAILED` côté exit
  (warren-core `warren-tunnel`) + mapping client + état GUI. Exige bench
  Hetzner avant commit (CLAUDE.md §1). Non fait ici.

## Chantiers PROD restants (un utilisateur lambda ne peut pas encore connecter)

1. **Onboarding identité (GUI)** : il n'existe aucun écran pour générer /
   importer / sauvegarder une identité Warren (mnemonic BIP39). Les
   handlers gRPC existent (`get/set_warren_mnemonic`) mais ne sont pas
   câblés dans un flow de première utilisation. Aujourd'hui : fichier
   mnemonic posé à la main.
2. **Abonnement + enrôlement** : pas de flow paiement → enrôlement de la
   pubkey sur l'exit. En remote, le backend renvoie `404 no subscription`
   et l'exit refuse une pubkey non enrôlée. Contourné via le mode
   **local-account** (compte local en mémoire, faux « 99 ans » d'expiration)
   + une identité déjà enrôlée.
3. **Signature Developer ID** : sans signature stable, le System Keychain
   est inutilisable (cf. cascade §2) → d'où le fallback fichier plaintext.
   Vrai correctif = signer (bloqué par société / D-U-N-S). Une fois signé,
   réactiver le backend Keychain et retirer `WARREN_USE_PLAINTEXT_STORAGE`.
4. **Routage macOS** : désormais **validé** sur vrai Mac (utun + split
   default + IP exit confirmée). À garder sous surveillance (le code
   n'émettait pas d'erreur si l'install des routes échouait — voir
   `talpid-warren-tunnel/src/lib.rs`, l'event `Up` est émis même si
   `add_routes` / `DefaultRouteSplitGuard::install` échouent en silence ;
   à durcir : passer en erreur visible plutôt que « connecté mais sans net »).

## Comment refaire marcher le tunnel sur un Mac de test (contournements)

Daemon = LaunchDaemon root `com.warrenbrowse.vpn.daemon`, logs dans
`/var/log/warren-vpn/daemon.log`, settings dans `/etc/warren-vpn/`.

1. **Stockage plaintext** (sinon `-25308`) — env du LaunchDaemon :
   `WARREN_USE_PLAINTEXT_STORAGE=1` (déjà injecté par le postinstall v1.0.3).
2. **Identité enrôlée** : poser la mnemonic dev (déjà enrôlée sur l'exit)
   dans `/etc/warren-vpn/secrets/warren_mnemonic.txt` (`0600 root`).
   ⚠️ Ne jamais committer/afficher de mnemonic.
3. **Mode local-account** (bypass abonnement) :
   `warren warren local-account set on` puis redémarrer le daemon.
4. **Localisation = pays** (tant que la GUI envoie une contrainte ville) :
   `warren relay set location de`.
5. Redémarrer le daemon :
   `sudo launchctl bootout system/com.warrenbrowse.vpn.daemon ; sudo launchctl bootstrap system /Library/LaunchDaemons/com.warrenbrowse.vpn.daemon.plist`
6. Vérifs : `warren status` → `Connected` ; `route -n get 8.8.8.8` →
   `interface: utunX` ; `curl https://api.ipify.org` → IP de l'exit.

## Pièges connus

- Un **daemon dev** (`target/debug/warren-daemon`) lancé en parallèle peut
  squatter le socket `/var/run/warren-vpn` et masquer le daemon installé
  → « désynchronisé ». Le tuer avant de tester l'install.
- **Mullvad VPN** installé en parallèle (`net.mullvad.daemon`) partage `pf`
  / les routes → conflits. Le couper pour tester Warren.
- Le « **99 ans** » de forfait = stub du mode local-account, pas un vrai
  abonnement.

---

## Audit 2026-05-31 — DNS loop fix + leak-tightness review

### Root cause "connecté mais pas d'internet" (macOS)

Le tunnel **forwarde bien l'IPv4** (capture exit : trafic bidirectionnel
réel, NAT OK, source 10.66.0.4 correcte). Le blocage était **DNS** :

```
WARN hickory: ignoring response from 127.245.179.107:53
     because it does not match name_server: 10.66.0.1:53
```

Le resolver DNS local (dans le daemon, `127.x`) forwarde vers `10.66.0.1`,
mais la règle pf anti-leak qui redirige tout le port 53 **renvoie ses
propres requêtes upstream vers lui-même** → hickory rejette toutes les
réponses → aucune résolution → "pas d'internet" (et `api.warrenbrowse.com
unreachable` en cascade).

### Fixes livrés (TDD, fmt+clippy+tests verts)

| Fix | Fichier | Test |
|---|---|---|
| Resolver DNS local OFF par défaut (Warren) → DNS système direct sur 10.66.0.1 | `talpid-core/src/resolver.rs` | 3 tests |
| `enable_ipv6` défaut `true`→`false` (Warren IPv4-only ; sinon fuite IPv6) | `mullvad-types/src/settings/mod.rs` | 1 test |
| `_metrics_task` détaché immortel → handle aborté au teardown | `talpid-warren-tunnel/src/lib.rs` | compile |
| Leak-checker faux positif gaté pour `TunnelType::Warren` (cf. Findings #1) | `mullvad-daemon/src/leak_checker/mod.rs` | 2 tests |

`TALPID_DISABLE_LOCAL_DNS_RESOLVER=0` ré-active le resolver local (opt-in).

### Audit leak (3 plateformes, vérifié par lecture de code)

En état **Connected**, le pare-feu bloque par défaut tout trafic
non-tunnel (IPv4, IPv6, DNS) sauf : endpoint exit, tun, et LAN si
`allow_lan` :

- **macOS** (pf) : règles catch-all `Drop` sans filtre `af`
  (`firewall/macos.rs:234-245`) → IPv4+IPv6 droppés ; DNS bloqué sauf
  10.66.0.1 scopé tun (`get_block_dns_rules` + `get_allow_tunnel_dns_rules_when_connected`).
- **Linux** (nftables) : table `Inet`, `out_chain` policy `Drop` +
  reject terminal (`firewall/linux.rs:300,730-737`). `enable_ipv6` ne
  touche pas le pare-feu (no-op Linux).
- **Windows** (WFP) : `baseline::BlockAll` (v4+v6) + sublayer DNS
  `BlockAll` → "smart multi-homed resolution" neutralisé structurellement.

Verdict : **leak-tight IPv4/IPv6/DNS sur les 3 plateformes en config par
défaut.** Mes changements n'introduisent aucune régression (captive-portal
préservé : le resolver est démarré inconditionnellement, le flag ne change
que l'état Connected).

### Findings

1. **Leak-checker faux positif — CORRIGÉ (fix #4)** : `mullvad_leak_checker`
   faisait un traceroute vers l'endpoint exit sur l'interface physique
   (`leak_checker/mod.rs`). Le transport QUIC userspace de Warren sort
   légitimement sur en0 pour joindre l'exit → le traceroute atteint le
   routeur LAN (1er hop) → `Network leak detected! Please contact Warren
   support` à **chaque** connexion. **Jamais une fuite de trafic user** (le
   pare-feu bloque tout le reste — c'est l'enforcement réel). Fix :
   `leak_test_applies_to(tunnel_type)` skippe le test pour
   `TunnelType::Warren` (le test reste actif pour WireGuard). Le pare-feu
   kill-switch n'est PAS touché. 2 tests TDD.
2. **Custom DNS : PAS de fuite (finding initial de l'agent erroné)** : la
   partition `mullvad-daemon/src/dns.rs:55-62` envoie le DNS custom
   **public** vers `tunnel_config` (routé in-tunnel) et le DNS custom
   **privé** vers `non_tunnel_config` (résolveur LAN explicite, LAN
   seulement). Confirmé par le test `test_custom_dns` + `is_local_address`
   (`firewall/mod.rs:70`). Aucune fuite DNS, ni en défaut ni en custom
   public. RAS.
3. **`enable_ipv6` stocké** : le défaut corrigé ne s'applique qu'aux
   nouvelles installs. Les installs existantes gardent leur valeur — l'user
   doit toggler "Enable IPv6" OFF (ou réinstaller) pour fermer la fuite.

### À valider en live (1 test propre, auto-disconnect 30s)

Un `curl -6 → 200` observé pendant une session de tests chaotique
(reconnexions rapides) suggère une fuite IPv6 transitoire ; le code dit
l'IPv6 bloqué en steady-state. Un connect propre unique confirmera.

---

## Root cause data-plane macOS PROUVÉE (2026-05-31, session root)

### Symptôme
Tunnel "Connected", routes installées, DNS OK (fix #1), mais **0 octet
d'internet** : `ping`/`dig`/`curl` 100% perte. Le pump client compte
`uplink` qui grimpe (ex. 536) mais `downlink=0` en permanence.

### Diagnostic empirique (captures double-bout)
- Mac en0 : ~0 paquet QUIC sortant vers l'exit APRES le handshake.
- Exit eth0 : ne reçoit que ~7 paquets (le handshake), puis plus rien.
- Exit warren0 (TUN) : **0** paquet décrypté pour notre IP tunnel.
- Un AUTRE client (10.66.0.5) passe du trafic normalement au même moment
  par le même exit → exit sain, data-path exit OK.

### Cause
Table de routage macOS en session connectée :
```
204.168.207.130     utun4          UHS     ← route hôte via le TUNNEL (gagne)
204.168.207.130/32  192.168.1.254  UGdSc   en0   ← route hôte via en0 (perd)
route get 204.168.207.130 → utun4
```
La route hôte vers l'IP exit pointe sur **utun4** au lieu de **en0**.
Conséquence : après le handshake (qui part AVANT l'install des routes,
donc via en0 → atteint l'exit), tous les datagrammes QUIC vers l'exit
sont **routés DANS le tunnel (utun4)** → boucle : le pump relit ses
propres paquets (uplink grimpe), rien n'atteint le vrai exit, downlink=0.

### Preuve du fix
En session connectée, forcer la route exit via le gateway physique :
```
route delete -host 204.168.207.130
route add    -host 204.168.207.130 192.168.1.254   # gateway form
```
→ `route get` = en0, **ping 1.1.1.1 = 0% perte, curl → 204.168.207.130
(IP exit) = internet réel par le tunnel.** Diagnostic confirmé à 100%.

### Localisation + fix
`crates/warren-client/src/default_route_split_macos.rs`,
`build_install_commands()` installe l'exception via
`route add -host <exit_ip> -interface <default_iface>` (forme
**interface**). Pour une IP exit **off-link** (joignable seulement via
le gateway par défaut), cette forme se fait masquer par la route
clonée utun4. Fix : forme **gateway** `route add -host <exit_ip>
<default_gateway>` (nécessite de requêter aussi le gateway via
`route -n get default`), et/ou ré-asserter l'exception APRES les /1.

### Indépendant des fixes précédents
Reproduit sur le **1.0.3 shipping (v3)** ET le dev (v4) → ce n'est NI le
fix DNS, NI le changement protocole v4 (device_id) de l'agent. C'est un
bug de routing macOS dans le code COMMITTÉ/release. Probable cause
dominante du "connecté mais pas d'internet" original sur macOS (le DNS
n'était qu'une partie).

### Note environnement
Pendant les tests, `relay set location` a été mis à `any` (le réglage
stocké `se` ne matche pas le pool d'exits qui ne contient que `DE` →
empêchait toute connexion : bug de config secondaire à traiter).

### FIX IMPLÉMENTÉ (TDD, 2026-05-31)
`build_install_commands` accepte désormais `default_gateway: Option<&str>`
et émet l'exception exit en **forme gateway** `route add -host <exit>
<gw>` (fallback forme interface si pas de gateway). `install()` découvre
le gateway physique (`route get default` → `gateway:`, fallback scutil
`Router :`, tunnel-resistant) et **supprime toute route exit périmée**
avant l'ajout. Self-contained dans `default_route_split_macos.rs` :
signature publique `DefaultRouteSplitGuard::install(exit_ip, tun_name)`
inchangée → zéro ripple warren-app, zéro conflit agent. 6 tests TDD
(RED→GREEN) + clippy/fmt clean + 98/98 lib tests warren-client.
Validation live e2e bloquée tant que l'exit reste v3 (un rebuild client
= v4), mais le mécanisme est prouvé empiriquement (route gateway-form
manuelle → ping/curl OK).
