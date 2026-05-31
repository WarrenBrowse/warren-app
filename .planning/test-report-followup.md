# Suivi des findings TEST-REPORT.pdf (Linux, 2026-05-31)

Source : `TEST-REPORT.pdf` (rapport de test consolidé, build `WarrenVPN-1.0.3`).
Verdict global du rapport : **tunnel étanche** (aucune fuite IP/DNS/IPv6,
kill-switch OK) ; **3 régressions** sur les fonctions périphériques (F1/F2/F3).

> ⚠️ Au moment d'écrire ceci, un autre agent travaille en parallèle dans le
> working tree (rebrand Iroh→Warren, suppression du toggle `warren mode`, fix
> DNS macOS dans `resolver.rs`). Ce document délimite qui possède quoi pour
> éviter les double-fix.

---

## F1 — Split tunnel non fonctionnel → **CORRIGÉ (code) + à valider on-device**

### Cause racine (corrigée après investigation)

L'hypothèse initiale « câbler `talpid_routing::create_routing_rules` » était
**fausse** : ce mécanisme fwmark standard n'est PAS celui de Warren. Le tunnel
Warren utilise son propre modèle de policy routing dans
`talpid-warren-tunnel/src/default_route_split/linux.rs` :

- table 100 : `0.0.0.0/1` + `128.0.0.0/1` via tun ;
- `ip rule` pref 50 : `to <exit_ip>/32 lookup main` (la socket QUIC sort en physique) ;
- `ip rule` pref 51 : `lookup 100` (tout le reste → tun).

La firewall (`talpid-core/src/firewall/linux.rs`) marque déjà les paquets des
process exclus (`meta mark = TUNNEL_FWMARK = 0x6d6f6c65`), les masquerade (SNAT)
et corrige le rpf en prerouting. **Mais** aucune `ip rule` ne routait ces
paquets marqués vers la table `main` : ils retombaient sur pref 51 (`lookup
100`) → tun → puis étaient **droppés** par la règle firewall « block marked
in-tunnel traffic ». D'où le black-hole observé (100 % loss).

### Fix appliqué

Ajout dans `build_install_commands` / `build_uninstall_commands` d'une règle :

    ip rule add fwmark 0x6d6f6c65 lookup main pref 49   # avant le lookup 100

- **Scoped aux seuls paquets marqués** → zéro impact sur le trafic normal ou le
  kill-switch (pas de régression).
- Const locale `SPLIT_TUNNEL_FWMARK` (pas de nouvelle dépendance `mullvad-types`,
  donc Cargo.toml/Cargo.lock non touchés).
- Tests unitaires purs ajoutés/mis à jour (counts, indices, ordre, pref < 51).
  Logique validée hôte (rustc --test, 9/9 OK). Le module étant
  `#[cfg(target_os="linux")]`, les tests s'exécutent au build Linux.

### Validation on-device restante (cf. runbook T2)

Lancer `.planning/warren-net-diagnostic.sh` (lecture seule) connecté :
- `ip rule show` doit lister la règle `49: ... fwmark 0x6d6f6c65 lookup main` ;
- `warren-exclude curl https://api.ipify.org` → **IP réelle** ; `curl` direct → IP exit.

---

## F2 — Content-blocking DNS casse toute résolution → **CORRIGÉ côté exit (warren-core), à valider on-device**

### Diagnostic (comparaison upstream)

- **warren-app est byte-identique à l'upstream Mullvad** : `mullvad-daemon/src/dns.rs`
  diff vide vs `upstream-baseline-2026-05-06` (composition `100.64.0.1` = pur
  Mullvad) ; `connected_state::set_dns` sur Linux pousse les adresses directement
  comme l'upstream (resolver local = macOS-only chez Mullvad aussi). **Rien à
  corriger dans warren-app.**
