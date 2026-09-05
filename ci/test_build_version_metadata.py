#!/usr/bin/env python3
"""Unit tests for ci/build-version-metadata.py.

The generator decides the installer URLs that go INSIDE the signed update
manifests, so a mistake there is shipped under a signature and cannot be fixed
by editing a served file. These tests pin the two shapes that matter: the
GitHub release default, and the update-host mirror the private-repo channels
need (a GitHub release asset of a private repo answers 404 without a token, so
a manifest pointing there is undownloadable for every user).
"""

import argparse
import importlib.util
import tempfile
import unittest
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "build_version_metadata", Path(__file__).with_name("build-version-metadata.py")
)
bvm = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(bvm)


class AssetBaseUrl(unittest.TestCase):
    def test_defaults_to_the_github_release_download_path(self):
        self.assertEqual(
            bvm.asset_base_url("WarrenBrowse/warren-app", "beta-v0.0.8", None),
            "https://github.com/WarrenBrowse/warren-app/releases/download/beta-v0.0.8",
        )

    def test_override_replaces_the_github_host_entirely(self):
        self.assertEqual(
            bvm.asset_base_url(
                "WarrenBrowse/warren-app",
                "beta-v0.0.8",
                "https://api.beta.warrenbrowse.com/updates/desktop",
            ),
            "https://api.beta.warrenbrowse.com/updates/desktop",
        )

    def test_override_tolerates_a_trailing_slash(self):
        # The value comes from a workflow env var, where a stray slash is easy
        # to introduce and would produce a double slash in a signed URL.
        self.assertEqual(
            bvm.asset_base_url("o/r", "v1.0.0", "https://host/updates/desktop/"),
            "https://host/updates/desktop",
        )

    def test_empty_override_is_treated_as_absent(self):
        # Workflows pass ${VAR:-} style values; an unset secret arrives as "".
        self.assertEqual(
            bvm.asset_base_url("o/r", "v1.0.0", ""),
            "https://github.com/o/r/releases/download/v1.0.0",
        )


class AssetUrl(unittest.TestCase):
    def test_joins_the_base_and_the_filename(self):
        self.assertEqual(
            bvm.asset_url("https://host/updates/desktop", "WarrenVPN-Beta-0.0.8-android.apk"),
            "https://host/updates/desktop/WarrenVPN-Beta-0.0.8-android.apk",
        )


class ChangelogTranslations(unittest.TestCase):
    """The manifest carries the release notes per language, so a client running
    in French shows French notes. English stays in `changelog` as the fallback.
    """

    def _write_changelogs(self, directory: Path) -> Path:
        english = directory / "CHANGELOG.md"
        english.write_text(
            "## [1.2.0] - 2026-08-05\n### Added\n- An English entry.\n\n## [1.1.0]\n- Older.\n",
            encoding="utf-8",
        )
        (directory / "CHANGELOG.fr.md").write_text(
            "## [1.2.0] - 2026-08-05\n### Ajoute\n- Une entree en francais.\n",
            encoding="utf-8",
        )
        # Translated only for an older version: this release has no Romanian
        # section, so Romanian clients must fall back to English.
        (directory / "CHANGELOG.ro.md").write_text(
            "## [1.1.0]\n### Adaugat\n- O intrare veche.\n",
            encoding="utf-8",
        )
        return english

    def test_collects_the_section_of_each_translated_changelog(self):
        with tempfile.TemporaryDirectory() as tmp:
            english = self._write_changelogs(Path(tmp))

            translations = bvm.extract_changelog_translations(english, "1.2.0")

        self.assertEqual(list(translations), ["fr"])
        self.assertIn("Une entree en francais.", translations["fr"])

    def test_omits_a_language_that_has_no_section_for_this_version(self):
        with tempfile.TemporaryDirectory() as tmp:
            english = self._write_changelogs(Path(tmp))

            translations = bvm.extract_changelog_translations(english, "1.2.0")

        self.assertNotIn("ro", translations)

    def test_release_omits_the_field_when_nothing_is_translated(self):
        # The signature covers the canonical JSON, so an always-present empty
        # map would change the bytes of every manifest for nothing.
        release = bvm.build_release("1.2.0", "notes", {}, [])

        self.assertNotIn("changelog_translations", release)

    def test_release_carries_the_translations_when_there_are_any(self):
        release = bvm.build_release("1.2.0", "notes", {"fr": "notes en francais"}, [])

        self.assertEqual(release["changelog_translations"], {"fr": "notes en francais"})


