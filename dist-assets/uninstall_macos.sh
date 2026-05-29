#!/usr/bin/env bash

set -ue

ASSUMEYES="n"

while [[ "$#" -gt 0 ]]; do
    case $1 in
        --yes) ASSUMEYES="y";;
        *)
            echo "Unknown parameter: $1"
            exit 1
            ;;
    esac
    shift
done

[[ $ASSUMEYES == "y" ]] || read -r -p "Are you sure you want to stop and uninstall Warren VPN? (y/n) "
if [[ $ASSUMEYES == "y" || "$REPLY" =~ [Yy]$ ]]; then
    echo "Uninstalling Warren VPN ..."
else
    echo "Aborting uninstall"
    exit 0
fi

echo "Stopping GUI process ..."
sudo pkill -x "Warren VPN" || echo "No GUI process found"

echo "Stopping and unloading warren-daemon system daemon ..."
DAEMON_PLIST_PATH="/Library/LaunchDaemons/com.warrenbrowse.vpn.daemon.plist"
sudo launchctl unload -w "$DAEMON_PLIST_PATH"
sudo rm -f "$DAEMON_PLIST_PATH"

echo "Resetting firewall"
sudo /Applications/Warren\ VPN.app/Contents/Resources/warren-setup reset-firewall || echo "Failed to reset firewall"
sudo /Applications/Warren\ VPN.app/Contents/Resources/warren-setup remove-device || echo "Failed to remove device from account"

echo "Removing zsh shell completion symlink ..."
sudo rm -f /usr/local/share/zsh/site-functions/_warren

echo "Removing fish shell completion symlink ..."

sudo rm -f "/opt/homebrew/share/fish/vendor_completions.d/warren.fish"
sudo rm -f "/usr/local/share/fish/vendor_completions.d/warren.fish"

echo "Removing CLI symlinks from /usr/local/bin/ ..."
sudo rm -f /usr/local/bin/warren /usr/local/bin/warren-problem-report

echo "Removing app from /Applications ..."
sudo rm -rf /Applications/Warren\ VPN.app
sudo pkgutil --forget com.warrenbrowse.vpn || true

echo "Removing login item ..."
osascript -e 'tell application "System Events" to delete login item "Warren VPN"' 2>/dev/null || true

[[ $ASSUMEYES == "y" ]] || read -r -p "Do you want to delete the log and cache files the app has created? (y/n) "
if [[ $ASSUMEYES == "y" || "$REPLY" =~ [Yy]$ ]]; then
    sudo rm -rf /var/log/warren-vpn /var/root/Library/Caches/warren-vpn /Library/Caches/warren-vpn
    for user in /Users/*; do
        user_log_dir="$user/Library/Logs/Warren VPN"
        if [[ -d "$user_log_dir" ]]; then
            echo "Deleting GUI logs at $user_log_dir"
            sudo rm -rf "$user_log_dir"
        fi
    done
fi

[[ $ASSUMEYES == "y" ]] || read -r -p "Do you want to delete the Warren VPN settings? (y/n) "
if [[ $ASSUMEYES == "y" || "$REPLY" =~ [Yy]$ ]]; then
    sudo rm -rf /etc/warren-vpn
    for user in /Users/*; do
        user_settings_dir="$user/Library/Application Support/Warren VPN"
        if [[ -d "$user_settings_dir" ]]; then
            echo "Deleting GUI settings at $user_settings_dir"
            sudo rm -rf "$user_settings_dir"
        fi
    done
fi

# When run from a non-standard directory, like when detecting that the app bundle is gone,
# we must also delete the uninstall script itself
rm -f "$0" || true
