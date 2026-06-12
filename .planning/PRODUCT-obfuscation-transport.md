# Product need: Warren obfuscation / anti-censorship transport

Status: backlog (not started). Captured 2026-06-12 during the full-repo audit.

## Problem

Warren has no obfuscation or pluggable circumvention transport. The tunnel is
QUIC on :443 with a raw-public-key TLS handshake (Ed25519), which:

- is a fingerprint distinct from standard web TLS, and
- rides QUIC/443, which is fingerprinted or blocked outright in hard-censorship
  regions (CN blocks QUIC; IR/RU fingerprint it).

The moment Warren needs to work behind a serious censor, raw QUIC/443 will be
blocked and there is no fallback.

## Direction

Add a pluggable obfuscation transport in **warren-core** (the data plane), not
app-side. Candidate designs, roughly in order of fit:

1. **MASQUE (UDP-over-HTTP/3)** so the tunnel looks like real HTTPS/H3 web
   traffic to a legitimate-looking server. Reference implementation already in
   the tree (do not delete): `mullvad-masque-proxy/` (see its `README.warren.md`).
   Note Warren is already QUIC, so this is about presenting a standard-web-TLS /
   real-h3 outer layer, not QUIC-in-QUIC for its own sake.
2. Domain-fronting / SNI strategies on the existing :443 endpoint.
3. Shadowsocks-style pre-tunnel obfuscation as a lightweight option.

## Why not just delete the Mullvad obfuscation code now

The audit found the upstream access-method / shadowsocks / encrypted-dns /
masque stack to be dead weight in warren-app today. But it encodes exactly the
circumvention capability Warren will need. Decision: keep the reference code,
do not reproduce it verbatim app-side, and design the real transport in
warren-core when the censorship requirement is concrete. Deleting it now would
throw away a tested MASQUE framing/fragmentation reference.

## Open questions

- Which censor environments are in scope for v1 (changes the design a lot)?
- Is the outer layer terminated at the exit, or at a separate front?
- Performance budget vs the current ~920 Mbps native path.
