#!/usr/bin/env bash
set -eu

chmod u+s "/usr/bin/warren-exclude"

# No management-socket group is provisioned: the daemon exposes the socket to
# local users and gates wallet/secret RPCs per-uid (matching upstream Mullvad's
# threat model). Group membership only applies at the next login, so a
# group-based install would leave the GUI unable to reach the daemon until the
# user logs out and back in. Operators wanting kernel-enforced restriction can
# create a group and set WARREN_MANAGEMENT_SOCKET_GROUP on the service.

systemctl enable "/usr/lib/systemd/system/warren-daemon.service"
systemctl start warren-daemon.service || echo "Failed to start warren-daemon.service"
systemctl enable "/usr/lib/systemd/system/warren-early-boot-blocking.service"

# The system bus reads /usr/share/dbus-1/system.d once, and denies ownership of
# any name it has no policy for. Without this reload the NetworkManager VPN
# plugin cannot take the name NetworkManager calls it on until the next boot,
# and the desktop's network indicator would keep showing no VPN. Reloading is
# cheap and disturbs no running connection.
systemctl reload dbus || echo "Failed to reload dbus; the VPN indicator starts working after a reboot"

# Check if the system supports a new-enough AppArmor version.
function supported_apparmor() {
    [[ -e /etc/apparmor.d/abi/4.0 ]]
}

if supported_apparmor; then
    # Install our AppArmor profile and try to reload AppArmor.
    # The AppArmor profile allow Electron sandbox to work.
    # This disables user namespace restrictions.
    echo "Creating apparmor profile"
    cp /opt/Warren\ VPN/resources/apparmor_warren /etc/apparmor.d/warren
    apparmor_parser -r /etc/apparmor.d/warren || echo "Failed to reload apparmor profile"
fi
