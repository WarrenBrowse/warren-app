#!/usr/bin/env bash
# shellcheck shell=bash
# Build universal installer for both ARM and x64.

set -eu

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR/../.."

CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"target"}

# If enabled, build in release mode with optimizations enabled
OPTIMIZE="false"

source scripts/utils/log

# The wrapper's file name, icon and version resources are production's
# (windows-installer/Cargo.toml and build.rs), so a beta or staging build would
# hand users an installer that names and draws itself as the production app.
if [[ "${WARREN_PRODUCT_ENV:-prod}" != "prod" ]]; then
    log_error "The universal Windows installer carries the production identity; refusing a ${WARREN_PRODUCT_ENV} build"
    exit 1
fi

echo "Computing build version..."
PRODUCT_VERSION=$(cargo run -q --bin mullvad-version)
log_header "Building universal Windows installer for Warren VPN $PRODUCT_VERSION"

while [[ "$#" -gt 0 ]]; do
    case $1 in
        --x64-installer)
            export WIN_X64_INSTALLER="$2"
            shift 2
            ;;
        --arm64-installer)
            export WIN_ARM64_INSTALLER="$2"
            shift 2
            ;;
        --optimize)
            OPTIMIZE="true"
            shift
            ;;
        *)
            log_error "Unknown argument: $1"
            exit 1
            ;;
    esac
done

CARGO_ARGS=()

if [[ "$OPTIMIZE" == "true" ]]; then
    CARGO_ARGS+=(--release)
    RUST_BUILD_MODE="release"
else
    RUST_BUILD_MODE="debug"
fi

if [[ "$OPTIMIZE" == "true" && "$PRODUCT_VERSION" != *"-dev-"* ]]; then
    CARGO_ARGS+=(--locked)
fi

if [[ -z ${WIN_X64_INSTALLER-} ]] || [[ -z ${WIN_ARM64_INSTALLER-} ]]; then
    log_error "Must provide --x64-installer and --arm64-installer"
    exit 1
fi

cargo build "${CARGO_ARGS[@]}" -p windows-installer --target x86_64-pc-windows-msvc

dest="dist/WarrenVPN-${PRODUCT_VERSION}.exe"

cp "$CARGO_TARGET_DIR/x86_64-pc-windows-msvc/${RUST_BUILD_MODE}/windows-installer.exe" "$dest"

log_success "Built universal installer: $dest"
