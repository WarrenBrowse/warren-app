# shellcheck shell=bash
#
# Tree preparation shared by the Codemagic Windows entry points. Expects the
# build inputs codemagic.yaml exports: WARREN_SHA, WARREN_CHANNEL (prod|beta),
# WARREN_VERSION (release version without channel prefix, empty for a dev
# build) and WARREN_API_URL.

# shellcheck source=ci/codemagic/windows-env.sh
source ci/codemagic/windows-env.sh

prepare_tree() {
    local head
    head="$(git rev-parse HEAD)"
    if [ "$head" != "$WARREN_SHA" ]; then
        echo "::error::checked out $head but the build was asked for $WARREN_SHA" >&2
        return 1
    fi
    case "$WARREN_CHANNEL" in
        prod | beta) ;;
        *) echo "::error::unknown channel '$WARREN_CHANNEL' (prod|beta)" >&2; return 1 ;;
    esac
    # Only the submodules a Windows build reads, at the gitlinks of this commit
    # (Codemagic initialised them for the branch head it cloned first).
    git submodule update --init --depth=1 dist-assets/binaries windows

    # A release version IS its tag: the daemon and the app bake it at compile
    # time from these files, and mullvad-version/build.rs only drops the -dev
    # suffix when a release tag naming that version points at HEAD. The tag is
    # created locally, as the headless stamp action does; it is never pushed.
    if printf '%s' "${WARREN_VERSION:-}" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
        printf '%s\n' "$WARREN_VERSION" > dist-assets/desktop-product-version.txt
        printf '%s\n' "$WARREN_VERSION" > dist-assets/android-version-name.txt
        local tag="v$WARREN_VERSION"
        [ "$WARREN_CHANNEL" = beta ] && tag="beta-v$WARREN_VERSION"
        git tag -f "$tag" > /dev/null
        echo "stamped version $WARREN_VERSION ($tag at HEAD)"
    else
        echo "no release version given: keeping the committed version (-dev build)"
    fi
}

install_toolchains() {
    timed install_rustup
    rustup target add x86_64-pc-windows-msvc i686-pc-windows-msvc
    timed install_protoc
    timed install_zig
    timed install_go
    add_msbuild
}

checkout_siblings() {
    checkout_sibling warrenguard .warrenguard-version
    checkout_sibling warren-contract .warren-contract-version
    checkout_sibling warren-sdk-rs .warren-sdk-version
}
