# Warren daemon architecture

How the Warren parts of the daemon fit together. The per-module detail lives
in the module headers of `mullvad-daemon/src/lib.rs` and in each module's own
documentation, which are kept with the code; this page is the map.

Related documents: [`app-routing.md`](app-routing.md) (which session each
app's traffic leaves through), [`UPSTREAM-DETACH.md`](UPSTREAM-DETACH.md)
(what is still inherited from Mullvad and how it is tracked),
[`security.md`](security.md).

## Overview

The Warren tunnel (QUIC, the `warrenguard` engine driven by
`talpid-warren-tunnel`) is the only tunnel backend. Account, device and
subscription operations go to the Warren API of the product environment the
binary was built for (`WARREN_PRODUCT_ENV`, resolved by
`warren_product_config`), signed with the Ed25519 identity derived from the
user's BIP39 recovery phrase. A release build always talks to its compiled
API host; only a development build (`api-override` feature) honours
`WARREN_API_URL` or the `warren_api_url` setting
(`warren_remote_config::api_url`).

## Identity

- No identity exists until the user creates or imports one: the onboarding
  actions (`CreateNewAccount`, `SetWarrenMnemonic`) are the only writers of a
  recovery phrase. Loading at boot is read-only.
- Storage (`warren_signer`, `os_secret_storage`): macOS System Keychain,
  Windows DPAPI blob at `<settings_dir>/secrets/warren_mnemonic.dpapi`, Linux
  plaintext file `<settings_dir>/secrets/warren_mnemonic.txt` (mode 0600,
  root). A file left at the older `<settings_dir>/warren_mnemonic.txt` is
  migrated on boot.
- `warren_identity_manager` is the single surface that swaps the in-memory
  identity (create, restore, logout), keeping the request signer and the SDK
  seed in step without a daemon restart.
- The key is derived with `warren_identity::derive_node_key` from the
  `warren-sdk-rs` sibling repository. The same key authenticates the QUIC
  handshake to an exit and signs API requests (`X-Warren-*` headers).

## Exit list

- `warren_relay_list_updater` fetches the signed exit list from
  `GET {api}/v2/exits` (conditional GET, signature verified against the
  pinned server key before caching, atomic write, hot swap into the live
  selector).
- The cache is `<cache_dir>/warren-relays.json`; a copy baked into the build
  at CI time (`<resource_dir>/warren-relays.json`) seeds a fresh install. The
  file is a signed envelope: a hand-written JSON list is rejected.
- `warren_relay_selector` wraps `warren_discovery_core::WarrenRelaySelector`
  (from the `warren-contract` sibling) and picks the exit for a query built by
  `warren_query_from_settings` from the location the user chose.
- `warren_relay_list_view` presents the list in the inherited `RelayList`
  shape the desktop location picker reads.

## Connecting

```
SetTargetState(Secured) over gRPC
  -> Daemon::set_target_state
  -> talpid-core tunnel state machine (ConnectingState)
  -> tunnel::ParametersGenerator::generate_warren_tunnel_params
       (selector, shared signing key, session token, multi-hop circuit)
  -> warren_tunnel_params::assemble_for_attempt
  -> talpid-warren-tunnel: QUIC handshake with the exit, tun device, routes
```

Multi-hop circuits come from the signed directory fetched by
`warren_multi_hop_directory`. Anonymous session tokens are minted by
`warren_token_provider`. The live tunnel status the clients show is cached in
`warren_status` and served over the `GetWarrenStatus` RPC and its update
stream.

## Account

`device::AccountManager` drives the account operations through
`device::account_backend::WarrenRemoteAccountBackend` (signed requests to the
Warren API: subscription expiry, voucher redemption and collection, account
deletion). The wallet's port-forward standing is polled by
`warren_account_standing`.

## Background refreshers

Signed artifacts refreshed on a schedule, all sharing the retry, ETag and
atomic-write policy of `warren_artifact_refresh`: launch announcements
(`warren_announcements_updater`), operator notices (`warren_notices_updater`),
the forum digest (`warren_forum_digest_updater`) and the network descriptor
(`warren_network_info`).

## Running the tests

```bash
cargo nextest run -p mullvad-daemon
cargo nextest run -p talpid-warren-tunnel
```

The forum login vectors need the `vectors` submodule:
`git submodule update --init vectors`.
