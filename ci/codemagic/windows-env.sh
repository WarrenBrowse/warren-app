# shellcheck shell=bash
#
# Build environment for a Codemagic windows_x2 machine, sourced by the
# ci/codemagic/windows-*.sh entry points (Git Bash).
#
# Every build starts on a fresh VM (Windows Server 2022, x64, VS 2022 17.14,
# Windows SDK 10.0.26100, Git, Node 20, Python 3.9) that carries none of the
# tools the self-hosted runner had installed, so each one is fetched here at
# the version the GitHub jobs use: rustup (the toolchain itself comes from
# rust-toolchain.toml), protoc 23.x (arduino/setup-protoc@v3's default series),
# zig 0.14.1 and Go 1.21.3 (mullvad-build-env), and the Node version pinned in
# desktop/package.json's volta block.
#
# What codemagic.yaml caches between builds is downloads only: the tools under
# $CM_TOOLS, rustup's toolchains, cargo's registry and git sources, and the npm
# and Electron download caches. Never a build output: a release must not
# inherit an intermediate from another channel (the beta-v1.1.9 winfw.dll
# carried prod WFP keys out of a restored cache), so target/ and windows/*/bin
# are rebuilt on every run.
#
# Each tool lives in a directory named after its version, so a restored cache
# is reused only when it holds exactly the pinned version: bumping a pin here or
# in rust-toolchain.toml installs the new one and drops the old from the cache.
# `scripts/codemagic-cache.sh clear warren-app` in the workspace empties it.

set -euo pipefail

# The step's PowerShell hands bash a stdin pipe it never closes, and Windows
# PowerShell 5.1 started from bash reads its stdin to the end before running
# -Command: scripts/utils/host's architecture probe then waits forever. That
# silently hung the first Codemagic release build for 42 minutes before
# build.sh printed its first line. Nothing here reads stdin.
exec < /dev/null


# Codemagic's PowerShell profile (posh-sshell, Start-SshAgent, the build's
# variables) makes every Windows PowerShell that loads it hold its caller's
# output open: `x="$(powershell.exe -Command 'Write-Output ok')"` never
# returned, the same call with -NoProfile did in 2 s (self-test of
# 2026-09-26). The build starts PowerShell without -NoProfile in places it
# does not own (msbuild's NMake steps, tool scripts), and whatever waits for
# that output then hangs with no CPU. Nothing in these builds needs the
# profile, so it is moved aside for the rest of the VM's life.
neutralize_powershell_profile() {
    local profile
    profile="$(powershell.exe -NoProfile -NonInteractive -Command 'Write-Output $PROFILE.CurrentUserCurrentHost' | tr -d '\r')"
    if [ -n "$profile" ] && [ -f "$(cygpath -u "$profile")" ]; then
        mv -f "$(cygpath -u "$profile")" "$(cygpath -u "$profile").off"
        echo "PowerShell profile moved aside: $profile"
    fi
}
neutralize_powershell_profile

CM_TOOLS="${CM_TOOLS:-$HOME/cm-tools}"
mkdir -p "$CM_TOOLS"

PROTOC_VERSION=23.4
ZIG_VERSION=0.14.1
GO_VERSION=1.21.3

fetch() { # fetch <url> <dest>
    curl -fsSL --retry 5 --retry-all-errors --connect-timeout 30 -o "$2" "$1"
}

# Run one setup phase and say how long it took, so the effect of the cache
# can be read off any build log.
timed() { # timed <function> [args...]
    local start=$SECONDS
    "$@"
    echo "[timing] $1: $((SECONDS - start))s"
}

# Unpack <zip> into $CM_TOOLS/<dir> unless a restored cache already holds it,
# and drop every other version of the same tool (<prefix>*) from the cache.
# With <sha256>, the download is checked before it is unpacked, and therefore
# before it can ever reach the cache.
tool_dir() { # tool_dir <prefix> <dir> <url> [<dir inside the zip> [<sha256>]]
    local prefix="$1" dir="$CM_TOOLS/$2" url="$3" inner="${4:-}" sum="${5:-}" old
    for old in "$CM_TOOLS/$prefix"*; do
        [ -e "$old" ] && [ "$old" != "$dir" ] && rm -rf "$old"
    done
    if [ -d "$dir" ]; then
        echo "$2: from cache"
        return 0
    fi
    fetch "$url" "$CM_TOOLS/download.zip"
    [ -z "$sum" ] || echo "$sum  $CM_TOOLS/download.zip" | sha256sum -c -
    rm -rf "$CM_TOOLS/unpack" && mkdir -p "$CM_TOOLS/unpack"
    7z x -y -o"$(cygpath -w "$CM_TOOLS/unpack")" "$(cygpath -w "$CM_TOOLS/download.zip")" > /dev/null
    mv "$CM_TOOLS/unpack${inner:+/$inner}" "$dir"
    rm -rf "$CM_TOOLS/unpack" "$CM_TOOLS/download.zip"
    echo "$2: downloaded"
}

