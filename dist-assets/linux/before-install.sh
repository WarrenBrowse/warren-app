#!/usr/bin/env bash
set -eu

# The postinstall registers and starts the daemon through systemd. On a host
# whose init is something else (MX Linux, antiX, Devuan) it either fails half
# way, leaving the package half-configured, or completes with no daemon ever
# started, and the app then waits for its system service forever. Refuse
# before anything is unpacked, and name the package built for those systems.
if ! command -v systemctl &> /dev/null || [ "$(systemctl is-system-running 2> /dev/null)" = offline ]; then
    echo "Warren VPN: this package needs systemd, and this system does not run it." >&2
    echo "Install the package whose name ends in -sysvinit.deb instead, from the same download page." >&2
    exit 1
fi

if systemctl status warren-daemon &> /dev/null; then
    # Arm the detached update guard first: it stages its own copy outside
    # this package and registers a systemd transient timer, so a machine
    # whose daemon never returns resets its own firewall. Not fatal.
    /opt/Warren\ VPN/resources/warren-setup arm-deadman \
        || echo "Failed to arm the detached update guard"
    # FATAL since 2026-08-08, matching Windows: this is what seals the host
    # for the swap. An installer that could not arm the protection must not
    # go on to stop the daemon.
    # Status 3 is warren-setup's DaemonNotRunning: the daemon went away between
    # the check above and the seal, which leaves nothing holding the host.
    seal_status=0
    /opt/Warren\ VPN/resources/warren-setup prepare-restart || seal_status=$?
    if [ "$seal_status" -ne 0 ] && [ "$seal_status" -ne 3 ]; then
        echo "Failed to send 'prepare-restart' to the daemon; aborting the install"
        echo "Stop it with 'systemctl stop warren-daemon.service', then install again."
        /opt/Warren\ VPN/resources/warren-setup disarm-deadman || true
        exit 1
    fi
    systemctl stop warren-daemon.service
    systemctl disable warren-daemon.service
    systemctl disable warren-early-boot-blocking.service || true
    cp /var/log/warren-vpn/daemon.log /var/log/warren-vpn/old-install-daemon.log \
        || echo "Failed to copy old daemon log"
fi

rm -f /var/cache/warren-vpn/relays.json
rm -f /var/cache/warren-vpn/api-ip-address.txt
