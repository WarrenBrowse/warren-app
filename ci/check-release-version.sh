#!/usr/bin/env bash
# Refuse a release version that regresses, or that another series already used.
#
#   ci/check-release-version.sh <prod|beta> <version>
#
# A release version IS its tag: the builds bake it, so a stale-tagged operator
# clone ships a version REGRESSION. That happened on 2026-07-19, when `v1.9.1`
# was cut while `v1.11.0` was live, because `git tag -l | tail` sorts
# LEXICOGRAPHICALLY (`v1.10.0` before `v1.8.5`) and hid the real latest.
#
# A channel now has TWO tag series that both produce artifacts carrying that
# version: the app (`v*` / `beta-v*`) and the headless client
# (`daemon-v*` / `daemon-beta-v*`). They share one version space on purpose. If
# they did not, a headless release and an app release could each call themselves
# 1.1.15 while being built from different commits, and no one reading a version
# could tell which build they had.
#
# So a new tag must carry the highest version of its CHANNEL across both series,
# and no other series may already have used it.
#
# The two channels never mix: their series are disjoint by prefix, and their
# version lines are independent (beta may sit at 1.1.15 while prod is at 0.0.1).
set -euo pipefail

channel="${1:?usage: check-release-version.sh <prod|beta> <version>}"
version="${2:?missing version (e.g. 1.1.15)}"

case "$channel" in
    beta)
        app_prefix="beta-v"
        headless_prefix="daemon-beta-v"
        ;;
    prod)
        # `v[0-9]*` cannot match `beta-v…` or `daemon-…`: the prefixes keep the
        # series disjoint without any extra filtering.
        app_prefix="v"
        headless_prefix="daemon-v"
        ;;
    *)
        echo "::error::channel must be prod or beta, got: $channel" >&2
        exit 2
        ;;
esac

versions_of() { # versions_of <prefix>
    git tag -l "${1}[0-9]*" | sed "s|^${1}||"
}

all_versions="$(
    versions_of "$app_prefix"
    versions_of "$headless_prefix"
)"

if [ -z "$all_versions" ]; then
    echo "no existing $channel tag: $version opens the series"
    exit 0
fi

# The tag being released is already in the list (it was pushed to get here), so
# it accounts for one use of its own version. A second use is the other series
# having claimed it.
uses="$(printf '%s\n' "$all_versions" | grep -cFx "$version" || true)"
if [ "$uses" -gt 1 ]; then
    echo "::error::version $version is already used by the other $channel series;" \
        "the app and the headless client share one version space, so pick the next number"
    exit 1
fi

highest="$(printf '%s\n' "$all_versions" | sort -V | tail -1)"
if [ "$version" != "$highest" ]; then
    echo "::error::$version is not the highest $channel version ($highest, counting both the" \
        "app and the headless series): regression refused"
    exit 1
fi

echo "$version is the highest $channel version across both series"
