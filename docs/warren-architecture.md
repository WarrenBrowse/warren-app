# Warren fork — Guide d'usage

Ce document explique comment fonctionnent les modes Warren ajoutés
au fork de Mullvad VPN. Pour le contexte produit (pourquoi Warren existe,
quelles décisions architecturales ont été prises), voir le repo POC
[`warren-core`](../../warren-core/) et son cahier des charges
[`docs/00-contexte-et-decisions.md`](../../warren-core/docs/00-contexte-et-decisions.md).

## Vue d'ensemble

Le **tunnel Warren (QUIC)** est l'**unique** backend tunnel du fork : il
remplace WireGuard et n'est plus optionnel (il n'y a plus de toggle pour
l'activer/désactiver). Les opérations compte/device/abonnement passent
par la **warren-api distante** (`https://api.warrenbrowse.com` par défaut,
override via `WARREN_API_URL`), signées Ed25519 avec l'identité dérivée de
la mnémonique BIP39 locale.

## Pré-requis

Le mode Warren tunnel nécessite :

1. **Une mnémonique BIP39** dans `<settings_dir>/warren_mnemonic.txt`.
   Générée automatiquement au premier boot si absente. Pour réutiliser
   une mnémonique existante, écrire le fichier avant de lancer le daemon.

2. **Une `warren-relays.json`** dans `<cache_dir>/warren-relays.json` qui
   liste les exits Warren accessibles. Format v1 :
   ```json
   {
     "version": 1,
     "relays": [
       {
         "endpoint_id": "<hex-64ch pubkey ed25519>",
         "ip_addrs": ["198.51.100.7:51820"],
         "country": "se",
         "city": "Stockholm",
         "weight": 100,
         "active": true
       }
     ]
   }
   ```
   Si absent, le selector retourne `NoRelayMatch` au premier connect (=
   l'utilisateur n'est pas en mode Warren utilisable).

## Architecture du fork

### Modules ajoutés (mullvad-daemon)

- [`warren_signer`](../mullvad-daemon/src/warren_signer.rs) — charge la
  mnémonique BIP39 et la dérive en `SigningKey` Ed25519 + `WarrenAuthSigner`.
- [`warren_remote_config`](../mullvad-daemon/src/warren_remote_config.rs) —
  résolution de l'URL warren-api (env > settings > défaut compilé).
- [`warren_relay_selector`](../mullvad-daemon/src/warren_relay_selector.rs) —
  wrapper daemon-side autour de la crate `warren-relay-selector` (pocs).
- [`warren_tunnel_params`](../mullvad-daemon/src/warren_tunnel_params.rs) —
  assemble `WarrenTunnelParameters` à partir du selector + signing key.
- [`device::account_backend`](../mullvad-daemon/src/device/account_backend.rs) —
  trait `WarrenAccountBackend` + `WarrenRemoteAccountBackend` (warren-api) /
  `RemoteAccountBackend` (fallback legacy).

### Path connect en mode Warren

```
mullvad connect
  → gRPC SetTargetState(Secured)
  → daemon.set_target_state
  → state machine ConnectingState
  → ParametersGenerator::generate(retry_attempt)
      → produce_warren_tunnel_params()
        (selector + signing_key + n_connections)
  → TunnelMonitor::start_warren_tunnel
  → QUIC handshake (TLS Ed25519) ↔ ExitListener côté serveur
  → tunnel établi
```

### Crypto handshake

L'identité Warren est portée par la `SigningKey` Ed25519 dérivée de la
mnémonique BIP39 via [`warren_identity::derive_node_key`](../../warren-core/crates/warren-identity/).
Cette même clé est utilisée pour :

1. **TLS QUIC handshake** : la `SigningKey` Ed25519 sert de clé privée TLS
   (raw public keys). Le serveur exit récupère la pubkey via `conn.remote_id()`
   cryptographiquement prouvée.
2. **Signature des requêtes API Warren** via `WarrenAuthSigner` posant 4
   headers `X-Warren-{PubKey,Signature,Timestamp,Nonce}` sur les endpoints
   migrés (cf. `mullvad-api/src/rest.rs::*_or_signed`).

L'exit warren-core supporte une **allowlist** optionnelle (`ExitBindOpts.allowlist`)
qui restreint les pubkeys autorisées au handshake — pour les déploiements
multi-tenants. Cf. [`crates/warren-tunnel/src/exit.rs`](../../warren-core/crates/warren-tunnel/src/exit.rs).

## Validation locale

```bash
# Lancer les tests Warren côté daemon
cargo test -p mullvad-daemon --lib warren_

# Lancer les tests Warren côté pocs (nécessite cargo-test-nofw sur macOS)
cd ../warren-core
./scripts/dev/cargo-test-nofw.sh test -p warren-tunnel
```

## État du fork (mai 2026)

- ✅ Phase 1.A : Backend tunnel Warren (`BackendParams::Warren`) — unique mode
- ✅ Phase 2.B/C/D : Identité Warren (mnémonique → SigningKey + WarrenPubKey)
- ✅ Phase 3 : GUI rebrand cosmétique
- ✅ Phase 4 : Relay selector + state machine wiring
- ✅ Phase C : `WarrenAccountBackend` trait + backend warren-api signé
- ✅ Phase D.1 : 10 endpoints REST signés via `WarrenAuthSigner`
- ✅ Phase D.2 : Allowlist côté exit warren-core
- ✅ Phase F : `mullvad warren` CLI (api-url + mnemonic)
- ✅ Le tunnel Warren QUIC est devenu l'unique mode tunnel (toggle
  `warren_mode` supprimé).
- ✅ Compte/device toujours servis par warren-api (mode compte local
  supprimé — plus de stub « 99 ans »).

### Reste à livrer

- **warren-api server** (côté pocs) — backend qui remplace `api.mullvad.net`
  pour la production multi-tenant.
- **Distribution `warren-relays.json` signée** — endpoint signé OU
  baked-in dans le binaire.
- **Bench Hetzner end-to-end** du daemon fork.
