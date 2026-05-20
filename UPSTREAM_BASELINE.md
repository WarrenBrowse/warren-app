# Warren — fork upstream baseline

## Métadonnées

- **Upstream** : https://github.com/mullvad/mullvadvpn-app
- **HEAD upstream cloné** : `440c97f36a6dbf77eebd25d0e1ef52a3efd4ff43`
- **Date du clone** : 2026-05-06
- **Tag local** : `upstream-baseline-2026-05-06`
- **Mode** : reconnaissance (pas de modif Warren à ce stade)

## Workspace upstream

- **52 crates Rust** (cf. `Cargo.toml [workspace.members]`)
- **103 MB** sur disque (clone shallow `--depth 1`)
- **Layouts top-level** : `mullvad-*` (21 crates), `talpid-*` (12 crates), `wireguard-go-rs/`, `desktop/` (Electron + Volta), `android/`, `ios/`

## Dépendances système (cf. `BuildInstructions.md`)

| Catégorie | Outil | Version | Notes Warren |
|---|---|---|---|
| Toolchain Rust | rustup stable | latest | Auto via `./scripts/setup-rust <platform>` |
| GUI Electron | Node + Volta | géré via `desktop/package.json` | Optionnel si `--daemon-only` |
| WireGuard backend | Go | 1.21+ | Optionnel si `--gotatun` désactivé. **Warren option B** : supprimer `wireguard-go-rs` + `gotatun` → plus de dep Go |
| gRPC codegen | protobuf-compiler | ≥ 3.15 | `brew install protobuf` macOS |
| Build scripts | bash | ≥ 4.0 | `brew install bash` sur macOS (default 3.2.5 KO) |
| Electron gRPC | podman | latest | Container pour bindings TypeScript |
| MSVC | Build Tools | — | Windows only |

## Inventaire des modifications Warren à venir (cf. `warren-core/docs/03-fork-mullvad.md`)

### À refondre (auth wallet : `Authorization: Token` → `X-Warren-*` signature)

| Fichier upstream | Modification Warren |
|---|---|
| `mullvad-api/src/access.rs` | Remplacer Bearer token par signature Ed25519 (méthode + path + ts + nonce + body_hash) |
| `mullvad-api/src/device.rs` | Idem |
| `mullvad-api/src/rest.rs` | Idem |
| `mullvad-api/src/lib.rs` | `AccountsProxy` → `WarrenIdentityProxy` |
| `mullvad-daemon/src/account_history.rs` | `AccountHistory` → `WarrenIdentityHistory` (par pubkey) |
| `mullvad-daemon/src/device/mod.rs` | `AccountService` → `WarrenIdentityService` |
| `mullvad-daemon/src/device/service.rs` | Idem |
| `mullvad-daemon/src/lib.rs` | Wire le nouveau service |
| `mullvad-daemon/src/migrations/device.rs` | Migration upstream conservée pour backwards compat |
| `mullvad-daemon/src/migrations/v5.rs` | Idem |
| `mullvad-daemon/src/migrations/account_history.rs` | Idem |
| `mullvad-types/src/device.rs` | `AccountAndDevice` → `WarrenIdentity { signing_key, public_key, subscription }` |

### À introduire (nouveau code)

| Élément | Description |
|---|---|
| `trait Tunnel` dans `talpid-tunnel/src/lib.rs` | **N'existe pas** dans l'upstream. À créer avec API `setup() / teardown() / tunnel_address()`. |
| Impl `Tunnel` pour `talpid-wireguard` | Adapter le code WG existant derrière le trait |
| Impl `Tunnel` pour `warren-iroh-tunnel` | Nouvelle crate (existe déjà dans `warren-core/crates/warren-iroh-tunnel/`), consommée via `path = "../../warren-core/crates/warren-iroh-tunnel"` |
| Crate `warren-identity` | Existe dans `warren-core/`, consommée via path |
| Crate `warren-natpmp-server` | Existe, consommée via path |
| Crate `warren-natpmp-client` | Existe, consommée via path |
| Crate `warren-killswitch` | Existe (Linux nft + macOS pf), consommée via path |
| Crate `warren-ratelimit` | Existe, consommée via path |
| Crate `warren-protocol` | Existe, consommée via path |
| Crate `warren-config` | Existe, consommée via path |

### Adapter (relay selector)

| Fichier | Modification |
|---|---|
| `mullvad-relay-selector/src/lib.rs` | Identifier les exits par `EndpointId` iroh (32 bytes Ed25519) en plus de IP/port |
| `mullvad-relay-selector/src/relay_selector/` | Algorithm de sélection adapté pour les nouveaux critères |

### GUI Electron à refondre

