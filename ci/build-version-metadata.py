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

Also emits `downloads.json`, the website-facing manifest consumed by the
warren.ro download page: latest downloadable version per platform with EVERY
user-facing asset (deb/rpm/pacman included), independent from the app-updater
installer contract above.

Anti-rollback: `metadata_version` is the previously published value + 1. The
previous signed manifest is fetched from the metadata base URL, so each release
channel continues its OWN history (a beta run must be given the beta base URL,
never the prod one). If it is unreachable or absent we start at 1 with an empty
release history. Older releases are preserved so existing clients still find
their version listed and `suggested_upgrade` keeps working.

Restarting a channel at a LOWER version (the beta line restarting at 0.0.1)
therefore requires clearing that channel's published manifests first: merged
into a preserved history, the older higher version stays the newest entry and
clients never resolve the reset release. Withdrawing a single bad release from a
live channel is `--drop-version`: republish the release below it with the bad
version dropped, and the bumped `metadata_version` carries the removal past the
client anti-rollback floor.

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

# Filename architecture token -> website architecture label (downloads.json).
SITE_ARCH_TOKENS = {
    "amd64": "x64",
    "x86_64": "x64",
    "x64": "x64",
    "arm64": "arm64",
    "aarch64": "arm64",
    "universal": "universal",
}

SITE_PLATFORMS = {"macos", "windows", "linux", "android", "ios"}

# Extensions that are two dot-separated parts. The NixOS flake ships as a
# tarball, and `rpartition(".")` alone would label it "gz".
COMPOUND_EXTENSIONS = ("tar.gz", "tar.xz", "tar.zst")

# Store/CI artifacts that are not a user-facing download, per channel. The AAB
# is a Play Store upload format, never a download. The APK is the beta's ONLY
# Android distribution (no store listing for the beta app id), so it belongs on
# the beta download page; on prod it stays hidden until the store listing is
# live and takes over.
SITE_SKIP_FORMATS = {
    "prod": {"aab", "apk"},
    "beta": {"aab"},
}

# iOS versions are calendar-based (YYYY.N marketing version), unlike the
# desktop/Android 1.x tag scheme (Android bakes the desktop tag as its
# versionName, so android.json legitimately shares the desktop history).
IOS_CALENDAR_VERSION = re.compile(r"^\d{4}\.")

# Single source of truth for the iOS app version, relative to the repo root.
IOS_VERSION_XCCONFIG = Path(__file__).resolve().parent.parent / "ios/Configurations/Version.xcconfig"


def ios_marketing_version() -> str:
    """Read MARKETING_VERSION from the iOS project's Version.xcconfig."""
    try:
        text = IOS_VERSION_XCCONFIG.read_text(encoding="utf-8")
    except OSError as error:
        raise SystemExit(f"cannot read {IOS_VERSION_XCCONFIG}: {error}")
    match = re.search(r"^MARKETING_VERSION\s*=\s*(\S+)", text, re.MULTILINE)
    if not match:
        raise SystemExit(f"MARKETING_VERSION not found in {IOS_VERSION_XCCONFIG}")
    return match.group(1)


