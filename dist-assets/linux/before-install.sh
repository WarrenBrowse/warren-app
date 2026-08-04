#!/usr/bin/env bash
set -eu

if which systemctl &> /dev/null && systemctl is-system-running | grep -vq offline &> /dev/null; then
    if systemctl status warren-daemon &> /dev/null; then
        /opt/Warren\ VPN/resources/warren-setup prepare-restart || true
        systemctl stop warren-daemon.service
        systemctl disable warren-daemon.service
        systemctl disable warren-early-boot-blocking.service || true
        cp /var/log/warren-vpn/daemon.log /var/log/warren-vpn/old-install-daemon.log \
            || echo "Failed to copy old daemon log"
    fi
fi

rm -f /var/cache/warren-vpn/relays.json
rm -f /var/cache/warren-vpn/api-ip-address.txt
