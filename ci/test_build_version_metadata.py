#!/usr/bin/env python3
"""Unit tests for ci/build-version-metadata.py.

The generator decides the installer URLs that go INSIDE the signed update
manifests, so a mistake there is shipped under a signature and cannot be fixed
by editing a served file. These tests pin the two shapes that matter: the
GitHub release default, and the update-host mirror the private-repo channels
need (a GitHub release asset of a private repo answers 404 without a token, so
a manifest pointing there is undownloadable for every user).
"""

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


if __name__ == "__main__":
    unittest.main()
