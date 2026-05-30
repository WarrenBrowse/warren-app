# Warren macOS — état, diagnostic, et chantiers prod

> Mis à jour : 2026-05-30 (v1.0.3). Document de référence sur l'état réel
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
