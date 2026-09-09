#!/usr/bin/env bash
# Smoke check for the Warren build pipeline configuration.
#
# Validates that all branding-critical config files reference Warren and
# not Mullvad. Fast (sub-second), no compilation. Intended for CI
# pre-flight before running the heavier build.sh / electron-builder.
#
# Usage: bash scripts/dev/smoke-build.sh
#
# Exit codes:
#   0 - all checks pass (Warren-branded)
#   1 - at least one Mullvad branding residue detected
#
# WHAT BELONGS HERE, AND WHAT DOES NOT. A `grep` for a literal value is only
# valid while that value IS a literal. The beta/prod coexistence campaign
# turned the electron-builder names into expressions over `productEnv`
# (`executableName: productEnv.packageName`, `/opt/${productEnv.productName}/`),
# and three assertions here went red on correct code and stayed red through five
# releases, which is how a gate stops being read at all.
#
# So the rule for this file: assert only what cannot be refactored out from
# under it, which in practice means the ABSENCE of Mullvad strings plus names
# that are genuinely fixed. Anything whose value is computed per environment is
# asserted on the RESOLVED config instead, by
# `desktop/packages/mullvad-vpn/test/unit/product-env-linux-packaging.spec.ts`,
# which loads the packaging config once per environment and checks what actually
# ships. That suite is stricter than the greps it replaced: it covers prod, beta
# and staging, where a grep only ever saw one of them.

set -eu

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR/../.."

source scripts/utils/log

FAIL=0

assert_contains() {
    local file="$1"
    local needle="$2"
    local label="$3"
    if grep -qF "$needle" "$file"; then
        log_info "PASS [$label] $file contains '$needle'"
    else
        log_error "FAIL [$label] $file MISSING '$needle'"
        FAIL=1
    fi
}

assert_absent() {
    local file="$1"
    local needle="$2"
    local label="$3"
    if grep -qF "$needle" "$file"; then
        log_error "FAIL [$label] $file STILL contains '$needle'"
        FAIL=1
    else
        log_info "PASS [$label] $file does not contain '$needle'"
    fi
}

log_header "Warren build smoke check"

# build.sh
assert_contains build.sh 'Building Warren VPN' 'build-banner'
assert_contains build.sh 'warren-daemon' 'binary-name-daemon'
# The daemon package base name. NOT `warren-vpn-daemon_`: the name carries the
# environment between the base and the version (`warren-vpn-daemon-beta_1.1.29`),
# so the underscore is not adjacent to the base any more.
assert_contains build.sh 'warren-vpn-daemon' 'linux-package-prefix'
assert_absent build.sh 'mullvad-vpn-daemon' 'no-mullvad-package-prefix'
assert_contains build.sh 'WarrenVPN-' 'universal-installer-filename'
assert_absent build.sh 'MullvadVPN-' 'no-mullvad-installer-filename'
assert_absent build.sh 'git.p2p.legal' 'no-gitea-url'

# Universal Windows installer
assert_contains desktop/scripts/pack-universal-win.sh 'Warren VPN' 'universal-win-banner'
assert_contains desktop/scripts/pack-universal-win.sh 'WarrenVPN-' 'universal-win-dest'
assert_absent desktop/scripts/pack-universal-win.sh 'Mullvad VPN' 'no-mullvad-banner'
assert_absent desktop/scripts/pack-universal-win.sh 'MullvadVPN-' 'no-mullvad-dest'

# Electron builder distribution config.
#
# Only the Mullvad-absence check lives here. The app id, the product name, the
# artifact name, the Linux executable name and the /opt install directory are
# all resolved from `productEnv` and are asserted per environment, on the
# resolved config, by test/unit/product-env-linux-packaging.spec.ts.
assert_absent desktop/packages/mullvad-vpn/tasks/distribution.cjs "appId: 'net.mullvad.vpn'" 'no-mullvad-appid'
assert_absent desktop/packages/mullvad-vpn/tasks/distribution.cjs "productName: 'Mullvad VPN'" 'no-mullvad-product-name'

# desktop package.json
assert_contains desktop/packages/mullvad-vpn/package.json '"productName": "Warren VPN"' 'pkg-product-name'
assert_contains desktop/packages/mullvad-vpn/package.json 'warrenbrowse' 'pkg-repo'

# macOS pkg-scripts
assert_contains dist-assets/pkg-scripts/preinstall 'Warren VPN.app' 'preinstall-app'
assert_contains dist-assets/pkg-scripts/preinstall 'com.warrenbrowse.vpn.daemon.plist' 'preinstall-plist'
assert_contains dist-assets/pkg-scripts/postinstall 'com.warrenbrowse.vpn.daemon' 'postinstall-bundle'

# Linux unit + apparmor
[[ -f dist-assets/linux/warren-daemon.service ]] && \
    log_info "PASS [linux-service] warren-daemon.service exists" || { log_error "FAIL [linux-service] missing"; FAIL=1; }
[[ -f dist-assets/linux/apparmor_warren ]] && \
    log_info "PASS [linux-apparmor] apparmor_warren exists" || { log_error "FAIL [linux-apparmor] missing"; FAIL=1; }
[[ -f dist-assets/linux/warren-gui-launcher.sh ]] && \
    log_info "PASS [linux-launcher] warren-gui-launcher.sh exists" || { log_error "FAIL [linux-launcher] missing"; FAIL=1; }

# Cargo binaries
assert_contains mullvad-daemon/Cargo.toml 'name = "warren-daemon"' 'cargo-bin-daemon'
assert_contains mullvad-cli/Cargo.toml 'name = "warren"' 'cargo-bin-cli'

if [[ $FAIL -eq 0 ]]; then
    log_success "**********************************"
    log_success " All Warren branding checks passed"
    log_success "**********************************"
    exit 0
else
    log_error "**********************************"
    log_error " Warren branding smoke check FAILED"
    log_error "**********************************"
    exit 1
fi