install_rustup() {
    export PATH="$HOME/.cargo/bin:$PATH"
    if ! command -v rustup > /dev/null 2>&1; then
        fetch https://win.rustup.rs/x86_64 "$CM_TOOLS/rustup-init.exe"
        "$CM_TOOLS/rustup-init.exe" -y --default-toolchain none --profile minimal --no-modify-path
        rm -f "$CM_TOOLS/rustup-init.exe"
    fi
    # Installs the channel rust-toolchain.toml names, in the current directory
    # (a no-op when the cache restored it), then drops any other toolchain.
    rustup toolchain install
    local active other
    active="$(rustup show active-toolchain | cut -d' ' -f1)"
    for other in $(rustup toolchain list | cut -d' ' -f1); do
        [ "$other" = "$active" ] || rustup toolchain uninstall "$other"
    done
    echo "rust: $active"
}

install_protoc() {
    tool_dir protoc- "protoc-$PROTOC_VERSION" \
        "https://github.com/protocolbuffers/protobuf/releases/download/v$PROTOC_VERSION/protoc-$PROTOC_VERSION-win64.zip"
    export PATH="$CM_TOOLS/protoc-$PROTOC_VERSION/bin:$PATH"
    export PROTOC="$CM_TOOLS/protoc-$PROTOC_VERSION/bin/protoc.exe"
    protoc --version
}

install_zig() {
    local name="zig-x86_64-windows-$ZIG_VERSION"
    tool_dir zig- "zig-$ZIG_VERSION" "https://ziglang.org/download/$ZIG_VERSION/$name.zip" "$name"
    export PATH="$CM_TOOLS/zig-$ZIG_VERSION:$PATH"
    zig version
}

install_go() {
    tool_dir go- "go-$GO_VERSION" "https://go.dev/dl/go$GO_VERSION.windows-amd64.zip" go
    export PATH="$CM_TOOLS/go-$GO_VERSION/bin:$PATH"
    go version
}

install_node() { # install_node <version>
    local version="$1" name="node-v$1-win-x64" sum=""
    if [ ! -d "$CM_TOOLS/node-$version" ]; then
        fetch "https://nodejs.org/dist/v$version/SHASUMS256.txt" "$CM_TOOLS/node.sums"
        sum="$(grep " $name.zip\$" "$CM_TOOLS/node.sums" | cut -d' ' -f1)"
        rm -f "$CM_TOOLS/node.sums"
        [ -n "$sum" ] || { echo "::error::$name.zip not in nodejs.org SHASUMS256" >&2; return 1; }
    fi
    tool_dir node- "node-$version" "https://nodejs.org/dist/v$version/$name.zip" "$name" "$sum"
    export PATH="$CM_TOOLS/node-$version:$PATH"
    node --version
    npm --version
}

add_msbuild() {
    local vswhere msbuild
    vswhere="/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe"
    msbuild="$("$vswhere" -latest -products '*' -requires Microsoft.Component.MSBuild \
        -find 'MSBuild\**\Bin\amd64\MSBuild.exe' | tr -d '\r' | head -1)"
    if [ -z "$msbuild" ]; then
        echo "::error::no MSBuild found by vswhere" >&2
        return 1
    fi
    msbuild="$(cygpath -u "$msbuild")"
    export PATH="${msbuild%/*}:$PATH"
    msbuild.exe -version -nologo | tail -1
}

# Clone a public WarrenBrowse sibling next to this checkout, at its pin.
checkout_sibling() { # checkout_sibling <repo> <pin file>
    local repo="$1" sha
    sha="$(tr -d '[:space:]' < "$2")"
    if ! printf '%s' "$sha" | grep -Eq '^[0-9a-f]{40}$'; then
        echo "::error::$2 must hold one full commit SHA, got '$sha'" >&2
        return 1
    fi
    rm -rf "../$repo"
    git clone --quiet --filter=blob:none "https://github.com/WarrenBrowse/$repo.git" "../$repo"
    git -C "../$repo" checkout --quiet --detach "$sha"
    echo "$repo @ $(git -C "../$repo" rev-parse HEAD)"
}

# Write outputs flat into cm-out/ with the checksum list the GitHub proxy
# (.github/actions/codemagic-build) verifies before anything is published.
export_outputs() { # export_outputs <file>...
    rm -rf cm-out
    mkdir -p cm-out
    cp -- "$@" cm-out/
    (cd cm-out && sha256sum -- * > codemagic.sha256)
    cat cm-out/codemagic.sha256
}
