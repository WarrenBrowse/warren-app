#!/usr/bin/env bash
set -eu

function remove_logs_and_cache {
  rm -r --interactive=never /var/log/warren-vpn/ || \
    echo "Failed to remove warren-vpn logs"
  rm -r --interactive=never /var/cache/warren-vpn/ || \
    echo "Failed to remove warren-vpn cache"
}

function get_home_dirs {
  if [[ -f "/etc/passwd" ]] && command -v cut > /dev/null; then
      cut -d: -f6 /etc/passwd
  fi
}

function clear_gpu_cache {
  local home_dirs
  home_dirs=$(get_home_dirs)

  for home_dir in $home_dirs; do
      local gpu_cache_dir="$home_dir/.config/Warren VPN/GPUCache"
      if [[ -d "$gpu_cache_dir" ]]; then
          echo "Clearing GPU cache in $gpu_cache_dir"
          rm -r --interactive=never "$gpu_cache_dir" || \
              echo "Failed to clear GPU cache"
      fi
  done
}

function remove_config {
  rm -r --interactive=never /etc/warren-vpn || \
    echo "Failed to remove warren-vpn config"

  # Remove app settings and auto-launcher for all users. This doesn't respect XDG_CONFIG_HOME due
  # to the complexity required.
  local home_dirs
  home_dirs=$(get_home_dirs)
  for home_dir in $home_dirs; do
      local warren_dir="$home_dir/.config/Warren VPN"
      if [[ -d "$warren_dir" ]]; then
          echo "Removing warren-vpn app settings from $warren_dir"
          rm -r --interactive=never "$warren_dir" || \
              echo "Failed to remove warren-vpn app settings"
      fi

      local autostart_path="$home_dir/.config/autostart/warren-vpn.desktop"
      # warren-vpn.desktop can be both a file or a symlink.
      if [[ -f "$autostart_path" || -L "$autostart_path" ]]; then
          echo "Removing warren-vpn app autostart file $autostart_path"
          rm --interactive=never "$autostart_path" || \
              echo "Failed to remove warren-vpn autostart file"
      fi
  done
}

# checking what kind of an action is taking place
case $@ in
  # apt purge passes "purge"
  "purge")
    remove_logs_and_cache
    remove_config
    ;;
  # apt remove passes "remove"
  "remove")
    ;;
  # dnf remove passes a 0
  "0")
    remove_logs_and_cache
    remove_config
    ;;
esac

# Different electron versions can have incompatible GPU caches. Clearing it on upgrades makes sure
# the same cache is not used across versions.
clear_gpu_cache

# Remove apparmor profile
if apparmor_parser -R /etc/apparmor.d/warren &>/dev/null; then
    echo "Removing apparmor profile"
    rm -f /etc/apparmor.d/warren || echo "Failed to delete apparmor profile"
fi
