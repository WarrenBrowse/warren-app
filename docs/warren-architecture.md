# Warren fork — Guide d'usage

Ce document explique comment activer et utiliser les modes Warren ajoutés
au fork de Mullvad VPN. Pour le contexte produit (pourquoi Warren existe,
quelles décisions architecturales ont été prises), voir le repo POC
[`warren-core`](../../warren-core/) et son cahier des charges
[`docs/00-contexte-et-decisions.md`](../../warren-core/docs/00-contexte-et-decisions.md).

## Vue d'ensemble

Le fork ajoute **deux modes orthogonaux** activables indépendamment :

| Mode | Effet | Toggle |
|---|---|---|
| **Warren tunnel** | Le backend tunnel devient Iroh QUIC au lieu de WireGuard | `Settings::warren_mode` ou env `WARREN_TUNNEL=1` |
| **Warren local account** | Les opérations account/device locales (sans `api.mullvad.net`) | `Settings::warren_local_account` ou env `WARREN_LOCAL_ACCOUNT=1` |

Quand les deux sont actifs, le daemon fonctionne **end-to-end sans backend
Mullvad** : identité dérivée d'une mnémonique BIP39 locale, tunnel via
Iroh QUIC vers un exit Warren custom, aucun appel HTTP vers
`api.mullvad.net`.

Quand **aucun** des deux n'est actif (défaut), le daemon est strictement
identique à Mullvad upstream (= path WireGuard + API Mullvad).

## Activer Warren mode

### Option A — Toggle persistant via CLI (recommandé)

```bash
# Activer le tunnel Warren (Iroh)
mullvad warren mode set on

# Activer le mode account local (= no api.mullvad.net)
mullvad warren local-account set on

# Vérifier l'état persisté
mullvad warren mode get
mullvad warren local-account get

# Restart requis pour appliquer (les flags sont lus au boot)
sudo systemctl restart mullvad-daemon  # Linux
# ou redémarrer l'app GUI
```

### Option B — Env var POC (dev / debug rapide)

Sans persister dans Settings :

```bash
WARREN_TUNNEL=1 WARREN_LOCAL_ACCOUNT=1 sudo mullvad-daemon
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

- [`warren_mode`](../mullvad-daemon/src/warren_mode.rs) — résolution
  env-or-Settings du flag `warren_mode`.
- [`warren_account_mode`](../mullvad-daemon/src/warren_account_mode.rs) —
  idem pour `warren_local_account`.
- [`warren_signer`](../mullvad-daemon/src/warren_signer.rs) — charge la
  mnémonique BIP39 et la dérive en `SigningKey` Ed25519 + `WarrenAuthSigner`.
- [`warren_device_bootstrap`](../mullvad-daemon/src/warren_device_bootstrap.rs) —
  bootstrap atomique du `device.json` cohérent avec la mnémonique.
- [`warren_relay_selector`](../mullvad-daemon/src/warren_relay_selector.rs) —
  wrapper daemon-side autour de la crate `warren-relay-selector` (pocs).
- [`warren_iroh_params`](../mullvad-daemon/src/warren_iroh_params.rs) —
  assemble `WarrenIrohParameters` à partir du selector + signing key.
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
      ├─ warren_mode actif → produce_warren_iroh_params()
      │                       (selector + signing_key + n_connections)
      └─ warren_mode inactif → produce_wireguard_params()
  → TunnelMonitor::start_warren_iroh OR ::start (= WG)
  → Iroh QUIC handshake (TLS Ed25519) ↔ ExitListener côté serveur
  → tunnel établi
```

### Crypto handshake

L'identité Warren est portée par la `SigningKey` Ed25519 dérivée de la
mnémonique BIP39 via [`warren_identity::derive_node_key`](../../warren-core/crates/warren-identity/).
Cette même clé est utilisée pour :

1. **TLS QUIC handshake** Iroh : `iroh::SecretKey::from_bytes(signing.to_bytes())`.
   Le serveur exit récupère la pubkey via `conn.remote_id()` cryptographiquement
   prouvée.
2. **Signature des requêtes API Warren** via `WarrenAuthSigner` posant 4
   headers `X-Warren-{PubKey,Signature,Timestamp,Nonce}` sur les endpoints
   migrés (cf. `mullvad-api/src/rest.rs::*_or_signed`).

L'exit warren-core supporte une **allowlist** optionnelle (`ExitBindOpts.allowlist`)
qui restreint les pubkeys autorisées au handshake — pour les déploiements
multi-tenants. Cf. [`crates/warren-iroh-tunnel/src/exit.rs`](../../warren-core/crates/warren-iroh-tunnel/src/exit.rs).

## Validation locale

```bash
# Lancer les tests Warren côté daemon
cargo test -p mullvad-daemon --lib warren_

# Lancer les tests Warren côté pocs (nécessite cargo-test-nofw sur macOS)
cd ../warren-core
./scripts/dev/cargo-test-nofw.sh test -p warren-iroh-tunnel
```

## État du fork (mai 2026)

- ✅ Phase 1.A : Backend tunnel dispatch (`TunnelBackend::{Wireguard,WarrenIroh}`)
- ✅ Phase 2.B/C/D : Identité Warren (mnémonique → SigningKey + WarrenPubKey)
- ✅ Phase 3 : GUI rebrand cosmétique
- ✅ Phase 4 : Relay selector + state machine wiring
- ✅ Phase B : `WARREN_LOCAL_ACCOUNT=1` mode + bootstrap device.json
- ✅ Phase C : `WarrenAccountBackend` + `WarrenDeviceBackend` traits
- ✅ Phase D.1 : 10 endpoints REST signés via `WarrenAuthSigner`
- ✅ Phase D.2 : Allowlist côté exit warren-core
- ✅ Phase E : `Settings::warren_mode` + `warren_local_account` persistants
- ✅ Phase F : gRPC `SetWarrenMode`/`SetWarrenLocalAccount` + `mullvad warren` CLI

### Reste à livrer

- **warren-api server** (côté pocs) — backend qui remplace `api.mullvad.net`
  pour la production multi-tenant.
- **Distribution `warren-relays.json` signée** — endpoint signé OU
  baked-in dans le binaire.
- **UI Electron toggle** — checkbox mode Warren dans les Settings de l'app
  desktop (équivalent visuel à `mullvad warren mode set`).
- **Bench Hetzner end-to-end** du daemon fork avec `WARREN_TUNNEL=1
  WARREN_LOCAL_ACCOUNT=1`.
