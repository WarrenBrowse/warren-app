#!/usr/bin/env python3
"""Run one Codemagic build for the current GitHub Actions job and wait for it.

The Windows jobs of every Warren repo run on Codemagic `windows_x2` machines,
while GitHub Actions stays the orchestrator: the release DAG (`needs:`), the
checks agents read with `gh run list`, and the GitHub tokens never leave it.
This script is the bridge. It starts the requested workflow on the job's exact
commit, replays each Codemagic step log into the job log as it ends, cancels
the build when the job is cancelled, and on success rebuilds the flat output
directory the build produced and verifies it against the build's own checksum
list before anything downstream can publish it.

Contract with the Codemagic side (codemagic.yaml + the repo's scripts):
  * every workflow declares a required `sha` input and builds that commit;
  * outputs are written flat into `cm-out/` together with `codemagic.sha256`
    (sha256sum format) covering every one of them.

Standard library only: it runs on any Linux runner with python3.
"""

import argparse
import hashlib
import html
import json
import os
import re
import shutil
import signal
import sys
import tempfile
import time
import urllib.error
import urllib.request
import zipfile
from pathlib import Path, PurePosixPath

API_V1 = "https://api.codemagic.io"
API_V3 = "https://codemagic.io/api/v3"
OUT_DIR = "cm-out"
SUMS_NAME = "codemagic.sha256"
HEARTBEAT = 300
LOG_ATTEMPTS = 3
TERMINAL = {"finished", "failed", "canceled", "timeout", "skipped"}
INPUT_KEY = re.compile(r"^[a-zA-Z]\w*$")
FULL_SHA = re.compile(r"^[0-9a-f]{40}$")


class ProxyError(Exception):
    pass


def parse_inputs(text):
    inputs = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line:
            continue
        key, sep, value = line.partition("=")
        key = key.strip()
        if not sep:
            raise ProxyError(f"input line without '=': {line!r}")
        if not INPUT_KEY.match(key):
            raise ProxyError(f"input id {key!r} is not a valid Codemagic input id")
        if key == "sha":
            raise ProxyError("the commit is the job's own and cannot be passed as an input")
        inputs[key] = value.strip()
    return inputs


def codemagic_ref(ref_type, ref_name, head_ref):
    if ref_type == "tag":
        return ("tag", ref_name)
    return ("branch", head_ref or ref_name)


def start_payload(workflow, ref, sha, inputs, labels):
    if not FULL_SHA.match(sha):
        raise ProxyError(f"not a full commit SHA: {sha!r}")
    kind, name = ref
    return {
        "workflow_id": workflow,
        kind: name,
        "inputs": {**inputs, "sha": sha},
        "labels": list(labels),
    }


def is_terminal(status):
    return status in TERMINAL


def succeeded(status):
    return status == "finished"


def finished_steps(build, seen):
    """Steps that ended since the last call, as (name, status, log url)."""
    ended = []
    for action in build.get("buildActions") or []:
        key = action.get("_id") or action.get("name")
        status = action.get("status")
        if key in seen or status is None:
            continue
        seen.add(key)
        url = action.get("logUrl")
        for sub in action.get("subactions") or []:
            url = url or sub.get("logUrl")
        ended.append((action.get("name", "?"), status, url))
    return ended


def running_step(build):
    """The step that has started and not ended, as (name, start), or None."""
    for action in build.get("buildActions") or []:
        if action.get("startedAt") and action.get("status") is None:
            return (action.get("name", "?"), action["startedAt"])
    return None


def retry_log(text, attempts):
    """Whether a step log read empty is worth reading again later."""
    return not text.strip() and attempts < LOG_ATTEMPTS


def clean_log(text):
    return html.unescape(re.sub(r"<[^>]+>", "", text))


def _read_sums(path):
    sums = {}
    for line in path.read_text().splitlines():
        if not line.strip():
            continue
        digest, _, name = line.partition(" ")
        sums[name.lstrip(" *")] = digest.lower()
    return sums


