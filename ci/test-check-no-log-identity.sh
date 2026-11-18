#!/usr/bin/env bash
# Tests for ci/check-no-log-identity.sh, which refuses Rust log calls that
# format a public key, an address, a nonce, a seed or a mnemonic.
#
#   bash ci/test-check-no-log-identity.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHECK="$SCRIPT_DIR/check-no-log-identity.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

failures=0
checks=0
check() { # check <description> <command...>
	local description="$1"
	shift
	checks=$((checks + 1))
	if "$@"; then
		printf '  ok   %s\n' "$description"
	else
		printf '  FAIL %s\n' "$description"
		failures=$((failures + 1))
	fi
}

refuses() { ! bash "$CHECK" "$1" > /dev/null 2>&1; }
accepts() { bash "$CHECK" "$1" > /dev/null 2>&1; }

n=0
source_with() { # source_with <body>: writes a file holding <body> in a function
	n=$((n + 1))
	local file="$TMP/case-$n.rs"
	printf 'fn f() {\n%s\n}\n' "$1" > "$file"
	printf '%s' "$file"
}

for call in \
	'    log::info!("connected as {pubkey}");' \
	'    log::debug!("peer {}", peer.public_key);' \
	'    log::warn!("wallet {:?} refused", account_address);' \
	'    info!("source {src_ip} dropped");' \
	'    log::trace!("nonce={}", hex::encode(nonce));' \
	'    log::error!("seed {seed:?}");' \
	'    tracing::debug!("mnemonic {}", self.mnemonic);' \
	'    log::info!(
        "bound to {}",
        socket.local_addr()
    );'; do
	check "refuses: $(printf '%s' "$call" | head -1)" refuses "$(source_with "$call")"
done

for call in \
	'    log::info!("connected, {} exits known", exits.len());' \
	'    log::debug!("pubkey rotation done in {elapsed:?}");' \
	'    log::debug!("address family {}", addr.family);' \
	'    log::debug!("{} addresses assigned", address_count);' \
	'    log::info!("seeded from disk (generation {})", seed.directory.generation);' \
	'    log::debug!("set_enable_ipv6({})", enable_ipv6);' \
	'    // no-log: a loopback address, the same on every machine
    log::debug!("resolver on {resolver_addr}");' \
	'    log::debug!("resolver on {addr}"); // no-log: loopback' \
	'    // log::info!("old {pubkey}");' \
	'    let summary = format!("{pubkey}");'; do
	check "accepts: $(printf '%s' "$call" | tail -1)" accepts "$(source_with "$call")"
done

# A waiver covers the call right below it, not a later one.
check "a waiver does not carry over to the next call" refuses "$(source_with '    // no-log: loopback
    log::debug!("resolver on {addr}");
    log::info!("peer {pubkey}");')"

printf '%d checks, %d failed\n' "$checks" "$failures"
[ "$failures" -eq 0 ]
