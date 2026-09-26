#!/usr/bin/env python3
"""Unit tests for codemagic_build.py, the GitHub-side half of every Windows job.

The proxy decides three things a release depends on: which commit the Windows
machine builds, whether the job is green, and which files reach the release.
A wrong ref builds someone else's commit under this tag, a misread status turns
a failed build green, and a sloppy artifact merge publishes a file nobody
checksummed. Only the pure halves are exercised: the HTTP round trip belongs to
the live job, not to an offline suite.
"""

import hashlib
import importlib.util
import tempfile
import unittest
import zipfile
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "codemagic_build", Path(__file__).with_name("codemagic_build.py")
)
cb = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(cb)


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class ParseInputs(unittest.TestCase):
    def test_reads_one_key_value_pair_per_line_and_skips_blank_lines(self):
        self.assertEqual(
            cb.parse_inputs("channel=beta\n\n  version=1.2.3  \n"),
            {"channel": "beta", "version": "1.2.3"},
        )

    def test_keeps_an_equals_sign_inside_the_value(self):
        self.assertEqual(cb.parse_inputs("url=https://x/?a=b"), {"url": "https://x/?a=b"})

    def test_refuses_a_line_without_a_value(self):
        with self.assertRaises(cb.ProxyError):
            cb.parse_inputs("channel")

    def test_refuses_a_key_codemagic_would_reject(self):
        # Codemagic input ids match ^[a-zA-Z]\w*$; a dash would be refused at
        # start time with a message that names nothing on our side.
        with self.assertRaises(cb.ProxyError):
            cb.parse_inputs("api-url=https://x")

    def test_refuses_a_caller_that_sets_the_commit_itself(self):
        # The commit is always the job's own; letting a caller override it
        # is how a Windows artifact gets built from a different tree.
        with self.assertRaises(cb.ProxyError):
            cb.parse_inputs("sha=deadbeef")


class CodemagicRef(unittest.TestCase):
    def test_a_tag_run_builds_the_tag(self):
        self.assertEqual(cb.codemagic_ref("tag", "beta-v1.2.3", ""), ("tag", "beta-v1.2.3"))

    def test_a_branch_run_builds_the_branch(self):
        self.assertEqual(cb.codemagic_ref("branch", "main", ""), ("branch", "main"))

    def test_a_pull_request_clones_its_source_branch(self):
        # GITHUB_REF_NAME is "12/merge" on a pull request, which is no branch
        # Codemagic can clone.
        self.assertEqual(cb.codemagic_ref("branch", "12/merge", "feat/x"), ("branch", "feat/x"))


class StartPayload(unittest.TestCase):
    def test_pins_the_commit_and_forwards_the_inputs(self):
        payload = cb.start_payload(
            "windows-release", ("tag", "v1.0.0"), "a" * 40, {"channel": "prod"}, ["run 7"]
        )
        self.assertEqual(
            payload,
            {
                "workflow_id": "windows-release",
                "tag": "v1.0.0",
                "inputs": {"channel": "prod", "sha": "a" * 40},
                "labels": ["run 7"],
            },
        )

    def test_refuses_a_commit_that_is_not_a_full_sha(self):
        with self.assertRaises(cb.ProxyError):
            cb.start_payload("w", ("branch", "main"), "abc123", {}, [])


class BuildStatus(unittest.TestCase):
    def test_only_finished_is_a_success(self):
        self.assertTrue(cb.succeeded("finished"))
        for status in ("failed", "canceled", "timeout", "skipped"):
            self.assertFalse(cb.succeeded(status), status)

    def test_every_end_state_stops_the_wait(self):
        for status in ("finished", "failed", "canceled", "timeout", "skipped"):
            self.assertTrue(cb.is_terminal(status), status)
        for status in ("queued", "preparing", "fetching", "building", "publishing"):
            self.assertFalse(cb.is_terminal(status), status)


class FinishedSteps(unittest.TestCase):
    BUILD = {
        "buildActions": [
            {"_id": "p", "name": "Preparing build machine", "status": "success",
             "logUrl": "https://l/p", "subactions": []},
            {"_id": "s", "name": "Build", "status": "failed", "logUrl": None,
             "subactions": [{"status": "failed", "logUrl": "https://l/s"}]},
            {"_id": "r", "name": "Package", "status": None, "logUrl": None,
             "subactions": [{"status": None, "logUrl": None}]},
        ]
    }

    def test_reports_each_ended_step_once_with_the_log_its_subaction_carries(self):
        seen = set()
        first = cb.finished_steps(self.BUILD, seen)
        self.assertEqual(
            first,
            [("Preparing build machine", "success", "https://l/p"),
             ("Build", "failed", "https://l/s")],
        )
        self.assertEqual(cb.finished_steps(self.BUILD, seen), [])