def collect(items, dest):
    """Rebuild the flat output directory from the downloaded artifacts.

    Codemagic hands a build's `.zip` outputs back one by one under their bare
    names and packs everything else into a `bundle` archive that keeps the
    `cm-out/` prefix. The result is only trusted once it matches the build's
    own checksum list exactly: nothing missing, nothing altered, nothing extra.
    """
    dest = Path(dest)
    dest.mkdir(parents=True, exist_ok=True)
    for item in items:
        if item["type"] == "bundle":
            with zipfile.ZipFile(item["file"]) as bundle:
                for member in bundle.infolist():
                    if member.is_dir():
                        continue
                    parts = PurePosixPath(member.filename).parts
                    if ".." in parts or parts[:1] != (OUT_DIR,):
                        raise ProxyError(f"bundle entry outside {OUT_DIR}/: {member.filename}")
                    if len(parts) != 2:
                        raise ProxyError(
                            f"{member.filename}: outputs must be flat in {OUT_DIR}/, "
                            "a nested zip could not be placed back"
                        )
                    with bundle.open(member) as src, open(dest / parts[1], "wb") as dst:
                        shutil.copyfileobj(src, dst)
        else:
            shutil.copyfile(item["file"], dest / item["name"])

    sums_path = dest / SUMS_NAME
    if not sums_path.is_file():
        raise ProxyError(f"the build published no {SUMS_NAME}; refusing unverified outputs")
    expected = _read_sums(sums_path)
    sums_path.unlink()
    present = {p.name for p in dest.iterdir() if p.is_file()}
    problems = []
    for name in sorted(expected.keys() - present):
        problems.append(f"missing: {name}")
    for name in sorted(present - expected.keys()):
        problems.append(f"not in {SUMS_NAME}: {name}")
    for name in sorted(expected.keys() & present):
        digest = hashlib.sha256((dest / name).read_bytes()).hexdigest()
        if digest != expected[name]:
            problems.append(f"checksum mismatch: {name}")
    if problems:
        raise ProxyError("; ".join(problems))
    return sorted(present)


class Client:
    def __init__(self, token):
        self.token = token

    def request(self, method, url, body=None, attempts=5):
        data = json.dumps(body).encode() if body is not None else None
        for attempt in range(1, attempts + 1):
            req = urllib.request.Request(url, data=data, method=method)
            req.add_header("x-auth-token", self.token)
            req.add_header("Content-Type", "application/json")
            try:
                with urllib.request.urlopen(req, timeout=60) as resp:
                    return resp.read()
            except urllib.error.HTTPError as err:
                if err.code < 500 and err.code != 429:
                    detail = err.read().decode(errors="replace")[:500]
                    raise ProxyError(f"{method} {url}: HTTP {err.code}: {detail}") from err
                if attempt == attempts:
                    raise ProxyError(f"{method} {url}: HTTP {err.code}") from err
            except (urllib.error.URLError, TimeoutError, ConnectionError) as err:
                if attempt == attempts:
                    raise ProxyError(f"{method} {url}: {err}") from err
            time.sleep(min(60, 5 * attempt))

    def json(self, method, url, body=None):
        return json.loads(self.request(method, url, body) or b"{}")

    def download(self, url, path):
        with open(path, "wb") as out:
            out.write(self.request("GET", url))


def _group(title, text):
    print(f"::group::{title}", flush=True)
    sys.stdout.write(text if text.endswith("\n") else text + "\n")
    print("::endgroup::", flush=True)


def _output(name, value):
    target = os.environ.get("GITHUB_OUTPUT")
    if target:
        with open(target, "a") as out:
            out.write(f"{name}={value}\n")