def sha256_and_size(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


def asset_base_url(repo: str, tag: str, override: str | None) -> str:
    """Where installers are downloaded from.

    Defaults to the GitHub release, which only works while the repo is public:
    an asset of a PRIVATE repo answers 404 without a token, so every user of a
    manifest pointing there is stuck, first install and in-app update alike.
    `override` points at the update host, which serves the mirrored installers
    next to the manifests over plain TLS.
    """
    if override:
        return override.rstrip("/")
    return f"https://github.com/{repo}/releases/download/{tag}"


def asset_url(asset_base: str, filename: str) -> str:
    return f"{asset_base}/{filename}"


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


TRANSLATED_CHANGELOG = re.compile(r"^CHANGELOG\.([a-z]{2}(?:-[A-Za-z]{2})?)\.md$")


def extract_changelog_translations(changelog_path: Path, version: str) -> dict[str, str]:
    """Collect the section for `version` from every `CHANGELOG.<lang>.md` sibling.

    A language whose file carries no section for this version is left out, so
    the client falls back to the English notes rather than showing none. The
    language tag comes from the filename, which is what the app matches its
    locale against.
    """
    translations: dict[str, str] = {}
    for sibling in sorted(changelog_path.parent.glob("CHANGELOG.*.md")):
        match = TRANSLATED_CHANGELOG.match(sibling.name)
        if not match:
            continue
        section = extract_changelog(sibling, version)
        if section:
            translations[match.group(1)] = section
    return translations


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


def installer_entry(path: Path, architecture: str, asset_base: str) -> dict:
    sha256, size = sha256_and_size(path)
    return {
        "architecture": architecture,
        "urls": [asset_url(asset_base, path.name)],
        "size": size,
        "sha256": sha256,
    }


def macos_installers(release_dir: Path, version: str, asset_base: str,
                     artifact_prefix: str = "WarrenVPN") -> list:
    pkg = find_one(release_dir, f"{artifact_prefix}-{version}-macos-universal.pkg")
    if pkg is None:
        return []
    # The macOS .pkg is universal: list it for both architectures so a client
    # of either arch resolves an installer.
    return [
        installer_entry(pkg, "x86", asset_base),
        installer_entry(pkg, "arm64", asset_base),
    ]


def windows_installers(release_dir: Path, version: str, asset_base: str,
                       artifact_prefix: str = "WarrenVPN") -> list:
    installers = []
    for path in sorted(release_dir.glob(f"{artifact_prefix}-{version}-windows-*.exe")):
        match = re.search(
            rf"{re.escape(artifact_prefix)}-{re.escape(version)}-windows-(\w+)\.exe$", path.name)
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
        installers.append(installer_entry(path, architecture, asset_base))
    return installers


def split_asset_name(rest: str) -> tuple[str, str, str, str] | None:
    """Split `<platform>-<arch>[-<flavor>].<ext>` into its four parts.

    The flavor is what distinguishes two downloads that share a platform, an
    architecture AND an extension: Linux ships one .deb per init system, so
    without it the download page would show two rows both reading ".deb".

    Returns None when the name carries no extension at all.

    >>> split_asset_name("linux-amd64.deb")
    ('linux', 'amd64', '', 'deb')
    >>> split_asset_name("linux-amd64-sysvinit.deb")
    ('linux', 'amd64', 'sysvinit', 'deb')
    >>> split_asset_name("linux-x86_64-nixos.tar.gz")
    ('linux', 'x86_64', 'nixos', 'tar.gz')
    >>> split_asset_name("macos-universal.pkg")
    ('macos', 'universal', '', 'pkg')
    >>> split_asset_name("android")
    >>> split_asset_name("android.apk")
    ('android', 'universal', '', 'apk')
    """
    lowered = rest.lower()
    for compound in COMPOUND_EXTENSIONS:
        if lowered.endswith(f".{compound}"):
            return (*_split_stem(rest[: -len(compound) - 1]), compound)
    stem, dot, ext = rest.rpartition(".")
    if not dot:
        return None
    return (*_split_stem(stem), ext.lower())


def _split_stem(stem: str) -> tuple[str, str, str]:
    """`<platform>[-<arch>[-<flavor>]]` -> the three tokens, arch defaulted."""
    tokens = stem.split("-")
    platform = tokens[0]
    arch_token = tokens[1] if len(tokens) > 1 else "universal"
    flavor = "-".join(tokens[2:])
    return platform, arch_token, flavor


def classify_site_assets(release_dir: Path, version: str, asset_base: str,
                         artifact_prefix: str = "WarrenVPN",
                         channel: str = "prod") -> dict:
    """Group every user-facing installer of this release by platform.

    Unlike the app manifests (whose installer lists are an app-updater
    contract: one installer per architecture, none on Linux), downloads.json
    lists everything a human can download on this channel, all formats included.
    """
    platforms: dict[str, list] = {}
    skip_formats = SITE_SKIP_FORMATS[channel]
    prefix = f"{artifact_prefix}-{version}-"
    for path in sorted(release_dir.glob(f"{prefix}*")):
        parts = split_asset_name(path.name[len(prefix):])
        if parts is None:
            continue
        platform, arch_token, flavor, fmt = parts
        if fmt in skip_formats:
            continue
        if platform not in SITE_PLATFORMS:
            print(f"  skipping unrecognized asset {path.name}", file=sys.stderr)
            continue
        architecture = SITE_ARCH_TOKENS.get(arch_token, arch_token)
        sha256, size = sha256_and_size(path)
        asset = {
            "filename": path.name,
            "url": asset_url(asset_base, path.name),
            "size": size,
            "sha256": sha256,
            "architecture": architecture,
            "format": fmt,
        }
        if flavor:
            asset["flavor"] = flavor
        platforms.setdefault(platform, []).append(asset)
    return platforms


def fetch_previous_downloads(metadata_base_url: str) -> dict:
    """Previous downloads.json platform map, or {} when absent/unreadable."""
    url = f"{metadata_base_url.rstrip('/')}/downloads.json"
    try:
        with urllib.request.urlopen(url, timeout=15) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except (urllib.error.URLError, ValueError, TimeoutError) as error:
        print(f"  no usable previous downloads.json ({error}); starting fresh", file=sys.stderr)
        return {}
    return dict(payload.get("platforms", {}))


def build_downloads(args, classified: dict) -> dict:
    """Website manifest: latest downloadable release per platform.

    Platforms absent from this release keep their previous entry, so a
    release that ships only some platforms never blanks the others on the
    download page.
    """
    platforms = drop_downloads_versions(
        fetch_previous_downloads(args.metadata_base_url), args.dropped_versions)
    for platform, assets in classified.items():
        platforms[platform] = {"version": args.version, "assets": assets}
    return {"updated_at": args.now, "platforms": platforms}


def build_release(
    version: str, changelog: str, changelog_translations: dict[str, str], installers: list
) -> dict:
    release = {"version": version, "changelog": changelog}
    # Omitted when empty: the signature covers the canonical JSON of the whole
    # `signed` object, so an always-present empty map would change the bytes of
    # every manifest that has no translations.
    if changelog_translations:
        release["changelog_translations"] = changelog_translations
    release["installers"] = installers
    return release


def merge_release(previous: list, new_release: dict) -> list:
    """Drop any existing entry for this version, then prepend the new one."""
    kept = [r for r in previous if r.get("version") != new_release["version"]]
    return [new_release] + kept


def drop_versions(releases: list, dropped: set) -> list:
    """Remove the entries of releases withdrawn from the channel.

    A withdrawn release has to leave the list: clients resolve the highest
    version listed, so republishing an older one on top never supersedes it,
    and the history-preserving merge would carry it forward for ever.
    """
    if not dropped:
        return list(releases)
    return [r for r in releases if str(r.get("version")) not in dropped]


def drop_downloads_versions(platforms: dict, dropped: set) -> dict:
    """Same withdrawal, applied to the website manifest's per-platform map.

    A platform absent from the republished release keeps its previous entry,
    which would go on offering the withdrawn installers on the download page.
    """
    if not dropped:
        return dict(platforms)
    return {p: v for p, v in platforms.items() if str(v.get("version")) not in dropped}


def build_platform(platform: str, installers: list, args, version: str, min_version: str) -> dict:
    prev_version, prev_releases = fetch_previous(args.metadata_base_url, platform)
    prev_releases = drop_versions(prev_releases, args.dropped_versions)
    if platform == "ios":
        # Self-heal: early ios.json manifests carried the desktop 1.x release
        # history (the script listed the tag version for every platform). Only
        # calendar-style iOS versions belong here; drop the rest so the wrong
        # entries do not survive through the merge below forever.
        prev_releases = [
            r for r in prev_releases if IOS_CALENDAR_VERSION.match(str(r.get("version", "")))
        ]
    changelog = extract_changelog(Path(args.changelog), version)
    changelog_translations = extract_changelog_translations(Path(args.changelog), version)
    releases = merge_release(
        prev_releases, build_release(version, changelog, changelog_translations, installers)
    )

    response = {
        "metadata_version": prev_version + 1,
        "metadata_expiry": args.expiry,
        "releases": releases,
    }
    if min_version:
        response["minimum_supported_version"] = min_version
    return response


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True, help="Release tag, e.g. v1.2.0")
    parser.add_argument("--version", required=True, help="Version without leading v, e.g. 1.2.0")
    parser.add_argument("--release-dir", required=True, help="Directory holding the installers")
    parser.add_argument("--repo", required=True, help="owner/name for GitHub-release asset URLs")
    parser.add_argument("--asset-base-url", default="",
                        help="Serve installers from this base instead of the GitHub release "
                             "(required while the repo is private, see asset_base_url)")
    parser.add_argument("--changelog", required=True, help="Path to CHANGELOG.md")
    parser.add_argument("--metadata-base-url", required=True, help="Where the signed manifests are served")
    parser.add_argument("--artifact-prefix", default="WarrenVPN",
                        help="Release-asset name prefix (WarrenVPN-Beta for the beta channel)")
    parser.add_argument("--channel", choices=sorted(SITE_SKIP_FORMATS), default="prod",
                        help="Release channel: decides which formats reach the download page "
                             "(the beta ships Android as a direct APK, prod does not)")
    parser.add_argument("--drop-version", action="append", default=[], metavar="VERSION",
                        help="Withdraw this version from the channel: its entry is removed from "
                             "the preserved history instead of being carried forward. Repeatable")
    parser.add_argument("--out-dir", required=True, help="Output directory for unsigned JSON")
    parser.add_argument("--now", required=True, help="Current time, RFC3339 (e.g. 2026-06-14T00:00:00Z)")
    parser.add_argument("--expiry-months", type=int, default=6)
    parser.add_argument("--min-version", default="", help="minimum_supported_version (optional, forces update below it)")
    parser.add_argument("--ios-version", default="",
                        help="iOS app version for ios.json (default: MARKETING_VERSION from the iOS project)")
    parser.add_argument("--ios-min-version", default="",
                        help="minimum_supported_version for ios.json. When empty, the field is "
                             "omitted and clients fall back to 'version listed in releases', "
                             "which blocks builds NEWER than the manifest; set it in practice.")
    args = parser.parse_args()

    args.dropped_versions = {v.strip() for v in args.drop_version if v.strip()}
    if args.version in args.dropped_versions:
        raise SystemExit(f"--drop-version {args.version} withdraws the release being published")
    if args.dropped_versions:
        print(f"withdrawing from the channel: {', '.join(sorted(args.dropped_versions))}")

    ios_version = args.ios_version or ios_marketing_version()
    if not IOS_CALENDAR_VERSION.match(ios_version):
        raise SystemExit(f"iOS version {ios_version!r} is not calendar-style (YYYY.N)")

    now = datetime.datetime.strptime(args.now, "%Y-%m-%dT%H:%M:%SZ")
    expiry = now + datetime.timedelta(days=30 * args.expiry_months)
    args.expiry = expiry.strftime("%Y-%m-%dT%H:%M:%SZ")

    release_dir = Path(args.release_dir)
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    asset_base = asset_base_url(args.repo, args.tag, args.asset_base_url)
    print(f"installer URLs resolve under {asset_base}")

    platforms = {
        "macos": macos_installers(release_dir, args.version, asset_base,
                                  args.artifact_prefix),
        "windows": windows_installers(release_dir, args.version, asset_base,
                                      args.artifact_prefix),
        # Linux: installer-less release (the daemon's allow_empty path).
        "linux": [],
        # Mobile: store-installed, so the OS store performs the actual update.
        # The manifest only carries the latest version + minimum_supported_version
        # so the app can show "update available" / hard-block and deep-link to the
        # store. No installer to self-download here (a direct-APK installer entry
        # could be added later for sideloaded Android builds).
        # Android bakes the desktop release tag as its versionName, so it shares
        # the desktop version/minimum; iOS uses its own calendar versioning.
        "android": [],
        "ios": [],
    }

    for platform, installers in platforms.items():
        if platform == "ios":
            version, min_version = ios_version, args.ios_min_version
        else:
            version, min_version = args.version, args.min_version
        response = build_platform(platform, installers, args, version, min_version)
        out_path = out_dir / f"{platform}.json"
        out_path.write_text(json.dumps(response, indent=2) + "\n", encoding="utf-8")
        print(f"wrote {out_path} (metadata_version={response['metadata_version']}, "
              f"installers={len(installers)}, releases={len(response['releases'])})")

    # Website manifest (warren.ro download page): every format, all platforms.
    # Served over TLS only; not part of the signed app-update contract.
    downloads = build_downloads(
        args,
        classify_site_assets(release_dir, args.version, asset_base,
                             args.artifact_prefix, args.channel))
    downloads_path = out_dir / "downloads.json"
    downloads_path.write_text(json.dumps(downloads, indent=2) + "\n", encoding="utf-8")
    counts = {p: len(v.get("assets", [])) for p, v in downloads["platforms"].items()}
    print(f"wrote {downloads_path} (assets per platform: {counts})")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
