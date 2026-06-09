#!/usr/bin/env bash
set -eu

chmod u+s "/usr/bin/warren-exclude"

# Management socket access group. The daemon restricts the management
# socket (which can return the wallet seed phrase) to root plus this
# group; without it the socket falls back to world-accessible. Create the
# group and add the human who triggered the install so the GUI can still
# connect (group changes take effect on next login).
if ! getent group warren >/dev/null 2>&1; then
    groupadd --system warren || echo "Failed to create 'warren' group"
fi
install_user=""
if [ -n "${PKEXEC_UID:-}" ]; then
    install_user="$(id -un "${PKEXEC_UID}" 2>/dev/null || true)"
fi
install_user="${install_user:-${SUDO_USER:-}}"
if [ -n "${install_user}" ] && [ "${install_user}" != "root" ]; then
    usermod -aG warren "${install_user}" 2>/dev/null \
        || echo "Could not add ${install_user} to 'warren' group; add it manually for GUI access"
fi

systemctl enable "/usr/lib/systemd/system/warren-daemon.service"
systemctl start warren-daemon.service || echo "Failed to start warren-daemon.service"
systemctl enable "/usr/lib/systemd/system/warren-early-boot-blocking.service"

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