def run(args):
    token = os.environ.get("CODEMAGIC_API_TOKEN", "")
    if not token:
        raise ProxyError("CODEMAGIC_API_TOKEN is empty (org secret missing?)")
    client = Client(token)
    ref = codemagic_ref(
        os.environ.get("GITHUB_REF_TYPE", "branch"),
        os.environ.get("GITHUB_REF_NAME", ""),
        os.environ.get("GITHUB_HEAD_REF", ""),
    )
    if not ref[1]:
        raise ProxyError("cannot tell which ref to build (GITHUB_REF_NAME is empty)")
    sha = args.sha or os.environ.get("GITHUB_SHA", "")
    run_url = "{}/{}/actions/runs/{}".format(
        os.environ.get("GITHUB_SERVER_URL", "https://github.com"),
        os.environ.get("GITHUB_REPOSITORY", "?"),
        os.environ.get("GITHUB_RUN_ID", "?"),
    )
    payload = start_payload(args.workflow, ref, sha, parse_inputs(args.inputs), [f"github {run_url}"])
    started = client.json("POST", f"{API_V3}/apps/{args.app_id}/builds", payload)
    build_id = started["data"]["id"]
    build_url = f"https://codemagic.io/app/{args.app_id}/build/{build_id}"
    _output("build-id", build_id)
    _output("build-url", build_url)
    print(f"Codemagic build {build_url}", flush=True)
    print(f"  workflow {args.workflow}, {ref[0]} {ref[1]} at {sha}", flush=True)

    def cancel(signum, _frame):
        print(f"job cancelled (signal {signum}): cancelling the Codemagic build", flush=True)
        try:
            client.request("POST", f"{API_V1}/builds/{build_id}/cancel", {}, attempts=2)
        finally:
            sys.exit(130)

    signal.signal(signal.SIGTERM, cancel)
    signal.signal(signal.SIGINT, cancel)

    def replay(steps, final=False):
        """Print the logs that arrived; return the steps to read again."""
        later = []
        for name, step_status, url, attempts in steps:
            log = clean_log(client.request("GET", url).decode(errors="replace")) if url else ""
            if url and retry_log(log, attempts) and not final:
                later.append((name, step_status, url, attempts + 1))
            else:
                _group(f"{name} [{step_status}]", log or "(Codemagic returned no log for this step)\n")
        return later

    # A step reports its end before its log is flushed: read at that moment,
    # and even one poll later, a script step's log came back empty. Each log
    # is read one poll after its step ended and read again while it is empty,
    # up to LOG_ATTEMPTS polls; the last ones get the same retries 20 s apart.
    # A running step's log cannot be read, so the job says which step is
    # running and repeats it every HEARTBEAT seconds: a long step and a hung
    # one then look different in the job log only by what follows, never by
    # silence.
    seen, pending, status, last, current, beat = set(), [], None, None, None, 0.0
    while True:
        build = client.json("GET", f"{API_V1}/builds/{build_id}")["build"]
        status = build.get("status")
        if status != last:
            print(f"status: {status}", flush=True)
            last = status
        pending = replay(pending)
        step = running_step(build)
        if step and (step != current or time.monotonic() - beat >= HEARTBEAT):
            first = step != current
            current, beat = step, time.monotonic()
            print(f"{'running' if first else 'still running'}: {step[0]} (since {step[1]})", flush=True)
        pending += [(n, st, u, 0) for n, st, u in finished_steps(build, seen)]
        if is_terminal(status):
            for attempt in range(LOG_ATTEMPTS + 1):
                time.sleep(20)
                pending = replay(pending, final=attempt == LOG_ATTEMPTS)
                if not pending:
                    break
            break
        time.sleep(args.poll_seconds)

    if not succeeded(status):
        raise ProxyError(f"Codemagic build {status}: {build.get('message') or 'no message'} ({build_url})")

    if args.out_dir:
        with tempfile.TemporaryDirectory() as tmp:
            items = []
            for art in build.get("artefacts") or []:
                path = Path(tmp) / art["name"]
                client.download(art["url"], path)
                items.append({"name": art["name"], "type": art.get("type"), "file": path})
            names = collect(items, args.out_dir)
        print(f"verified {len(names)} output(s) into {args.out_dir}/:", flush=True)
        for name in names:
            print(f"  {name}", flush=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--app-id", required=True)
    parser.add_argument("--workflow", required=True)
    parser.add_argument("--inputs", default="", help="KEY=VALUE lines passed as build inputs")
    parser.add_argument("--sha", default="", help="commit to build (default: GITHUB_SHA)")
    parser.add_argument("--out-dir", default="", help="where to place the verified outputs")
    parser.add_argument("--poll-seconds", type=int, default=20)
    args = parser.parse_args(argv)
    try:
        run(args)
    except ProxyError as err:
        print(f"::error title=Codemagic::{err}", flush=True)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
