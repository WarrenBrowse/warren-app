#!/usr/bin/env bash
#
# Codemagic `windows-release` workflow: the NSIS/MSI installers of one channel,
# staged under their release names, for release.yml's build-windows job to
# publish. Runs on a native x64 machine; the x86_64 target stays explicit so
# the output paths are the ones build.sh and the stage script expect.
#
#   ci/codemagic/windows-release.sh <prepare|sources|build|stage>
#
# One Codemagic step per phase, so the GitHub job shows which phase is running
# and each phase's log as soon as it ends: a running step's log cannot be read.
# Every phase re-enters the tool environment, which costs nothing once the
# first phase (or the cache) has put the tools on disk.
set -euo pipefail
source ci/codemagic/windows-common.sh

enter_tools() {
    install_toolchains
    timed install_node "$(node -p "require('./desktop/package.json').volta.node")"
}

case "${1:?usage: windows-release.sh <prepare|sources|build|stage>}" in
    prepare)
        prepare_tree
        enter_tools
        ;;
    sources)
        enter_tools
        timed checkout_siblings
        bash ci/check-quinn-fork-lock.sh
        timed bash ci/fetch-warren-relays.sh "$WARREN_API_URL"
        ;;
    build)
        enter_tools
        # Azure Trusted Signing keeps the key in Azure and signs through
        # signtool's dlib, so no key material reaches the machine. Its
        # credentials come from the app's `windows_signing` Codemagic variable
        # group; without them the build is unsigned, which is the state every
        # Warren release is in today.
        sign=()
        if [ -n "${AZURE_CLIENT_ID:-}" ]; then
            ts="$CM_TOOLS/trusted-signing"
            nuget install Microsoft.Trusted.Signing.Client -OutputDirectory "$(cygpath -w "$ts")" \
                -Source https://api.nuget.org/v3/index.json > /dev/null
            dlib="$(find "$ts" -path '*x64*' -name Azure.CodeSigning.Dlib.dll | head -1)"
            [ -n "$dlib" ] || { echo "::error::Azure.CodeSigning.Dlib.dll not found" >&2; exit 1; }
            meta="$CM_TOOLS/trusted-signing-metadata.json"
            printf '{"Endpoint":"%s","CodeSigningAccountName":"%s","CertificateProfileName":"%s"}\n' \
                "$TRUSTED_SIGNING_ENDPOINT" "$TRUSTED_SIGNING_ACCOUNT" "$TRUSTED_SIGNING_PROFILE" > "$meta"
            WARREN_TS_DLIB="$(cygpath -w "$dlib")"
            WARREN_TS_METADATA="$(cygpath -w "$meta")"
            export WARREN_TS_DLIB WARREN_TS_METADATA
            export WARREN_WIN_SIGN_BACKEND=trusted-signing
            sign=(--sign)
        else
            echo "::warning::AZURE_CLIENT_ID not set; producing an unsigned build"
        fi
        export TARGETS=x86_64-pc-windows-msvc
        [ "$WARREN_CHANNEL" = beta ] && export WARREN_PRODUCT_ENV=beta
        timed ./build.sh --optimize "${sign[@]}"
        ;;
    stage)
        stage_env=prod
        [ "$WARREN_CHANNEL" = beta ] && stage_env=beta
        bash ci/stage-release-assets.sh windows "$WARREN_VERSION" "$stage_env"
        export_outputs release-assets/*
        ;;
    *)
        echo "unknown phase: $1" >&2
        exit 2
        ;;
esac
