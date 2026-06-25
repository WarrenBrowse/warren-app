#!/usr/bin/env python3
"""Generate unsigned Warren update metadata (one Response JSON per platform).

Reads the installers built by the release pipeline, computes their size and
SHA-256, and emits `macos.json` / `windows.json` / `linux.json` in the exact
`mullvad_update` Response schema, ready to be signed by
`mullvad-version-metadata sign`.

In-app installer per platform (harmonized release-asset names, see
ci/stage-release-assets.sh):
  - macOS: the universal `-macos-universal.pkg` (one file, listed for both
    x86 and arm64).
  - Windows: the per-arch `-windows-x64.exe` -> x86, `-windows-arm64.exe` ->
    arm64 (the `-windows-universal.exe` downloader is not listed per-arch).
  - Linux: none. The release is listed without installers (the daemon's
    `allow_empty` path); the GUI sends Linux users to the download page.

Anti-rollback: `metadata_version` is the previously published value + 1. The
previous signed manifest is fetched from the metadata base URL; if it is
unreachable or absent we start at 1 with an empty release history. Older
releases are preserved so existing clients still find their version listed
and `suggested_upgrade` keeps working.

stdlib only (runs on a stock python3); no third-party dependencies.
"""

# PEP 604 unions (`Path | None`) appear in annotations below; the CI runner
# ships Python 3.9, which evaluates annotations eagerly. Defer them to keep the
# script runnable on 3.9+ without rewriting to typing.Optional.
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

# Filename architecture token -> metadata architecture enum value.
WIN_ARCH_TOKENS = {"x64": "x86", "arm64": "arm64"}


