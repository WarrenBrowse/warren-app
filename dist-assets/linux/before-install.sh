#!/usr/bin/env bash
set -eu

if which systemctl &> /dev/null && systemctl is-system-running | grep -vq offline &> /dev/null; then
    if systemctl status warren-daemon &> /dev/null; then
        # Arm the detached update guard first: it stages its own copy outside
        # this package and registers a systemd transient timer, so a machine
        # whose daemon never returns resets its own firewall. Not fatal.
        /opt/Warren\ VPN/resources/warren-setup arm-deadman \
            || echo "Failed to arm the detached update guard"
        # FATAL since 2026-08-08, matching Windows: this is what seals the host
        # for the swap. An installer that could not arm the protection must not
        # go on to stop the daemon.
        if ! /opt/Warren\ VPN/resources/warren-setup prepare-restart; then
            echo "Failed to send 'prepare-restart' to the daemon; aborting the install"
            /opt/Warren\ VPN/resources/warren-setup disarm-deadman || true
            exit 1
        fi
        systemctl stop warren-daemon.service
        systemctl disable warren-daemon.service
        systemctl disable warren-early-boot-blocking.service || true
        cp /var/log/warren-vpn/daemon.log /var/log/warren-vpn/old-install-daemon.log \
            || echo "Failed to copy old daemon log"
    fi
fi

rm -f /var/cache/warren-vpn/relays.json
rm -f /var/cache/warren-vpn/api-ip-address.txt