| Fichier | Modification |
|---|---|
| `desktop/packages/mullvad-vpn/src/renderer/components/AccountNumberLabel.tsx` | Remplacer affichage du numéro de compte par hash court de la pubkey |
| `desktop/packages/mullvad-vpn/src/renderer/components/views/account/AccountView.tsx` | Refondre en écran « Keys » : pubkey, mnemonic backup, import via mnemonic |
| `desktop/packages/mullvad-vpn/src/renderer/components/views/account/components/account-number-row/AccountNumberRow.tsx` | → `PublicKeyRow.tsx` (affichage + bouton copy) |
| `desktop/packages/mullvad-vpn/src/renderer/components/views/account/components/account-expiry-row/AccountExpiryRow.tsx` | → `SubscriptionRow.tsx` (statut abonnement Warren) |

### À désactiver (feature flags)

| Élément | Action |
|---|---|
| `wireguard-go-rs` (FFI Go) | Behind feature `tunnel_backend = "wireguard"`. POC Warren = `tunnel_backend = "iroh"` actif, WG en repli optionnel |
| `gotatun` | Idem |
| `tunnel-obfuscation` (Shadowsocks, QUIC-over-TCP, LWO) | Désactivé POC. Iroh fait QUIC sur 443 nativement |
| Feature `discovery-pkarr-dht` côté iroh | **Jamais** activée — verrouiller dans `warren-config` (cf. doc 13.2 rapport amont) |

## Estimation effort

- **Phase 1 setup** : 1 jour (clone + tag baseline + build green sans modif)
- **Phase 1.A trait Tunnel** : 2-3 j (introduire le trait + ré-impl WG derrière + valider en CI upstream)
- **Phase 1.B impl Iroh Tunnel** : 1-2 j (câbler `warren-iroh-tunnel` existant via le trait)
- **Phase 2 auth wallet** : 5-7 j (mullvad-api refonte + AccountService → WarrenIdentityService + migrations + tests)
- **Phase 3 GUI Electron** : 3-5 j (Account → Keys + designs)
- **Phase 4 relay selector** : 2-3 j (EndpointId + Iroh discovery)

**Total estimé** : 14-21 j de dev focus, avant que le binaire Warren-branded soit shippable. Multiple PRs internes pour chaque phase, sans toucher l'upstream Mullvad.

## Risques connus (cf. `docs/03-fork-mullvad.md` § Risques fork)

- Cadence upstream élevée (110-180 commits/semaine) → conflits de merge fréquents
- Test coverage upstream limité (CI privé Mullvad)
- Trademarks Mullvad **non couverts** par GPL → exclure nom + logo dans le fork

## Décisions actées (2026-05-06)

1. **Hébergement repo** : `github.com/WarrenBrowse/warren-app` (depuis 2026-05-20, M4.H.D migration Gitea → GitHub pour cohérence avec `warren-core` sur `github.com/WarrenBrowse/warren-core` ; ancien remote Gitea `git.p2p.legal/warren/warren-app` conservé comme `backup-gitea` fetch-only en lecture seule)
2. **Visibilité** : **privé** pendant la phase POC. Public au lancement freemium (GPL-3.0 oblige le source du fork — la visibilité publique sera réactivée à ce moment-là)
3. **CI** : workflows séparés du `warren-core`. À adapter : retirer les jobs upstream Mullvad inutiles (tests Android/iOS si on ne ship pas mobile dès POC), ajouter le check `cargo build` Warren-only via le feature flag `tunnel_backend = "iroh"`. Détails à figer en début phase 1
4. **Cadence merge upstream** : **weekly cherry-pick** de `main` upstream. Branche `warren-base` divergente, on rebase les commits Warren sur le HEAD upstream du jour J chaque semaine (lundi typique). Limites les conflits accumulés vs un freeze long

## État reconnaissance (2026-05-06)

- ✅ Clone shallow OK
- ✅ Tag local `upstream-baseline-2026-05-06`
- ✅ Inventaire workspace (52 crates)
- ✅ Inventaire dépendances système
- ✅ Inventaire fichiers à toucher (auth wallet + tunnel trait + relay selector + GUI)
- ✅ Décisions hébergement / CI / cadence merge (cf. § Décisions actées)
- ✅ Push initial `main` + tag `upstream-baseline-2026-05-06` + branche `warren-base`
- ⏸ Build green sanity check (deferred — deps externes lourdes : Volta, podman, protobuf, bash 4. À faire au début phase 1)

Reconnaissance terminée. Phase 1 amorcée 2026-05-07 (post-bench stab 24h validé `warren-core` shippable).

## Setup phase 1 (commandes prêtes)

Une fois le repo Gitea créé :

```bash
cd /Users/poka/dev/warrenBros/warren-app
git remote add origin git@github.com:WarrenBrowse/warren-app.git
git push -u origin main
git push origin upstream-baseline-2026-05-06   # tag baseline
```

Première branche de travail Warren :

```bash
git checkout -b warren-base
git push -u origin warren-base
```

Commandes weekly cherry-pick (à automatiser via script `scripts/sync-upstream.sh` plus tard) :

```bash
git remote add upstream https://github.com/mullvad/mullvadvpn-app.git
git fetch upstream main
git checkout warren-base
git rebase upstream/main   # ou cherry-pick sélectif si conflits massifs
git push --force-with-lease origin warren-base
```
