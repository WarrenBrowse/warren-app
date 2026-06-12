# mullvad-masque-proxy (kept as a Warren reference, not built)

This crate is **not** a workspace member and nothing in warren-app depends on
it. It is retained on purpose, as a reference implementation, and must not be
deleted in dead-code passes.

## What it is

A limited UDP-over-HTTP/3 proxy implementing MASQUE (IETF `CONNECT-UDP` over
QUIC). In upstream Mullvad it is the censorship-circumvention transport: the
VPN's UDP (WireGuard) is wrapped in HTTP/3 toward what looks like an ordinary
web server (`/.well-known/masque/udp/`, h3 ALPN), so deep-packet inspection
sees plain HTTPS web traffic.

## Why it is product-relevant for Warren

Warren's tunnel is already QUIC on :443, so wrapping it verbatim would be
QUIC-in-QUIC and is the wrong move. But the underlying need is real and
currently unmet:

- Warren's QUIC handshake uses raw-public-key TLS (Ed25519), which is a
  fingerprint and not standard web TLS.
- In hard-censorship regions QUIC/443 is fingerprinted or blocked outright.
- Warren has **no** obfuscation / pluggable circumvention transport today.

So Warren will need an obfuscation transport that makes the tunnel
indistinguishable from real HTTPS/H3. MASQUE is one good design; the correct
home for it is **warren-core** (the data plane), as a pluggable transport, not
this WireGuard-shaped app-side crate. This crate's framing/fragmentation
(`fragment.rs`, the datagram context-id handling, MTU/payload-size math in
`lib.rs`) is a high-quality reference for that future work.

Tracking: see `.planning/PRODUCT-obfuscation-transport.md`.