- **macOS** : déjà traité par l'agent parallèle dans `talpid-core/src/resolver.rs`
  (`LOCAL_DNS_RESOLVER` off, résolution déléguée à l'exit `10.66.0.1`). **NE PAS retoucher.**
- **Vraie divergence** : chez Mullvad c'est le **relais** qui répond sur
  `100.64.0.0/10` in-tunnel. L'exit Warren ne servait que `10.66.0.1:53` → activer
  un filtre pointait le resolver système sur une adresse sans réponse → casse totale.

### Fix appliqué (repo `warren-core`, stratégie « joignabilité d'abord »)

`crates/warren-exit/src/dns_redirect.rs` (nouveau) + câblage dans
`spawn_dns_forwarder` (main.rs) + déclaration module (lib.rs) :

- Règle **nftables DNAT** isolée dans `inet warren_dns` :
  `ip daddr 100.64.0.0/10 udp/tcp dport 53 dnat ip to 10.66.0.1:53`.
- conntrack réécrit la source de la réponse vers `100.64.0.x` → le stub resolver
  du client accepte la réponse (pas de mismatch de source).
- Le forwarder existant relaie vers Quad9 → la résolution remarche quand un filtre
  est activé (malware bloqué via Quad9). **Best-effort** (un échec nft ne fait pas
  tomber l'exit). Linux only, déployable en livrant le nouveau binaire exit.
- Builder de script **pur + tests** (6/6 OK hôte) ; exécution `nft -f -` Linux-only.

**Caveat** : le filtrage **par catégorie** (ads vs trackers vs …) n'est pas
implémenté — toutes les adresses `100.64.0.x` résolvent identiquement pour
l'instant. Vraie parité = blocklists curées côté exit (travail futur).

### Validation on-device restante

Connecté à l'exit patché, avec un filtre activé côté client :
- `dig example.com` doit résoudre ; `nft list table inet warren_dns` doit montrer la règle DNAT.
- `warren-net-diagnostic.sh` (warren-app) signale toujours si un resolver `100.64.0.x` est configuré.

---

## F3 — `warren mode off` → relais inutilisable + risque de lock-out → **en cours ailleurs + garde-fou produit à décider**

- Le toggle `warren mode` est **en cours de suppression** par l'agent parallèle
  (`warren_mode.rs` supprimé, champ proto `reserved 15`, composants UI
  supprimés). → supprime le déclencheur direct. **NE PAS retoucher ces fichiers.**
- La protection « fail-closed vs lock-out » de l'observation 3 est **déjà en
  place** : lockdown OFF → reset firewall (trafic autorisé) dans
  `talpid-core/src/tunnel_state_machine/error_state.rs`.

### Risque résiduel (tout échec de relais, pas seulement le toggle)

Avec lockdown ON + auto-connect ON, un `NoMatchingRelay` (relais POC unique)
arme le blocage qui survit au reboot (early-boot-blocking + config persistée).
Déblocage local : `warren lockdown-mode set off` (sans Internet).

À décider (changement de comportement kill-switch → non codé ici, non testable
hors device) : avertir avant d'armer un fail-closed indéfini sans relais
sélectionnable, **ou** ne pas réarmer après N échecs consécutifs de sélection.
Documenter la procédure de déblocage dans la doc utilisateur.

---

## Observations déjà actées (rien à faire)

- **AppArmor** : profil non requis (système 3.0.8 < 4.0) — décision : ne pas l'installer.
- **Artefact script 1.2.1.1** : endpoints à défi Cloudflare (`ifconfig.co`)
  remplacés par `ipify`/`icanhazip` (texte brut).

---

## Runbook — tests on-device restants (sudo / reboot / serveur distant)

T1 kill-switch sur crash · **T2 inspection `nft`/`ip rule` (→ `warren-net-diagnostic.sh`)** ·
T3 fuite en transition · T4 fuite DNS capture · T5 early-boot · T6 diag Iroh on/off
(bloqué tant que le fail-closed n'est pas désarmé) · T7 débit iperf3 · T8 LAN
allow/block · T9 anti-censure · T10 cycle de vie désinstall (destructif).

## TODO non couvert

- Latence & débit Iroh on vs off (bloqué tant qu'un seul relais POC).
- Endpoint de vérif dédié (`am.i.warrenbrowse.com` ?) au lieu de tiers.
- Fuite WebRTC (niveau navigateur, hors script CLI).
