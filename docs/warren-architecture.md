# Warren client: how the fork works

This document describes what Warren adds to the Mullvad VPN app it is forked from: the tunnel, the
identity, and where each lives in the daemon. The security guarantees (firewall, kill switch,
lockdown mode) are in [`security.md`](security.md); the build is in
[`BuildInstructions.md`](../BuildInstructions.md).

## Overview

The Warren tunnel runs over **QUIC** and is the only tunnel the app has: the WireGuard data plane
and its obfuscation modes are gone. Account, device and subscription operations go to the Warren
API of the build's channel (`api.beta.warrenbrowse.com` for beta builds, `api.warrenbrowse.com`
for production builds), each request signed with the wallet key below. Development builds may point
at another API with `WARREN_API_URL`; release builds ignore it.

## Identity

The user's identity is a non-custodial wallet: a BIP39 recovery phrase (12 or 24 words) from which
one Ed25519 key is derived. The phrase is kept in the operating system's secret store (macOS
Keychain, Windows DPAPI, a `0600` file under `<settings_dir>/secrets/` on Linux) and never leaves
the device. There is no account number and no bearer token.

That one key:

1. authenticates the QUIC TLS handshake to the exit (raw public keys), so the exit learns the
   client's key from the handshake itself;
2. signs every request to the Warren API, in the `X-Warren-PubKey`, `X-Warren-Signature`,
   `X-Warren-Timestamp` and `X-Warren-Nonce` headers. The signed message format is the
   `warren-contract` crate's, pinned by the golden vectors in `vectors/`.

The GUI flows:

- **Create a new account** generates a fresh phrase, shows it, and asks for three of its words
  back before the account is used.
- **Restore** imports an existing phrase.
- **Log out** from the GUI erases the phrase from this device after a confirmation. A device
  revoked from another device keeps its phrase unless the revocation carries the account's own
  signature.

## The exit list

The exits are listed in a `SignedRelayList`, fetched from `GET {api}/v2/exits` and verified
against the server key baked into the build before it is used or written to
`<cache_dir>/warren-relays.json`. A list that fails verification never replaces the last good one.
A copy baked in at build time (`<resource_dir>/warren-relays.json`) covers the first start.
Hand-written JSON is not accepted: without a valid signature the file is ignored.

## Where the code lives

The daemon (`mullvad-daemon`, built as `warren-daemon`) keeps the upstream state machine and adds
`warren_*` modules. The main ones:

| module | role |
|---|---|
| [`warren_signer`](../mullvad-daemon/src/warren_signer.rs) | loads the recovery phrase and derives the signing key |
| [`warren_identity_manager`](../mullvad-daemon/src/warren_identity_manager.rs) | the one owner of the live identity: the signing key and the seed the SDK client uses, swapped together |
| [`warren_remote_config`](../mullvad-daemon/src/warren_remote_config.rs) | resolves the API URL (development overrides, then the channel default) |
| [`warren_relay_list_updater`](../mullvad-daemon/src/warren_relay_list_updater.rs) | fetches and verifies the signed exit list |
| [`warren_relay_selector`](../mullvad-daemon/src/warren_relay_selector.rs) | picks an exit from the verified list |
| [`warren_tunnel_params`](../mullvad-daemon/src/warren_tunnel_params.rs) | assembles the tunnel parameters from the selected exit and the key |
| [`warren_session_gate`](../mullvad-daemon/src/warren_session_gate.rs) | refuses a connect while no account is logged in, the device is revoked or the account's access is revoked, and orders a logout after the tunnel is down |
| [`device/account_backend`](../mullvad-daemon/src/device/account_backend.rs) | account and device calls to the Warren API |

The tunnel itself is [`talpid-warren-tunnel`](../talpid-warren-tunnel/), which plugs the engine of
the [`warrenguard`](https://github.com/WarrenBrowse/warrenguard) repository into the talpid state
machine.

## Connecting

```
warren connect
  -> gRPC SetTargetState(Secured)
  -> state machine: ConnectingState
  -> tunnel parameters from the selected exit and the signing key
  -> talpid-warren-tunnel starts the warrenguard engine
  -> QUIC handshake with the exit (TLS, Ed25519 raw public keys)
  -> tunnel up, routes installed, firewall switched to the connected policy
```

When the exit's descriptor publishes an ML-KEM key, the tunnel is sealed with the hybrid X-Wing
scheme, and the GUI shows the "Quantum resistance" indicator while it is.

## Testing locally

```bash
cargo test -p mullvad-daemon --lib warren_
```