class DropVersions(unittest.TestCase):
    """Withdrawing a release means erasing its entry from the manifest.

    Clients resolve the highest version listed, so publishing an older release
    on top never supersedes a bad one, and the merge that preserves history
    would otherwise carry it forward for ever.
    """

    def test_removes_the_withdrawn_entry_from_the_kept_history(self):
        previous = [{"version": "1.1.5"}, {"version": "1.1.4"}, {"version": "1.1.3"}]

        kept = bvm.drop_versions(previous, {"1.1.5"})

        self.assertEqual([r["version"] for r in kept], ["1.1.4", "1.1.3"])

    def test_keeps_everything_when_nothing_is_withdrawn(self):
        previous = [{"version": "1.1.5"}, {"version": "1.1.4"}]

        self.assertEqual(bvm.drop_versions(previous, set()), previous)

    def test_republishing_an_older_release_over_a_withdrawn_one_leaves_it_newest(self):
        previous = [{"version": "1.1.5"}, {"version": "1.1.4"}, {"version": "1.1.3"}]

        merged = bvm.merge_release(
            bvm.drop_versions(previous, {"1.1.5"}),
            bvm.build_release("1.1.4", "notes", {}, []),
        )

        self.assertEqual([r["version"] for r in merged], ["1.1.4", "1.1.3"])

    def test_download_page_stops_advertising_the_withdrawn_version(self):
        # A platform absent from the republished release keeps its previous
        # entry, which would go on offering the withdrawn installers.
        platforms = {
            "macos": {"version": "1.1.5", "assets": []},
            "linux": {"version": "1.1.4", "assets": []},
        }

        kept = bvm.drop_downloads_versions(platforms, {"1.1.5"})

        self.assertEqual(list(kept), ["linux"])


class SiteAssetsWithTorrents(unittest.TestCase):
    """The download page's BitTorrent half is generated here.

    A magnet link is only useful if it names the same bytes the direct link
    serves, so the torrent is built from the installer this pass just hashed,
    never from a separate listing that could drift.
    """

    BASE = "https://api.beta.warrenbrowse.com/updates/desktop"

    def classify(self, tmp: str, payload: bytes, name: str, **kwargs) -> dict:
        release_dir = Path(tmp) / "release-files"
        release_dir.mkdir()
        (release_dir / name).write_bytes(payload)
        return bvm.classify_site_assets(
            release_dir,
            "1.2.3",
            self.BASE,
            "WarrenVPN",
            "beta",
            torrent_dir=release_dir,
            min_torrent_bytes=kwargs.pop("min_torrent_bytes", 16),
            creation_date=0,
        )

    def test_an_installer_gains_a_torrent_and_a_magnet(self):
        with tempfile.TemporaryDirectory() as tmp:
            platforms = self.classify(
                tmp, b"x" * 1024, "WarrenVPN-1.2.3-linux-amd64.deb"
            )

        asset = platforms["linux"][0]
        self.assertEqual(
            asset["torrent"], f"{self.BASE}/WarrenVPN-1.2.3-linux-amd64.deb.torrent"
        )
        self.assertTrue(asset["magnet"].startswith("magnet:?xt=urn:btih:"))

    def test_the_torrent_web_seeds_the_direct_download(self):
        # Without it the swarm dies the moment the last peer leaves, and the
        # magnet on the download page becomes a link to nothing.
        with tempfile.TemporaryDirectory() as tmp:
            release_dir = Path(tmp) / "release-files"
            self.classify(tmp, b"x" * 1024, "WarrenVPN-1.2.3-linux-amd64.deb")
            written = (release_dir / "WarrenVPN-1.2.3-linux-amd64.deb.torrent").read_bytes()

        self.assertIn(
            f"{self.BASE}/WarrenVPN-1.2.3-linux-amd64.deb".encode(), written
        )

    def test_a_file_below_the_threshold_gets_no_torrent(self):
        # The NixOS flake tarball weighs a few kB: a swarm for it costs more
        # than it saves, and an empty one on the page reads as broken.
        with tempfile.TemporaryDirectory() as tmp:
            platforms = self.classify(
                tmp,
                b"x" * 8,
                "WarrenVPN-1.2.3-linux-x86_64-nixos.tar.gz",
                min_torrent_bytes=1024,
            )

        asset = platforms["linux"][0]
        self.assertNotIn("torrent", asset)
        self.assertNotIn("magnet", asset)

    def test_a_generated_torrent_is_not_itself_a_download_row(self):
        # The torrents are written beside the installers so the release job
        # uploads them with no extra machinery; classification must not then
        # offer them to visitors as a package to install.
        with tempfile.TemporaryDirectory() as tmp:
            platforms = self.classify(
                tmp, b"x" * 1024, "WarrenVPN-1.2.3-linux-amd64.deb"
            )

        self.assertEqual([a["format"] for a in platforms["linux"]], ["deb"])

    def test_torrents_can_be_turned_off_entirely(self):
        with tempfile.TemporaryDirectory() as tmp:
            release_dir = Path(tmp) / "release-files"
            release_dir.mkdir()
            (release_dir / "WarrenVPN-1.2.3-linux-amd64.deb").write_bytes(b"x" * 1024)

            platforms = bvm.classify_site_assets(
                release_dir, "1.2.3", self.BASE, "WarrenVPN", "beta"
            )

        self.assertNotIn("magnet", platforms["linux"][0])
        self.assertEqual(list(release_dir.glob("*.torrent")), [])


class DownloadsBitTorrentBlock(unittest.TestCase):
    def test_names_the_trackers_the_torrents_announce_on(self):
        # The download page tells visitors which tracker their client will talk
        # to; reading it from the manifest keeps that claim true after a
        # tracker change, instead of hardcoding a name in three languages.
        args = argparse.Namespace(
            version="1.2.3",
            now="2026-09-05T00:00:00Z",
            metadata_base_url="https://example.invalid/updates/desktop",
            dropped_versions=set(),
        )

        downloads = bvm.build_downloads(args, {})

        self.assertEqual(
            downloads["bittorrent"]["trackers"][0],
            "udp://tracker.opentrackr.org:1337/announce",
        )


if __name__ == "__main__":
    unittest.main()