class RunningStep(unittest.TestCase):
    def test_names_the_step_that_started_and_has_not_ended(self):
        build = {"buildActions": [
            {"name": "Prepare", "status": "success", "startedAt": "t0"},
            {"name": "Build", "status": None, "startedAt": "t1"},
            {"name": "Stage", "status": None, "startedAt": None},
        ]}
        self.assertEqual(cb.running_step(build), ("Build", "t1"))

    def test_is_none_between_steps_and_before_the_first(self):
        build = {"buildActions": [
            {"name": "Prepare", "status": "success", "startedAt": "t0"},
            {"name": "Stage", "status": None, "startedAt": None},
        ]}
        self.assertIsNone(cb.running_step(build))


class LogRetry(unittest.TestCase):
    def test_retries_an_empty_log_a_few_times(self):
        # Measured: a step's log read 20 s after its end can still be empty.
        self.assertTrue(cb.retry_log("", 0))
        self.assertTrue(cb.retry_log("  \n", 2))

    def test_gives_up_after_the_last_attempt(self):
        self.assertFalse(cb.retry_log("", cb.LOG_ATTEMPTS))

    def test_prints_a_log_that_arrived(self):
        self.assertFalse(cb.retry_log("22 checks, 0 failure(s)\n", 0))


class CleanLog(unittest.TestCase):
    def test_strips_the_markup_codemagic_wraps_commands_in(self):
        raw = '<span style="color:#268BD2">&gt; git clone x</span>\nHEAD &amp; tail\n'
        self.assertEqual(cb.clean_log(raw), "> git clone x\nHEAD & tail\n")


class Collect(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.dl = self.root / "dl"
        self.dl.mkdir()
        self.out = self.root / "out"

    def tearDown(self):
        self.tmp.cleanup()

    def bundle(self, members):
        path = self.dl / "warren-app_4_artifacts.zip"
        with zipfile.ZipFile(path, "w") as z:
            for name, data in members.items():
                z.writestr(name, data)
        return {"name": path.name, "type": "bundle", "file": path}

    def loose(self, name, data):
        path = self.dl / name
        path.write_bytes(data)
        return {"name": name, "type": "glob_matched", "file": path}

    def test_rebuilds_the_flat_output_from_the_bundle_and_the_loose_zips(self):
        sums = f"{sha(b'm')} *App.msi\n{sha(b'z')} *App.zip\n"
        items = [
            self.bundle({"cm-out/App.msi": b"m", f"cm-out/{cb.SUMS_NAME}": sums}),
            self.loose("App.zip", b"z"),
        ]
        cb.collect(items, self.out)
        self.assertEqual(sorted(p.name for p in self.out.iterdir()), ["App.msi", "App.zip"])

    def test_fails_when_a_file_differs_from_what_the_build_summed(self):
        sums = f"{sha(b'm')} *App.msi\n"
        items = [self.bundle({"cm-out/App.msi": b"tampered", f"cm-out/{cb.SUMS_NAME}": sums})]
        with self.assertRaisesRegex(cb.ProxyError, "App.msi"):
            cb.collect(items, self.out)

    def test_fails_when_a_summed_file_never_arrived(self):
        sums = f"{sha(b'm')} *App.msi\n{sha(b'z')} *App.zip\n"
        items = [self.bundle({"cm-out/App.msi": b"m", f"cm-out/{cb.SUMS_NAME}": sums})]
        with self.assertRaisesRegex(cb.ProxyError, "App.zip"):
            cb.collect(items, self.out)

    def test_fails_on_a_file_the_build_did_not_sum(self):
        sums = f"{sha(b'm')} *App.msi\n"
        items = [self.bundle({"cm-out/App.msi": b"m", "cm-out/extra.txt": b"x",
                              f"cm-out/{cb.SUMS_NAME}": sums})]
        with self.assertRaisesRegex(cb.ProxyError, "extra.txt"):
            cb.collect(items, self.out)

    def test_fails_without_a_checksum_list(self):
        items = [self.bundle({"cm-out/App.msi": b"m"})]
        with self.assertRaisesRegex(cb.ProxyError, cb.SUMS_NAME):
            cb.collect(items, self.out)

    def test_refuses_a_bundle_entry_outside_the_output_directory(self):
        items = [self.bundle({"cm-out/../../escape": b"x"})]
        with self.assertRaises(cb.ProxyError):
            cb.collect(items, self.out)

    def test_refuses_a_nested_output_it_could_not_place_back(self):
        # A zip in a subdirectory arrives loose under its bare name, so the
        # proxy would publish it at the wrong path: outputs must stay flat.
        items = [self.bundle({"cm-out/sub/x.txt": b"x"})]
        with self.assertRaisesRegex(cb.ProxyError, "flat"):
            cb.collect(items, self.out)


if __name__ == "__main__":
    unittest.main()
