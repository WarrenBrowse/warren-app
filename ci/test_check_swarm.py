#!/usr/bin/env python3
"""Unit tests for ci/check-swarm.py.

The release publishes magnet links whether or not anything seeds them, and a
magnet into an empty swarm looks exactly like a healthy one on the download
page. This checker is what turns that into a red run, so its parsing of the
UDP tracker protocol (BEP 15) has to be exact: a misread response would
either pass a dead swarm or fail a live one.

Only the pure protocol halves are exercised here. The live tracker round trip
belongs to the release job, not to a unit suite that must stay offline.
"""

import importlib.util
import struct
import tempfile
import unittest
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "check_swarm", Path(__file__).with_name("check-swarm.py")
)
cs = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(cs)

HASH_A = "aa" * 20
HASH_B = "bb" * 20


class TrackerEndpoint(unittest.TestCase):
    def test_reads_the_host_and_port_of_a_udp_announce(self):
        self.assertEqual(
            cs.tracker_endpoint("udp://tracker.opentrackr.org:1337/announce"),
            ("tracker.opentrackr.org", 1337),
        )

    def test_refuses_a_tracker_it_cannot_scrape(self):
        # The HTTP tier exists for clients behind UDP-blocking networks; this
        # checker speaks the UDP protocol only, and must say so rather than
        # silently reporting an unreachable tracker.
        with self.assertRaises(ValueError):
            cs.tracker_endpoint("http://tracker.opentrackr.org:1337/announce")


class ScrapeRequest(unittest.TestCase):
    def test_lays_out_the_connection_action_and_hashes(self):
        payload = cs.build_scrape_request(0x1122334455667788, 0xDEADBEEF, [HASH_A, HASH_B])

        connection_id, action, transaction_id = struct.unpack(">QII", payload[:16])
        self.assertEqual(connection_id, 0x1122334455667788)
        self.assertEqual(action, cs.ACTION_SCRAPE)
        self.assertEqual(transaction_id, 0xDEADBEEF)
        self.assertEqual(payload[16:], bytes.fromhex(HASH_A) + bytes.fromhex(HASH_B))


class ScrapeResponse(unittest.TestCase):
    def response(self, transaction_id: int, triples: list) -> bytes:
        body = b"".join(struct.pack(">III", *triple) for triple in triples)
        return struct.pack(">II", cs.ACTION_SCRAPE, transaction_id) + body

    def test_reads_one_triple_per_hash_in_order(self):
        payload = self.response(7, [(3, 10, 1), (0, 0, 0)])

        parsed = cs.parse_scrape_response(payload, 7, [HASH_A, HASH_B])

        self.assertEqual(parsed[HASH_A], cs.SwarmCount(seeders=3, completed=10, leechers=1))
        self.assertEqual(parsed[HASH_B].seeders, 0)

    def test_refuses_a_reply_to_another_transaction(self):
        # UDP: a late reply to a previous attempt would otherwise be read as
        # the answer to this one, on the wrong set of hashes.
        payload = self.response(8, [(3, 10, 1), (0, 0, 0)])

        with self.assertRaises(ValueError):
            cs.parse_scrape_response(payload, 7, [HASH_A, HASH_B])

    def test_refuses_an_error_action(self):
        payload = struct.pack(">II", cs.ACTION_ERROR, 7) + b"nope"

        with self.assertRaises(ValueError):
            cs.parse_scrape_response(payload, 7, [HASH_A])

    def test_refuses_a_truncated_body(self):
        payload = self.response(7, [(3, 10, 1)])

        with self.assertRaises(ValueError):
            cs.parse_scrape_response(payload, 7, [HASH_A, HASH_B])


class Batching(unittest.TestCase):
    def test_never_asks_for_more_hashes_than_a_datagram_holds(self):
        # BEP 15 caps a scrape at 74 hashes; asking for more silently truncates
        # the answer, which would read as "those releases have no seeder".
        batches = list(cs.batched([str(i) for i in range(150)], cs.MAX_SCRAPE_HASHES))

        self.assertEqual([len(b) for b in batches], [74, 74, 2])


class Collecting(unittest.TestCase):
    def test_reads_every_torrent_of_a_release_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            payload = root / "WarrenVPN-1.2.3-linux-amd64.deb"
            payload.write_bytes(b"x" * 4096)
            cs.torrentlib.write_torrent(
                payload,
                root,
                tracker_tiers=cs.torrentlib.DEFAULT_TRACKER_TIERS,
                webseeds=[],
                creation_date=0,
            )

            described = cs.collect_torrents(root)

        self.assertEqual([d.name for d in described], ["WarrenVPN-1.2.3-linux-amd64.deb"])

    def test_an_empty_directory_yields_nothing_to_check(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(cs.collect_torrents(Path(tmp)), [])


class Verdict(unittest.TestCase):
    def test_a_release_is_seeded_when_every_artifact_has_a_seeder(self):
        counts = {HASH_A: cs.SwarmCount(1, 0, 0), HASH_B: cs.SwarmCount(4, 2, 9)}

        self.assertEqual(cs.unseeded(counts, [HASH_A, HASH_B]), [])

    def test_an_artifact_the_tracker_never_answered_for_counts_as_unseeded(self):
        # A missing entry is not a pass: it means the scrape did not cover it.
        self.assertEqual(cs.unseeded({HASH_A: cs.SwarmCount(1, 0, 0)}, [HASH_A, HASH_B]), [HASH_B])

    def test_a_swarm_with_only_leechers_is_not_seeded(self):
        counts = {HASH_A: cs.SwarmCount(seeders=0, completed=5, leechers=3)}

        self.assertEqual(cs.unseeded(counts, [HASH_A]), [HASH_A])


class TorrentUrlsFromManifest(unittest.TestCase):
    """The scheduled watch has no release directory: it starts from the manifest."""

    def test_collects_the_torrent_of_every_platform(self):
        doc = {
            "platforms": {
                "linux": {"assets": [{"torrent": "https://h/a.deb.torrent"}, {"url": "https://h/b"}]},
                "macos": {"assets": [{"torrent": "https://h/c.pkg.torrent"}]},
            }
        }

        self.assertEqual(
            cs.torrent_urls(doc), ["https://h/a.deb.torrent", "https://h/c.pkg.torrent"]
        )

    def test_a_manifest_predating_the_channel_yields_nothing(self):
        # Not an error: the channel simply has not published a torrent yet, and
        # the watch must say so rather than fail.
        self.assertEqual(cs.torrent_urls({"platforms": {"linux": {"assets": [{"url": "x"}]}}}), [])

    def test_tolerates_a_document_with_no_platforms_at_all(self):
        self.assertEqual(cs.torrent_urls({}), [])


if __name__ == "__main__":
    unittest.main()
