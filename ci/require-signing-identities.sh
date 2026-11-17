#!/usr/bin/env bash
# Refuses a production release while any of its signing identities is missing.
# A production build must never ship unsigned, or signed with the debug key the
# beta channel uses; beta stays buildable without them.
#
#   bash ci/require-signing-identities.sh <prod|beta>
# The identities are read from the environment, under their secret names.
set -euo pipefail

channel="${1:-}"
case "$channel" in
    beta) exit 0 ;;
    prod) ;;
    *)
        echo "::error::unknown release channel '$channel'" >&2
        exit 1
        ;;
esac

# macOS: the Developer ID certificate and the notarization profile (Gatekeeper
# refuses a signed pkg that is not notarized). Windows: the cloud signing
# service ci/codemagic/windows-release.sh signs through; a different provider
# replaces these names in the same change that wires it.
missing=()
for name in WARREN_CSC_LINK_MACOS WARREN_CSC_KEY_PASSWORD_MACOS \
    WARREN_NOTARIZE_KEYCHAIN_PROFILE \
    AZURE_TENANT_ID AZURE_CLIENT_ID AZURE_CLIENT_SECRET \
    TRUSTED_SIGNING_ENDPOINT TRUSTED_SIGNING_ACCOUNT TRUSTED_SIGNING_PROFILE \
    ANDROID_KEYSTORE_BASE64 WARREN_UPDATE_SIGNING_KEY; do
    if [ -z "${!name:-}" ]; then
        missing+=("$name")
    fi
done

if [ "${#missing[@]}" -gt 0 ]; then
    echo "::error::a production release needs every signing identity; missing: ${missing[*]}" >&2
    exit 1
fi
echo "every production signing identity is present"
