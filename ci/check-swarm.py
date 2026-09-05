#!/usr/bin/env python3
"""Refuse to call a release done while its torrents have no seeder.

The download page advertises a magnet for every installer, and a magnet into
an empty swarm is indistinguishable from a healthy one until a user tries it.
Nothing about the release pipeline notices: the .torrent uploaded fine, the
manifest is correct, every job is green. The only observable difference is on
the tracker, so that is what this asks.

It scrapes the announce tracker (BEP 15, UDP) for every infohash the release
published and waits for the seedbox to appear. Two failures are told apart on
purpose, because they call for opposite reactions:

  the tracker answered and reports 0 seeders -> the seedbox is not seeding
      this release. Red the run: users are being offered dead magnets.
  the tracker never answered -> nothing can be concluded. Warn, do not fail,
      because a public tracker's outage is not this release's problem.

stdlib only, same reason as the rest of ci/.
"""

from __future__ import annotations

import argparse
import os
import socket
import struct
import sys
import time
import urllib.parse
from collections import namedtuple
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import torrentlib  # noqa: E402

PROTOCOL_ID = 0x41727101980
ACTION_CONNECT = 0
ACTION_SCRAPE = 2
ACTION_ERROR = 3

# BEP 15: a scrape request carries at most 74 infohashes.
MAX_SCRAPE_HASHES = 74

SwarmCount = namedtuple("SwarmCount", ["seeders", "completed", "leechers"])


def tracker_endpoint(announce_url: str) -> tuple:
    parsed = urllib.parse.urlparse(announce_url)
    if parsed.scheme != "udp" or not parsed.hostname or not parsed.port:
        raise ValueError(f"not a UDP announce URL with an explicit port: {announce_url}")
    return parsed.hostname, parsed.port


def batched(items: list, size: int):
    for start in range(0, len(items), size):
        yield items[start : start + size]


def build_scrape_request(connection_id: int, transaction_id: int, infohashes: list) -> bytes:
    header = struct.pack(">QII", connection_id, ACTION_SCRAPE, transaction_id)
    return header + b"".join(bytes.fromhex(h) for h in infohashes)


def parse_scrape_response(payload: bytes, transaction_id: int, infohashes: list) -> dict:
    if len(payload) < 8:
        raise ValueError("scrape reply shorter than its header")
    action, echoed = struct.unpack(">II", payload[:8])
    if echoed != transaction_id:
        # UDP has no connection: a late reply to a previous attempt would
        # otherwise be read against the wrong set of hashes.
        raise ValueError("scrape reply carries another transaction id")
    if action == ACTION_ERROR:
        raise ValueError(f"tracker error: {payload[8:].decode('utf-8', 'replace')}")
    if action != ACTION_SCRAPE:
        raise ValueError(f"unexpected action {action} in a scrape reply")
    body = payload[8:]
    if len(body) < 12 * len(infohashes):
        raise ValueError("scrape reply covers fewer hashes than were asked for")
    counts = {}
    for index, infohash in enumerate(infohashes):
        seeders, completed, leechers = struct.unpack(">III", body[12 * index : 12 * index + 12])
        counts[infohash] = SwarmCount(seeders, completed, leechers)
    return counts


def scrape(host: str, port: int, infohashes: list, timeout: float = 8.0) -> dict:
    """Every infohash's swarm counts, in as few datagrams as the protocol allows."""
    family, _, _, _, address = socket.getaddrinfo(host, port, 0, socket.SOCK_DGRAM)[0]
    counts = {}
    with socket.socket(family, socket.SOCK_DGRAM) as sock:
        sock.settimeout(timeout)
        transaction_id = int.from_bytes(os.urandom(4), "big")
        sock.sendto(struct.pack(">QII", PROTOCOL_ID, ACTION_CONNECT, transaction_id), address)
        reply, _ = sock.recvfrom(2048)
        action, echoed, connection_id = struct.unpack(">IIQ", reply[:16])
        if action != ACTION_CONNECT or echoed != transaction_id:
            raise ValueError("tracker refused the connect handshake")
        for batch in batched(infohashes, MAX_SCRAPE_HASHES):
            transaction_id = int.from_bytes(os.urandom(4), "big")
            sock.sendto(build_scrape_request(connection_id, transaction_id, batch), address)
            reply, _ = sock.recvfrom(4 + 4 + 12 * len(batch))
            counts.update(parse_scrape_response(reply, transaction_id, batch))
    return counts


def collect_torrents(torrent_dir: Path) -> list:
    return [torrentlib.describe_torrent(p) for p in sorted(torrent_dir.glob("*.torrent"))]


def unseeded(counts: dict, infohashes: list) -> list:
    """The hashes the tracker does not report at least one seeder for.

    A hash the reply never covered counts as unseeded: an unanswered scrape is
    not evidence of a healthy swarm.
    """
    return [h for h in infohashes if counts.get(h, SwarmCount(0, 0, 0)).seeders < 1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--torrent-dir", required=True,
                        help="Directory holding this release's .torrent files")
    parser.add_argument("--tracker", default=torrentlib.DEFAULT_TRACKER_TIERS[0][0],
                        help="UDP announce URL to scrape")
    parser.add_argument("--timeout-seconds", type=int, default=900,
                        help="How long to wait for the seedbox to pick the release up. "
                             "The seeder polls the manifest on its own interval, so a "
                             "release is never seeded the instant it is uploaded")
    parser.add_argument("--interval-seconds", type=int, default=60)
    args = parser.parse_args()

    torrents = collect_torrents(Path(args.torrent_dir))
    if not torrents:
        print("::warning::no .torrent in the release; nothing to check")
        return 0
    by_hash = {t.infohash: t for t in torrents}
    host, port = tracker_endpoint(args.tracker)
    print(f"scraping {args.tracker} for {len(torrents)} infohash(es)")

    deadline = time.monotonic() + args.timeout_seconds
    missing = list(by_hash)
    last_error = ""
    while True:
        try:
            counts = scrape(host, port, list(by_hash))
            last_error = ""
            missing = unseeded(counts, list(by_hash))
            for infohash, count in sorted(counts.items()):
                print(f"  {by_hash[infohash].name}: {count.seeders} seeder(s), "
                      f"{count.leechers} leecher(s)")
            if not missing:
                print(f"every artifact of this release has a seeder on {host}")
                return 0
        except (OSError, ValueError) as error:
            last_error = str(error)
            print(f"  scrape failed: {error}", file=sys.stderr)
        if time.monotonic() >= deadline:
            break
        time.sleep(args.interval_seconds)

    if last_error:
        # Never red a release over a public tracker's bad day: the artifacts
        # are published, the web seed serves them, and nothing here observed
        # an actual absence of seeders.
        print(f"::warning::could not reach {host} to verify the swarm ({last_error}); "
              "the seed state of this release is unknown")
        return 0
    names = ", ".join(by_hash[h].name for h in missing)
    print(f"::error::{len(missing)} artifact(s) have no seeder on {host} after "
          f"{args.timeout_seconds}s: {names}. The download page is advertising magnet "
          "links into an empty swarm; check the warren-seeder stack.")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
