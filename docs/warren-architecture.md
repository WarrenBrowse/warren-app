# Warren fork — Guide d'usage

Ce document explique comment fonctionnent les modes Warren ajoutés
au fork de Mullvad VPN. Pour le contexte produit (pourquoi Warren existe,
quelles décisions architecturales ont été prises), voir le repo POC
[`warren-core`](../../warren-core/) et son cahier des charges
[`docs/00-contexte-et-decisions.md`](../../warren-core/docs/00-contexte-et-decisions.md).

## Vue d'ensemble

Le **tunnel Warren (QUIC)** est l'**unique** backend tunnel du fork : il
remplace WireGuard et n'est plus optionnel (il n'y a plus de toggle pour
l'activer/désactiver). À côté, le fork ajoute un mode orthogonal optionnel :

| Mode | Effet | Toggle |
|---|---|---|
| **Warren local account** | Les opérations account/device locales (sans `api.mullvad.net`) | `Settings::warren_local_account` ou env `WARREN_LOCAL_ACCOUNT=1` |

Avec le mode local account actif, le daemon fonctionne **end-to-end sans
backend Mullvad** : identité dérivée d'une mnémonique BIP39 locale, tunnel
QUIC vers un exit Warren custom, aucun appel HTTP vers `api.mullvad.net`.
Sinon, le tunnel reste le tunnel Warren QUIC mais les opérations
account/device passent par la warren-api distante.

## Mode local account

### Option A — Toggle persistant via CLI

```bash
# Activer le mode account local (= no api.mullvad.net)
mullvad warren local-account set on

# Vérifier l'état persisté
mullvad warren local-account get

# Restart requis pour appliquer (le flag est lu au boot)
sudo systemctl restart mullvad-daemon  # Linux
# ou redémarrer l'app GUI
```

### Option B — Env var POC (dev / debug rapide)

Sans persister dans Settings :

```bash
WARREN_LOCAL_ACCOUNT=1 sudo mullvad-daemon
```

L'env var prend **toujours priorité** sur Settings — pratique pour
tester sans toucher la config persistée.

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

En mode local account, un `device.json` est **bootstrappé automatiquement
au boot** à partir de la mnémonique (cf.
[`mullvad-daemon/src/warren_device_bootstrap.rs`](../mullvad-daemon/src/warren_device_bootstrap.rs)).

## Architecture du fork

### Modules ajoutés (mullvad-daemon)

- [`warren_account_mode`](../mullvad-daemon/src/warren_account_mode.rs) —
  résolution env-or-Settings du flag `warren_local_account`.
- [`warren_signer`](../mullvad-daemon/src/warren_signer.rs) — charge la
  mnémonique BIP39 et la dérive en `SigningKey` Ed25519 + `WarrenAuthSigner`.
- [`warren_device_bootstrap`](../mullvad-daemon/src/warren_device_bootstrap.rs) —
  bootstrap atomique du `device.json` cohérent avec la mnémonique.
- [`warren_relay_selector`](../mullvad-daemon/src/warren_relay_selector.rs) —
  wrapper daemon-side autour de la crate `warren-relay-selector` (pocs).
- [`warren_tunnel_params`](../mullvad-daemon/src/warren_tunnel_params.rs) —
  assemble `WarrenTunnelParameters` à partir du selector + signing key.
- [`device::account_backend`](../mullvad-daemon/src/device/account_backend.rs) —
  trait `WarrenAccountBackend` + `Remote*`/`Local*` impls.
- [`device::device_backend`](../mullvad-daemon/src/device/device_backend.rs) —
  trait `WarrenDeviceBackend` + `Remote*`/`Local*` impls.

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
- ✅ Phase B : `WARREN_LOCAL_ACCOUNT=1` mode + bootstrap device.json
- ✅ Phase C : `WarrenAccountBackend` + `WarrenDeviceBackend` traits
- ✅ Phase D.1 : 10 endpoints REST signés via `WarrenAuthSigner`
- ✅ Phase D.2 : Allowlist côté exit warren-core
- ✅ Phase E : `Settings::warren_local_account` persistant
- ✅ Phase F : gRPC `SetWarrenLocalAccount` + `mullvad warren` CLI
- ✅ Le tunnel Warren QUIC est devenu l'unique mode tunnel (toggle
  `warren_mode` supprimé).

### Reste à livrer

- **warren-api server** (côté pocs) — backend qui remplace `api.mullvad.net`
  pour la production multi-tenant.
- **Distribution `warren-relays.json` signée** — endpoint signé OU
  baked-in dans le binaire.
- **Bench Hetzner end-to-end** du daemon fork avec `WARREN_LOCAL_ACCOUNT=1`.
