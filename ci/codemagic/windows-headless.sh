#!/usr/bin/env bash
#
# Codemagic `windows-headless` workflow: the daemon + CLI bundle (no GUI) of
# one channel, for release-daemon.yml's windows job to publish on warren-cli.
#
#   ci/codemagic/windows-headless.sh <prepare|sources|build|bundle>
#
# One Codemagic step per phase, for the reason windows-release.sh gives.
set -euo pipefail
source ci/codemagic/windows-common.sh

export WARREN_PRODUCT_ENV="$WARREN_CHANNEL"

case "${1:?usage: windows-headless.sh <prepare|sources|build|bundle>}" in
    prepare)
        prepare_tree
        install_toolchains
        ;;
    sources)
        install_toolchains
        timed checkout_siblings
        bash ci/check-quinn-fork-lock.sh
        # The install scripts the bundle carries live in the distribution repo.
        rm -rf warren-cli
        git clone --quiet --depth=1 https://github.com/WarrenBrowse/warren-cli.git warren-cli
        timed bash ci/fetch-warren-relays.sh "$WARREN_API_URL"
        ;;
    build)
        install_toolchains
        # winfw in Release: talpid-core links the release cargo build against
        # windows/winfw/bin/x64-Release/winfw.lib, and the script defaults to Debug.
        CPP_BUILD_MODES=Release CPP_BUILD_TARGETS=x64 timed ./build-windows-modules.sh build winfw
        # beta-v1.1.9 shipped a beta winfw.dll carrying PRODUCTION WFP keys, and
        # every Windows host updating into it was walled by its predecessor's
        # kill switch.
        bash ci/verify-winfw-salt.sh windows/winfw/bin/x64-Release/winfw.dll "$WARREN_CHANNEL"
        timed cargo build --release --target x86_64-pc-windows-msvc \
            -p mullvad-daemon -p mullvad-cli -p mullvad-setup -p mullvad-problem-report
        ;;
    bundle)
        install_toolchains
        bash ci/build-headless-bundle.sh windows "${WARREN_VERSION:-0.0.0}" warren-cli
        export_outputs release-assets/*
        ;;
    *)
        echo "unknown phase: $1" >&2
        exit 2
        ;;
esac
