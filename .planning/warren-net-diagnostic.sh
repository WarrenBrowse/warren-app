#!/usr/bin/env bash
# Warren VPN — read-only network diagnostic (Linux).
#
# Purpose: confirm the F1 split-tunnel fix on-device and surface the F2
# DNS state, mapping to runbook test T2 of TEST-REPORT.pdf.
#
# SAFETY: this script is STRICTLY READ-ONLY. It only runs `... show` /
# `list` / lookup commands. It never adds, deletes, flushes or modifies a
# single route, rule, nft object or DNS setting. Safe to run while
# connected. Most commands need neither sudo nor root, except
# `nft list ruleset` (kernel restricts ruleset reads to CAP_NET_ADMIN).
#
# Usage:
#   ./warren-net-diagnostic.sh            # full read-only sweep
#   sudo ./warren-net-diagnostic.sh       # also dumps the nftables ruleset
#
# Expected AFTER the F1 fix, while connected with an excluded process:
#   - `ip rule show` lists:  49: from all fwmark 0x6d6f6c65 lookup main
#                            50: from all to <exit_ip>/32 lookup main
#                            51: from all lookup 100
#   - table 100 has 0.0.0.0/1 and 128.0.0.0/1 dev <tun>
#   - `warren-exclude curl https://api.ipify.org` returns the REAL IP.

set -u

SPLIT_TUNNEL_FWMARK="0x6d6f6c65"   # = mullvad_types::TUNNEL_FWMARK
WARREN_ROUTE_TABLE="100"           # = default_route_split::ROUTE_TABLE

hr()      { printf '\n========== %s ==========\n' "$1"; }
have()    { command -v "$1" >/dev/null 2>&1; }
run()     { printf '$ %s\n' "$*"; "$@" 2>&1; echo; }

hr "Policy routing rules (ip rule)"
if have ip; then
  run ip rule show
  echo "--- F1 check: is the split-tunnel exclude rule present? ---"
  if ip rule show | grep -q "$SPLIT_TUNNEL_FWMARK"; then
    echo "OK  : fwmark $SPLIT_TUNNEL_FWMARK rule found (excluded traffic -> main table)"
  else
    echo "MISS: no fwmark $SPLIT_TUNNEL_FWMARK rule -> excluded traffic is black-holed (F1 not active)"
  fi
else
  echo "ip(8) not found"
fi

hr "Warren tunnel table ($WARREN_ROUTE_TABLE)"
have ip && run ip route show table "$WARREN_ROUTE_TABLE"

hr "Main routing table"
have ip && run ip route show table main

hr "Default route (physical egress)"
have ip && run ip route show default

hr "nftables ruleset (split-tunnel mark / masquerade / drop)"
if have nft; then
  if [ "$(id -u)" -eq 0 ]; then
    # Highlight the rules relevant to F1/F2 without altering anything.
    run nft list ruleset
  else
    echo "Re-run with sudo to read the nft ruleset (CAP_NET_ADMIN required)."
  fi
else
  echo "nft(8) not found"
fi

hr "DNS resolver state (F2)"
if have resolvectl; then
  run resolvectl status
elif have systemd-resolve; then
  run systemd-resolve --status
fi
echo "--- /etc/resolv.conf ---"
[ -r /etc/resolv.conf ] && run cat /etc/resolv.conf
echo "--- F2 check: is the system pointed at the unreachable filtering IP? ---"
if grep -qs "100.64.0" /etc/resolv.conf 2>/dev/null || (have resolvectl && resolvectl status 2>/dev/null | grep -q "100.64.0"); then
  echo "WARN: a 100.64.0.x resolver is configured. If no in-tunnel filtering"
  echo "      resolver answers there, ALL resolution breaks (F2)."
else
  echo "OK  : no 100.64.0.x resolver configured."
fi

hr "DNS resolution probe (read-only)"
for host in example.com api.ipify.org; do
  if have dig; then
    printf '$ dig +short +time=3 +tries=1 %s\n' "$host"; dig +short +time=3 +tries=1 "$host" 2>&1; echo
  elif have getent; then
    run getent hosts "$host"
  fi
done

hr "Public IP via tunnel (read-only)"
if have curl; then
  run curl -s --max-time 8 https://api.ipify.org
fi

hr "Split-tunnel egress probe (F1) — run manually"
cat <<'NOTE'
To confirm F1 end to end, with an excluded process (needs the daemon's
split-tunnel exclusion mechanism, i.e. the `mullvad-exclude`/`warren-exclude`
wrapper):

    warren-exclude curl -s --max-time 8 https://api.ipify.org   # expect REAL IP
    curl          -s --max-time 8 https://api.ipify.org         # expect EXIT IP

PASS = the two IPs differ (excluded traffic leaves via the physical link).
NOTE

hr "Done (no changes were made)"