def sha256_and_size(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


def asset_url(repo: str, tag: str, filename: str) -> str:
    return f"https://github.com/{repo}/releases/download/{tag}/{filename}"


def extract_changelog(changelog_path: Path, version: str) -> str:
    """Pull the section for `version` out of a keep-a-changelog file.

    Matches the first `## ...` header line that mentions the version and
    captures everything up to the next `## ` header. Returns an empty string
    if no matching section is found (still valid metadata).
    """
    if not changelog_path.is_file():
        return ""
    lines = changelog_path.read_text(encoding="utf-8").splitlines()
    capturing = False
    captured: list[str] = []
    for line in lines:
        if line.startswith("## "):
            if capturing:
                break
            capturing = version in line
            continue
        if capturing:
            captured.append(line)
    return "\n".join(captured).strip()


def fetch_previous(metadata_base_url: str, platform: str) -> tuple[int, list]:
    """Return (previous metadata_version, previous releases) for a platform.

    On any network/parse error we conservatively start fresh: version 0 (so the
    new manifest becomes 1) and no release history.
    """
    url = f"{metadata_base_url.rstrip('/')}/{platform}.json"
    try:
        with urllib.request.urlopen(url, timeout=15) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except (urllib.error.URLError, ValueError, TimeoutError) as error:
        print(f"  no usable previous {platform}.json ({error}); starting fresh", file=sys.stderr)
        return 0, []
    signed = payload.get("signed", {})
    return int(signed.get("metadata_version", 0)), list(signed.get("releases", []))


def find_one(release_dir: Path, pattern: str) -> Path | None:
    matches = sorted(release_dir.glob(pattern))
    if len(matches) > 1:
        raise SystemExit(f"ambiguous installer match for {pattern!r}: {matches}")
    return matches[0] if matches else None


def installer_entry(path: Path, architecture: str, repo: str, tag: str) -> dict:
    sha256, size = sha256_and_size(path)
    return {
        "architecture": architecture,
        "urls": [asset_url(repo, tag, path.name)],
        "size": size,
        "sha256": sha256,
    }


def macos_installers(release_dir: Path, version: str, repo: str, tag: str) -> list:
    pkg = find_one(release_dir, f"WarrenVPN-{version}-macos-universal.pkg")
    if pkg is None:
        return []
    # The macOS .pkg is universal: list it for both architectures so a client
    # of either arch resolves an installer.
    return [
        installer_entry(pkg, "x86", repo, tag),
        installer_entry(pkg, "arm64", repo, tag),
    ]


def windows_installers(release_dir: Path, version: str, repo: str, tag: str) -> list:
    installers = []
    for path in sorted(release_dir.glob(f"WarrenVPN-{version}-windows-*.exe")):
        match = re.search(rf"WarrenVPN-{re.escape(version)}-windows-(\w+)\.exe$", path.name)
        if not match:
            continue
        token = match.group(1)
        # The universal installer-downloader is not a per-arch payload; the
        # metadata lists the concrete x64/arm64 installers only.
        if token == "universal":
            continue
        architecture = WIN_ARCH_TOKENS.get(token)
        if architecture is None:
            print(f"  skipping unknown Windows arch token in {path.name}", file=sys.stderr)
            continue
        installers.append(installer_entry(path, architecture, repo, tag))
    return installers


def build_release(version: str, changelog: str, installers: list) -> dict:
    return {"version": version, "changelog": changelog, "installers": installers}


def merge_release(previous: list, new_release: dict) -> list:
    """Drop any existing entry for this version, then prepend the new one."""
    kept = [r for r in previous if r.get("version") != new_release["version"]]
    return [new_release] + kept


def build_platform(platform: str, installers: list, args) -> dict:
    prev_version, prev_releases = fetch_previous(args.metadata_base_url, platform)
    changelog = extract_changelog(Path(args.changelog), args.version)
    releases = merge_release(prev_releases, build_release(args.version, changelog, installers))

    response = {
        "metadata_version": prev_version + 1,
        "metadata_expiry": args.expiry,
        "releases": releases,
    }
    if args.min_version:
        response["minimum_supported_version"] = args.min_version
    return response


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True, help="Release tag, e.g. v1.2.0")
    parser.add_argument("--version", required=True, help="Version without leading v, e.g. 1.2.0")
    parser.add_argument("--release-dir", required=True, help="Directory holding the installers")
    parser.add_argument("--repo", required=True, help="owner/name for asset URLs")
    parser.add_argument("--changelog", required=True, help="Path to CHANGELOG.md")
    parser.add_argument("--metadata-base-url", required=True, help="Where the signed manifests are served")
    parser.add_argument("--out-dir", required=True, help="Output directory for unsigned JSON")
    parser.add_argument("--now", required=True, help="Current time, RFC3339 (e.g. 2026-06-14T00:00:00Z)")
    parser.add_argument("--expiry-months", type=int, default=6)
    parser.add_argument("--min-version", default="", help="minimum_supported_version (optional, forces update below it)")
    args = parser.parse_args()

    now = datetime.datetime.strptime(args.now, "%Y-%m-%dT%H:%M:%SZ")
    expiry = now + datetime.timedelta(days=30 * args.expiry_months)
    args.expiry = expiry.strftime("%Y-%m-%dT%H:%M:%SZ")

    release_dir = Path(args.release_dir)
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    platforms = {
        "macos": macos_installers(release_dir, args.version, args.repo, args.tag),
        "windows": windows_installers(release_dir, args.version, args.repo, args.tag),
        # Linux: installer-less release (the daemon's allow_empty path).
        "linux": [],
        # Mobile: store-installed, so the OS store performs the actual update.
        # The manifest only carries the latest version + minimum_supported_version
        # so the app can show "update available" / hard-block and deep-link to the
        # store. No installer to self-download here (a direct-APK installer entry
        # could be added later for sideloaded Android builds).
        "android": [],
        "ios": [],
    }

    for platform, installers in platforms.items():
        response = build_platform(platform, installers, args)
        out_path = out_dir / f"{platform}.json"
        out_path.write_text(json.dumps(response, indent=2) + "\n", encoding="utf-8")
        print(f"wrote {out_path} (metadata_version={response['metadata_version']}, "
              f"installers={len(installers)}, releases={len(response['releases'])})")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
