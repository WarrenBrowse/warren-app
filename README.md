# Warren VPN desktop app

Welcome to the Warren VPN client app source code repository.

Warren VPN est un **fork de [Mullvad VPN](https://github.com/mullvad/mullvadvpn-app)** qui remplace
le backend tunnel WireGuard par un tunnel **QUIC** (handshake TLS Ed25519, identité dérivée
d'une mnémonique BIP39 locale) et qui peut fonctionner **sans backend Mullvad** (`api.mullvad.net`)
grâce à un mode account local.

Le fork conserve l'architecture daemon / frontend de l'upstream, système de service
([`mullvad-daemon`](mullvad-daemon/), renommé en binaire `warren-daemon`), GUI Electron
([`desktop/`](desktop/)) et CLI ([`mullvad-cli`](mullvad-cli/), renommé en binaire `warren`), et
toutes les garanties de sécurité réseau (firewall lockdown, killswitch, split-tunneling) sont
préservées. Voir [`docs/warren-architecture.md`](docs/warren-architecture.md) pour le guide d'usage et
[`UPSTREAM_BASELINE.md`](UPSTREAM_BASELINE.md) pour le suivi de la baseline upstream.

## État du fork

Phase POC privée. Le fork est en cours d'iteration ; **aucune release publique n'est encore
disponible**. La cible de la phase POC :

- Backend tunnel QUIC opérationnel sur Linux et macOS desktop
- Identité Warren BIP39 + signature Ed25519 sur les endpoints REST migrés
- Compte/abonnement servis par warren-api (`https://api.warrenbrowse.com`)
- GUI Electron rebrandée

Android et iOS ne sont **pas migrés** à ce stade, les sources upstream sont conservées telles
quelles pour réduire les conflits de merge weekly.

### Plateformes supportées (fork)

| Plateforme | Statut fork Warren |
|---|---|
| Linux (x86_64) | ✅ ciblé POC |
| macOS (arm64 / x86_64) | ✅ ciblé POC |
| Windows | ⏸ hérité upstream, non testé fork |
| Android | ⏸ upstream uniquement |
| iOS | ⏸ upstream uniquement |

Pour la matrice upstream (OS, versions, archis supportées par le code Mullvad), voir
[Supported Platforms](docs/supported-platforms.md).

## Fonctionnalités

| | Linux (Warren) | macOS (Warren) | Notes |
|---|:-:|:-:|---|
| **Warren tunnel (QUIC + Ed25519)** | ✓ | ✓ | Seul mode tunnel du fork |
| **Compte/abonnement Warren (BIP39 + warren-api)** | ✓ | ✓ | Identité BIP39, abonnement servi par warren-api |
| Quantum-resistant tunnels (PQ-WG) | ✓ | ✓ | Code WireGuard upstream hérité |
| Split tunneling | ✓ | ✓ | |
| Custom DNS server | ✓ | ✓ | |
| Content blockers (Ads etc) | ✓ | ✓ | |
| Killswitch / lockdown mode | ✓ | ✓ | |
| Local network access (optionnel) | ✓ | ✓ | |

Les modes obfuscation upstream (WireGuard over TCP / Shadowsocks / QUIC / LWO) et DAITA ne sont
**pas activés** sur le path Warren : le tunnel Warren fait QUIC sur 443 nativement.

## Sécurité utilisateur, vie privée, anonymat

Le fork hérite des garanties du client Mullvad : c'est un client VPN respectant la vie privée, qui
fait son maximum pour empêcher les fuites de trafic, avec des défauts orientés sécurité. Le
[document de sécurité dédié](docs/security.md) décrit en détail ce que l'app bloque, ce qu'elle
autorise, et comment.

**Spécifique Warren** : l'identité est portée par une clé Ed25519 dérivée d'une mnémonique BIP39
(12 mots) stockée dans le coffre de secrets de l'OS (Keychain macOS / DPAPI Windows / fichier
`<settings_dir>/secrets/warren_mnemonic.txt` 0600 sous Linux). Aucun numéro de compte, aucun token bearer.
Cette même clé authentifie le handshake TLS QUIC vers l'exit *et* signe les requêtes API Warren
(headers `X-Warren-{PubKey,Signature,Timestamp,Nonce}`). Voir
[`docs/warren-architecture.md`](docs/warren-architecture.md) § « Crypto handshake ».

## Développement sécurisé

Le fork conserve les pratiques de signature et de revue de l'upstream :

### Signatures git

Tout merge commit sur la branche `main` doit être signé PGP. Les commits individuels d'une feature
branch n'ont pas besoin d'être signés, sauf s'ils modifient un fichier *locked-down* listé dans
[`verify-locked-down-signatures`](.github/workflows/verify-locked-down-signatures.yml).

### Audits externes (upstream)

L'app upstream Mullvad est auditée tous les deux ans par des experts externes. Les résultats sont
publiés bruts dans [`audits/`](./audits/README.md). Le fork Warren **n'a pas encore d'audit
dédié** ; les modifications introduites par le fork ne sont pas couvertes par les audits Mullvad
existants. Pour signaler un problème de sécurité, voir [SECURITY.md](SECURITY.md).

## Récupérer le code

Ce repo utilise des submodules. Pour cloner :

```bash
git clone git@github.com:WarrenBrowse/warren-app.git
cd warren-app
git submodule update --init
```

Sur Linux et macOS, si vous voulez aussi le path WireGuard fallback :

```bash
git submodule update --init wireguard-go-rs/libwg/wireguard-go
```

Détails dans la [crate `wireguard-go-rs`](./wireguard-go-rs/README.md).

### Submodule `dist-assets/binaries`

Le submodule à `dist-assets/binaries` contient des binaires tiers bundlés avec l'app (Wintun, etc.).
Il pointe encore sur le repo upstream Mullvad, le fork n'a pas (encore) son propre miroir de
binaries.

### Crates Warren consommées via `path`

Plusieurs crates Warren vivent dans le repo voisin [`warren-core/`](../warren-core/) et sont
référencées par chemin (cf. `[patch.crates-io]` dans [`Cargo.toml`](Cargo.toml)) :
`warren-identity`, `warren-tunnel`, `warren-natpmp-{server,client}`, `warren-killswitch`,
`warren-ratelimit`, `warren-protocol`, `warren-config`, `warren-relay-selector`. La crate
[`talpid-warren-tunnel`](talpid-warren-tunnel/) du workspace fait le pont entre la state machine
talpid et ces crates POC.

## Builder l'app

Voir les [instructions de build](BuildInstructions.md). Notes spécifiques fork dans
[`docs/warren-architecture.md`](docs/warren-architecture.md) et dans le commit `f6a850ba58` (deps natives Linux +
workaround cross-compile).

## Lancer l'app depuis les sources (dev)

Pour itérer en local sans packager, le repo fournit un launcher de dev :
[`scripts/dev/warren-dev.sh`](scripts/dev/warren-dev.sh). Il build et lance le daemon Rust
(`warren-daemon`, avec sudo) et la GUI Electron (Vite hot-reload), en gérant proprement le
cycle de vie (Ctrl+C, cleanup du socket, restauration DNS macOS si le daemon est tué avant
de l'avoir rétablie).

Pré-requis : `cargo` + `protoc` pour le daemon, `node` + `npm` pour la GUI (Linux/macOS).

### Workflow deux terminaux

```bash
# Terminal 1 : daemon en release (perfs tunnel réelles)
./scripts/dev/warren-dev.sh daemon --release

# Terminal 2 : GUI Electron (hot-reload)
./scripts/dev/warren-dev.sh app
```

### Workflow un seul terminal

`both` lance les deux avec un cycle de vie unifié (Ctrl+C arrête daemon **et** app ; les logs
daemon sont préfixés `[daemon]`) :

```bash
./scripts/dev/warren-dev.sh both --release
```

### Commandes et options

```
daemon   Build & run le daemon Rust en foreground (sudo)
app      Lance uniquement la GUI Electron (Vite hot-reload)
both     Daemon + app, cycle de vie unifié (Ctrl+C stoppe les deux)
stop     Stoppe un daemon lancé en background
status   Affiche les composants en cours d'exécution

Options daemon :
  --release        Build le daemon en mode release
  -v / -vv / -vvv  Verbosité des logs (défaut : -v / INFO)
  --no-log-file    Logs sur stdout uniquement
  -- <args>        Passe des args supplémentaires à warren-daemon
```

> **Note sur le « mode release »** : `--release` ne s'applique qu'au **daemon**. La commande
> `app` lance toujours la GUI via `npm run develop` (dev hot-reload) ; il n'y a pas de variante
> release de la GUI dans ce script. Le workflow ci-dessus est donc « daemon release + GUI dev »,
> utile pour mesurer les perfs réelles du tunnel sans recompiler le Rust en debug. Pour un vrai
> build packagé de la GUI, voir `npm run pack:<OS>` plus bas.

Détails de comportement utiles en dev :

- Le daemon dev tourne avec `WARREN_USE_PLAINTEXT_STORAGE=1` : il persiste la mnémonique en
  fichier `0600 root:root` sous `<settings_dir>/secrets/` au lieu du Keychain macOS / DPAPI
  Windows. Sur un build dev non signé, le hash du binaire change à chaque `cargo build`, ce qui
  déclencherait un prompt d'autorisation macOS à chaque lancement ; cette variable garde la
  boucle de dev sans friction. Un build release signé (Developer ID stable) doit la laisser unset.
- Socket de management : `/var/run/warren-vpn`. La GUI prévient si le daemon n'est pas encore là.
- Daemon background : log dans `/tmp/warren-daemon-dev.log`, PID dans `/tmp/warren-daemon-dev.pid`.

## Releaser l'app

La procédure de release upstream est documentée dans [Release.md](Release.md). **Pas encore de
release Warren publique**, le repo reste privé pendant la phase POC. Voir
[`UPSTREAM_BASELINE.md`](UPSTREAM_BASELINE.md) § « Décisions actées » pour la cadence merge
upstream weekly.

## Variables d'environnement utilisées par le daemon

### Spécifiques Warren

* `WARREN_API_URL` : URL du backend warren-api (compte/abonnement/device). Vide = défaut compilé
  (`https://api.warrenbrowse.com`).

* `WARREN_SETTINGS_DIR`, `WARREN_LOG_DIR`, `WARREN_CACHE_DIR`, `WARREN_RPC_SOCKET_PATH` : Surchargent les paths daemon. Si non setés, les variantes upstream `MULLVAD_*` sont consultées en
  fallback (alias compat).

### Héritées de l'upstream (toutes encore valides)

* `TALPID_FIREWALL_DEBUG` : Aide au debug du firewall (Linux: compteurs de paquets ; macOS: log
  des packets matchés sur `pflog0`, valeurs `all` / `pass` / `drop`).

* `TALPID_FIREWALL_DONT_SET_SRC_VALID_MARK`, Linux : empêche le daemon de setter
  `net.ipv4.conf.all.src_valid_mark=1` lorsqu'un tunnel s'établit. À utiliser uniquement si vous
  comprenez les conséquences sur `rp_filter` strict.

* `TALPID_FIREWALL_DONT_SET_ARP_IGNORE`, Linux : empêche le daemon de setter
  `net.ipv4.conf.all.arp_ignore=2`. Le défaut protège l'IP in-tunnel des sondes ARP.

* `TALPID_DNS_MODULE` : Force la méthode de config DNS. Linux : `static-file` / `resolvconf` /
  `systemd` / `network-manager`. Windows : `iphlpapi` / `netsh` / `tcpip`.

* `TALPID_DISABLE_LOCAL_DNS_RESOLVER` : macOS only. À `1` pour désactiver le resolver DNS local.

* `TALPID_NEVER_FILTER_AAAA_QUERIES` : macOS only. À `1` pour ne jamais ignorer les requêtes DNS AAAA.

* `TALPID_FORCE_USERSPACE_WIREGUARD` : Force le daemon à utiliser l'implémentation userspace de
  WireGuard (path fallback).

* `TALPID_DISABLE_OFFLINE_MONITOR` : Force le daemon à toujours considérer l'hôte comme online.

* `TALPID_CGROUP2_FS`, Linux : surcharge le path cgroup2 (défaut `/sys/fs/cgroup`) utilisé pour
  split tunneling.

* `TALPID_NET_CLS_MOUNT_DIR`, Linux : force le mount point du controller `net_cls` (cgroup v1
  legacy split tunneling).

* `WARREN_MANAGEMENT_SOCKET_GROUP` (alias hérité : `MULLVAD_MANAGEMENT_SOCKET_GROUP`), Linux/macOS :
  restreint l'accès au socket UDS de management à un groupe Unix donné (= seul root et ce groupe
  peuvent piloter CLI/GUI et lire la phrase mnémonique du wallet). Si la variable est définie mais
  que le groupe n'existe pas, le daemon refuse de démarrer le socket (fail-closed). Si elle n'est
  pas définie, le daemon utilise le groupe `warren` (créé par l'installeur). En l'absence de ce
  groupe, le socket retombe en accès global (`0o766`) avec un avertissement : dans ce mode, les RPC
  wallet/secrets sont restreints au premier uid local qui s'y connecte (trust-on-first-use). Pour la
  sûreté multi-utilisateurs, créez le groupe `warren` et ajoutez-y votre utilisateur de bureau.

* `MULLVAD_BACKTRACE_ON_FAULT` : Sur SIGSEGV etc., log un backtrace dans `daemon.log`. Activé par
  défaut en debug-build, désactivé en release-build. Allocation depuis le signal handler =
  techniquement UB ; à activer à vos risques.

### Builds de développement uniquement

* `MULLVAD_API_HOST` : Hostname à utiliser pour les requêtes API upstream (path account remote).

* `MULLVAD_API_ADDR` : IP:port à utiliser pour les requêtes API upstream.

* `MULLVAD_API_DISABLE_TLS` : Force du HTTP en clair pour les requêtes API.

* `MULLVAD_CONNCHECK_HOST` : Hostname utilisé pour les requêtes de connection check.

* `MULLVAD_ENABLE_DEV_UPDATES` : Active les version checks dans les builds dev.

### Setter les variables d'environnement

#### Linux

Edit du systemd unit via `systemctl edit warren-daemon.service` :

```ini
[Service]
Environment="WARREN_API_URL=https://api.warrenbrowse.com"
```

Restart du daemon :

```bash
sudo systemctl restart warren-daemon
```

#### macOS

Utiliser `plutil` (path plist à confirmer selon l'installer fork) :

```bash
sudo plutil -replace EnvironmentVariables -json \
  '{"WARREN_API_URL": "https://api.warrenbrowse.com"}' \
  /Library/LaunchDaemons/net.mullvad.daemon.plist
launchctl unload -w /Library/LaunchDaemons/net.mullvad.daemon.plist
launchctl load   -w /Library/LaunchDaemons/net.mullvad.daemon.plist
```

#### Windows

Hérité upstream, `setx` depuis un shell élevé, puis `sc.exe stop / start`. Non couvert par le
fork POC.

## Variables d'environnement utilisées par le frontend desktop

* `MULLVAD_PATH` : Path du dossier contenant les outils annexes (`warren-problem-report`) en dev.
  Défaut : `<repo>/target/debug/`.
* `MULLVAD_DISABLE_UPDATE_NOTIFICATION` : À `1` pour désactiver la notification de mise à jour.

## Commandes de développement Electron

- `npm run develop` : develop l'app avec live-reload
- `npm run lint` : lint le code
- `npm run pack:<OS>` : package l'app pour distribution (`linux`, `mac`, `win`)
- `npm test` : run les tests

## Icône de tray sur Linux

Les pré-requis varient selon le desktop environment. Si le tray n'apparaît pas :

### GNOME

Installer l'extension shell `AppIndicator and KStatusNotifierItem Support` :
https://extensions.gnome.org/extension/615/appindicator-support/

### Autres DE

Installer un de :
- `libappindicator3-1`
- `libappindicator1`
- `libappindicator`

## Structure du repo

### App Electron + assets electron-builder

- **desktop/packages/mullvad-vpn/** (le nom de package est conservé pour éviter les conflits de
  merge avec upstream ; le `productName` Electron est `Warren VPN`)
  - **assets/** : assets graphiques + stylesheets
  - **src/**
    - **main/index.ts** : entry du process main
    - **renderer/app.tsx** : entry du process renderer
    - **renderer/routes.tsx** : configuration des routes
    - **renderer/transitions.ts** : règles de transition entre views
  - **tasks/** : tâches Gulp pour build + watch dev
    - **distribution.js** : config `electron-builder`
  - **test/** : tests GUI Electron
- **dist-assets/** : icônes, binaires et fichiers utilisés pour produire les distribuables
  - **binaries/** : submodule (encore upstream Mullvad)
  - **linux/** : scripts + config pour deb et rpm
  - **pkg-scripts/** : scripts bundle pkg macOS
  - **windows/** : config NSIS installer + assets

### Build, tests, misc

- **build-windows-modules.sh** : compile les libs C++ Windows
- **build.sh** : sanity check du working dir + build des installers

### Daemon Warren

Le daemon est en Rust, multi-crates. La crate top-level qui produit le binaire `warren-daemon` est
[`mullvad-daemon`](mullvad-daemon/) (nom de package upstream conservé, binaire renommé via
`[[bin]] name = "warren-daemon"`).

Comme upstream, le code se sépare en deux familles :

- Crates `talpid-*` : librairie VPN générique, *agnostique* du backend account. Le fork ajoute
  [`talpid-warren-tunnel`](talpid-warren-tunnel/) qui plugge le tunnel QUIC Warren dans la state
  machine talpid.
- Crates `mullvad-*` : code spécifique à l'app (settings, management interface, GUI integration).
  Le fork ajoute les modules `warren_*` dans `mullvad-daemon/src/` (cf. liste dans
  [`docs/warren-architecture.md`](docs/warren-architecture.md)).

Fichiers à connaître :

- **Cargo.toml** : workspace root. Liste les 52 crates membres + `[patch.crates-io]` pour
  pointer les crates `warren-*` vers `../warren-core/`.
- **mullvad-daemon/** : crate qui builde le binaire `warren-daemon`.
- **mullvad-cli/** : crate qui builde le binaire `warren` (frontend CLI).
- **talpid-core/** : coeur de l'implémentation VPN, agnostique Mullvad/Warren.
- **talpid-warren-tunnel/** : adaptateur du tunnel QUIC Warren pour la state machine talpid (fork-only).

## Vocabulaire

- **App** : l'ensemble de ce repo = « Warren VPN App ».
  - **Daemon** : process headless `warren-daemon` (Rust), expose un management interface.
  - **Frontend** : tout programme qui se connecte au management interface pour piloter le daemon.
    - **GUI** : app Electron + React (binaire bundlé `Warren VPN`).
    - **CLI** : binaire Rust `warren` (frontend terminal).
- **Warren tunnel** : le tunnel QUIC Warren (handshake TLS Ed25519). C'est l'unique backend tunnel
  du fork : il n'y a plus de toggle pour l'activer/désactiver.
- **Compte Warren** : les opérations account/device/abonnement passent par warren-api
  (backend distant signé Ed25519). L'identité vient de la mnémonique BIP39 locale. « Créer un
  compte » génère une mnémonique fraîche (sauvegarde obligatoire de la phrase à l'écran avant de
  continuer) ; « Restaurer » importe une phrase existante ; la « Déconnexion » efface la mnémonique
  de cet appareil (vraie déconnexion). Il n'y a pas de connexion par clé publique : on s'identifie
  avec la phrase de restauration.
- **Mnémonique** : BIP39 12 mots stockée dans le coffre de secrets de l'OS (Keychain / DPAPI /
  fichier `secrets/warren_mnemonic.txt` 0600 sous Linux), source de la `SigningKey` Ed25519 qui sert
  d'identité Warren.
- **EndpointId / WarrenPubKey** : pubkey Ed25519 (32 bytes) qui identifie un exit Warren dans le
  `warren-relays.json`.

## Paths de fichiers utilisés par l'app Warren

### Daemon

Tous les paths sont définis dans la crate [`mullvad-paths`](mullvad-paths/) et incluent les alias
`WARREN_*` (prioritaires) + `MULLVAD_*` (fallback compat).

Sous Windows, lorsqu'un process tourne en service, `%LOCALAPPDATA%` se résout en
`C:\Windows\system32\config\systemprofile\AppData\Local`.

#### Settings (env override : `WARREN_SETTINGS_DIR`)

| Plateforme | Path |
|---|---|
| Linux | `/etc/warren-vpn/` |
| macOS | `/etc/warren-vpn/` |
| Windows | `%LOCALAPPDATA%\Warren VPN\` |

#### Logs (env override : `WARREN_LOG_DIR`)

| Plateforme | Path |
|---|---|
| Linux | `/var/log/warren-vpn/` + systemd |
| macOS | `/var/log/warren-vpn/` |
| Windows | `C:\ProgramData\Warren VPN\` |

#### Cache (env override : `WARREN_CACHE_DIR`)

| Plateforme | Path |
|---|---|
| Linux | `/var/cache/warren-vpn/` |
| macOS | `/Library/Caches/warren-vpn/` |
| Windows | `C:\ProgramData\Warren VPN\cache` |

#### Socket RPC (env override : `WARREN_RPC_SOCKET_PATH`)

| Plateforme | Path |
|---|---|
| Linux | `/var/run/warren-vpn` |
| macOS | `/var/run/warren-vpn` |
| Windows | `//./pipe/Warren VPN` |

Le rename de `PRODUCT_NAME` (de `mullvad-vpn` à `warren-vpn`) est volontaire : il évite les
collisions filesystem/sockets avec un client Mullvad upstream installé en parallèle sur la même
machine. Cf. `mullvad-paths/tests/warren_collision_safety.rs`.

#### Fichiers Warren-only sous `<settings_dir>/`

| Fichier | Rôle |
|---|---|
| `secrets/warren_mnemonic.txt` | Mnémonique BIP39 12 mots : uniquement le repli Linux/plaintext (perms 0600, owner root) ; sur macOS/Windows elle est dans le Keychain/DPAPI. Un fichier hérité `<settings_dir>/warren_mnemonic.txt` est migré puis supprimé au boot. |

#### Fichiers Warren-only sous `<cache_dir>/`

| Fichier | Rôle |
|---|---|
| `warren-relays.json` | Liste des exits Warren signée Ed25519 (format v2). Format détaillé dans [`docs/warren-architecture.md`](docs/warren-architecture.md) |

### App Electron desktop

| Plateforme | Path |
|---|---|
| Linux | `$XDG_CONFIG_HOME/Warren VPN/gui_settings.json` |
| macOS | `~/Library/Application Support/Warren VPN/gui_settings.json` |
| Windows | `%LOCALAPPDATA%\Warren VPN\gui_settings.json` |

## Icônes

Voir le [README graphics](graphics/README.md). Les icônes Warren ne sont pas encore intégrées,les assets upstream sont temporairement réutilisés.

## Locales et traductions

Procédure générale : [README locales](./desktop/packages/mullvad-vpn/locales/README.md).
Les strings user-facing « Mullvad VPN » ont été remplacées par « Warren VPN » dans le commit
`22d84f69a7` ; les fichiers `.po` des locales n'ont pas encore été re-traduits, les traductions
existantes peuvent contenir « Mullvad ».

# Licence

Ce repo est un fork sous **GPL-3.0** de [`mullvadvpn-app`](https://github.com/mullvad/mullvadvpn-app).

Copyright original : (C) 2026  Mullvad VPN AB
Modifications fork : (C) 2026  Warren contributors

This program is free software: you can redistribute it and/or modify it under the terms of the
GNU General Public License as published by the Free Software Foundation, either version 3 of
the License, or (at your option) any later version.

Pour l'accord de licence complet, voir [LICENSE.md](LICENSE.md).

**Trademarks** : les noms « Mullvad » et « Mullvad VPN » et le logo associé sont des marques de
Mullvad VPN AB **non couvertes par la GPL**. Le fork Warren n'utilise pas ces marques dans ses
binaires distribués (rebrand `productName` + paths + bin names), voir
[`UPSTREAM_BASELINE.md`](UPSTREAM_BASELINE.md) § « Risques connus ».
