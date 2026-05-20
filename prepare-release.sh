#!/usr/bin/env bash
#
# Prepare a Warren VPN release: verify working tree, bump version, commit,
# create a signed tag. Pushing the tag triggers the release.yml workflow.
#
# Usage:
#   ./prepare-release.sh [--desktop] [--dry-run] <VERSION>
#
# Examples:
#   ./prepare-release.sh --desktop 2026.5.0
#   ./prepare-release.sh --desktop --dry-run 2026.5.0-beta1
#
# Notes:
#   - --desktop is required (the only release surface in M4.H.D scope).
#     The flag exists for forward compatibility with future surfaces
#     (--android, --ios) and to mirror the upstream Mullvad CLI shape.
#   - <VERSION> must follow CalVer YYYY.MILESTONE.PATCH[-prerelease]
#     (e.g., 2026.5.0, 2026.5.0-beta1) or SemVer V.X.Y[-prerelease].
#   - A GPG/SSH signing key must be configured via git config user.signingkey
#     and the tag command will fail otherwise (signed tags are mandatory).

set -eu

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

source scripts/utils/log

DESKTOP="false"
DRY_RUN="false"
VERSION=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --desktop) DESKTOP="true"; shift;;
        --dry-run) DRY_RUN="true"; shift;;
        -h|--help)
            grep '^#' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        -*)
            log_error "Unknown flag: $1"
            exit 1
            ;;
        *)
            if [[ -n "$VERSION" ]]; then
                log_error "VERSION already set to '$VERSION', got extra positional arg '$1'"
                exit 1
            fi
            VERSION="$1"
            shift
            ;;
    esac
done

if [[ "$DESKTOP" != "true" ]]; then
    log_error "Pass --desktop to prepare a desktop release."
    log_error "Usage: ./prepare-release.sh --desktop [--dry-run] <VERSION>"
    exit 1
fi

if [[ -z "$VERSION" ]]; then
    log_error "Missing VERSION argument."
    log_error "Usage: ./prepare-release.sh --desktop [--dry-run] <VERSION>"
    exit 1
fi

# Validate version format: CalVer YYYY.MILESTONE[.PATCH][-pre] or SemVer X.Y.Z[-pre]
if ! [[ "$VERSION" =~ ^(20[2-9][0-9]\.[0-9]+(\.[0-9]+)?|[0-9]+\.[0-9]+\.[0-9]+)(-[A-Za-z0-9.-]+)?$ ]]; then
    log_error "Invalid VERSION format: '$VERSION'"
    log_error "Expected CalVer (e.g., 2026.5.0, 2026.5.0-beta1) or SemVer (e.g., 0.1.0)."
    exit 1
fi

TAG_NAME="v${VERSION}"

log_header "Preparing Warren VPN release ${VERSION}"

# Refuse on a dirty working tree
if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
    log_error "Working tree is dirty. Commit or stash changes before releasing."
    git status --short
    exit 1
fi

# Refuse if the tag already exists locally
if git rev-parse -q --verify "refs/tags/${TAG_NAME}" >/dev/null; then
    log_error "Tag '${TAG_NAME}' already exists locally. Choose a different VERSION or delete the existing tag."
    exit 1
fi

# Verify a signing key is configured (we mandate signed tags)
if ! git config --get user.signingkey >/dev/null; then
    log_error "git config user.signingkey is not set. Configure GPG/SSH signing before releasing."
    exit 1
fi

PKG_JSON="desktop/packages/mullvad-vpn/package.json"
if [[ ! -f "$PKG_JSON" ]]; then
    log_error "Could not find ${PKG_JSON}"
    exit 1
fi

log_info "Updating ${PKG_JSON} version -> ${VERSION}"
node -e "
const fs = require('fs');
const path = '${PKG_JSON}';
const pkg = JSON.parse(fs.readFileSync(path, 'utf8'));
pkg.version = '${VERSION}';
fs.writeFileSync(path, JSON.stringify(pkg, null, 2) + '\n');
"

if [[ "$DRY_RUN" == "true" ]]; then
    log_info "[dry-run] Would commit + tag ${TAG_NAME}. Diff:"
    git --no-pager diff -- "$PKG_JSON"
    log_info "[dry-run] Reverting package.json change."
    git checkout -- "$PKG_JSON"
    exit 0
fi

log_info "Committing version bump"
git add "$PKG_JSON"
git commit -m "release: ${VERSION}"

log_info "Creating signed tag ${TAG_NAME}"
git tag -s "${TAG_NAME}" -m "Warren VPN ${VERSION}"

log_success "**********************************"
log_success ""
log_success " Prepared release ${VERSION}"
log_success ""
log_success " Next steps:"
log_success "   git push origin main"
log_success "   git push origin ${TAG_NAME}"
log_success ""
log_success " Pushing the tag will trigger .github/workflows/release.yml,"
log_success " which builds signed artifacts and creates a draft GitHub Release."
log_success "**********************************"
