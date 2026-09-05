#!/usr/bin/env python3
"""Unit tests for ci/torrentlib.py.

A torrent's infohash IS its identity: publish a wrong one and every magnet
link on the download page points at a swarm nobody else is in, silently. So
the anchor here is not our own output but an infohash produced independently
by two reference implementations (mktorrent 1.1 and transmission-create 4.x)
over the same fixture bytes, recorded once and pinned for ever.
"""

import hashlib
import importlib.util
import tempfile
import unittest
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "torrentlib", Path(__file__).with_name("torrentlib.py")
)
tl = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(tl)

# 256 KiB of a repeating byte ramp, hashed at 64 KiB pieces: four pieces, the
# last one full, which is the shape both reference tools were run against.
FIXTURE = bytes(range(256)) * 1024
FIXTURE_PIECE_LENGTH = 1 << 16
FIXTURE_NAME = "fixture.bin"
FIXTURE_INFOHASH = "66895753e6140ba100a97ea0eca560b50b95b60f"

TRACKERS = [
    ["udp://tracker.opentrackr.org:1337/announce"],
    ["http://tracker.opentrackr.org:1337/announce"],
]
WEBSEED = "https://api.beta.warrenbrowse.com/updates/desktop/fixture.bin"


def write_fixture(directory: Path, payload: bytes = FIXTURE, name: str = FIXTURE_NAME) -> Path:
    path = directory / name
    path.write_bytes(payload)
    return path


class Bencode(unittest.TestCase):
    def test_encodes_an_integer(self):
        self.assertEqual(tl.bencode(42), b"i42e")

    def test_encodes_a_negative_integer(self):
        self.assertEqual(tl.bencode(-1), b"i-1e")

    def test_encodes_bytes_as_a_length_prefixed_string(self):
        self.assertEqual(tl.bencode(b"spam"), b"4:spam")

    def test_encodes_text_as_its_utf8_bytes(self):
        # A non-ASCII filename must be counted in bytes, not in code points, or
        # every client rejects the torrent as malformed.
        self.assertEqual(tl.bencode("naïve"), b"6:na\xc3\xafve")

    def test_encodes_a_list_in_order(self):
        self.assertEqual(tl.bencode([1, b"a"]), b"li1e1:ae")

    def test_sorts_dictionary_keys_by_raw_bytes(self):
        # Bencode requires sorted keys, and the infohash is taken over these
        # exact bytes: an unsorted dict is a different torrent.
        self.assertEqual(tl.bencode({"b": 2, "a": 1}), b"d1:ai1e1:bi2ee")

    def test_refuses_a_type_it_cannot_represent(self):
        with self.assertRaises(TypeError):
            tl.bencode(1.5)


class PieceLength(unittest.TestCase):
    def test_is_always_a_power_of_two(self):
        for size in (1, 10_000_000, 250_000_000, 4_000_000_000):
            length = tl.choose_piece_length(size)
            self.assertEqual(length & (length - 1), 0, size)

    def test_small_payloads_get_the_floor(self):
        self.assertEqual(tl.choose_piece_length(5_000_000), tl.MIN_PIECE_LENGTH)

    def test_never_exceeds_the_ceiling(self):
        self.assertEqual(tl.choose_piece_length(500 * 1024**3), tl.MAX_PIECE_LENGTH)

    def test_grows_to_keep_the_piece_count_bounded(self):
        # An installer is a few hundred MB; the pieces string stays small
        # enough that the .torrent itself is cheap to serve.
        length = tl.choose_piece_length(8 * 1024**3)
        self.assertLessEqual(8 * 1024**3 // length, tl.TARGET_PIECES * 2)


class HashPieces(unittest.TestCase):
    def test_hashes_each_piece_in_order(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = write_fixture(Path(tmp))

            pieces = tl.hash_pieces(path, FIXTURE_PIECE_LENGTH)

        self.assertEqual(len(pieces), 4 * 20)
        expected = hashlib.sha1(FIXTURE[:FIXTURE_PIECE_LENGTH]).digest()
        self.assertEqual(pieces[:20], expected)

    def test_hashes_a_trailing_partial_piece(self):
        # The last piece is shorter than the rest, and hashing it padded is the
        # classic way to produce a torrent no peer can verify.
        payload = FIXTURE + b"tail"
        with tempfile.TemporaryDirectory() as tmp:
            path = write_fixture(Path(tmp), payload)

            pieces = tl.hash_pieces(path, FIXTURE_PIECE_LENGTH)

        self.assertEqual(len(pieces), 5 * 20)
        self.assertEqual(pieces[-20:], hashlib.sha1(b"tail").digest())


class InfoHash(unittest.TestCase):
    def test_matches_the_reference_implementations(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = write_fixture(Path(tmp))

            info = tl.build_info(path, FIXTURE_PIECE_LENGTH)

        self.assertEqual(tl.infohash_hex(info), FIXTURE_INFOHASH)

    def test_info_carries_only_the_four_v1_single_file_keys(self):
        # Any extra key changes the infohash, so the swarm splits in two the
        # day someone adds one without meaning to.
        with tempfile.TemporaryDirectory() as tmp:
            path = write_fixture(Path(tmp))

            info = tl.build_info(path, FIXTURE_PIECE_LENGTH)

        self.assertEqual(sorted(info), ["length", "name", "piece length", "pieces"])


class Metainfo(unittest.TestCase):
    def build(self, tmp: str, **kwargs) -> dict:
        path = write_fixture(Path(tmp))
        return tl.build_metainfo(
            path,
            tracker_tiers=TRACKERS,
            webseeds=[WEBSEED],
            creation_date=1_700_000_000,
            piece_length=FIXTURE_PIECE_LENGTH,
            **kwargs,
        )

    def test_announce_is_the_first_tracker_of_the_first_tier(self):
        with tempfile.TemporaryDirectory() as tmp:
            meta = self.build(tmp)

        self.assertEqual(meta["announce"], TRACKERS[0][0])

    def test_announce_list_keeps_the_tiers_apart(self):
        # BEP 12 tiers are tried in order, so the primary tracker must not be
        # flattened into one pool with its fallbacks.
        with tempfile.TemporaryDirectory() as tmp:
            meta = self.build(tmp)

        self.assertEqual(meta["announce-list"], TRACKERS)

    def test_carries_the_webseed_as_a_url_list(self):
        with tempfile.TemporaryDirectory() as tmp:
            meta = self.build(tmp)

        self.assertEqual(meta["url-list"], [WEBSEED])

    def test_declares_no_private_flag(self):
        # A private torrent disables DHT and PEX, which is exactly the
        # censorship-resistance this distribution channel exists for.
        with tempfile.TemporaryDirectory() as tmp:
            meta = self.build(tmp)

        self.assertNotIn("private", meta["info"])

    def test_encoding_is_stable_across_runs(self):
        # A rerun of the release job must republish byte-identical torrents,
        # or mirrors and caches see a "new" file for the same release.
        with tempfile.TemporaryDirectory() as tmp:
            first = tl.bencode(self.build(tmp))
            second = tl.bencode(self.build(tmp))

        self.assertEqual(first, second)

    def test_picks_the_piece_length_when_none_is_given(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = write_fixture(Path(tmp))

            meta = tl.build_metainfo(
                path, tracker_tiers=TRACKERS, webseeds=[], creation_date=0
            )

        self.assertEqual(meta["info"]["piece length"], tl.choose_piece_length(len(FIXTURE)))


class Magnet(unittest.TestCase):
    def test_carries_the_infohash_name_size_trackers_and_webseeds(self):
        uri = tl.magnet_uri(
            FIXTURE_INFOHASH,
            FIXTURE_NAME,
            len(FIXTURE),
            tracker_tiers=TRACKERS,
            webseeds=[WEBSEED],
        )

        self.assertTrue(uri.startswith(f"magnet:?xt=urn:btih:{FIXTURE_INFOHASH}"))
        self.assertIn(f"dn={FIXTURE_NAME}", uri)
        self.assertIn(f"xl={len(FIXTURE)}", uri)
        self.assertIn("tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce", uri)
        self.assertIn("ws=https%3A%2F%2Fapi.beta.warrenbrowse.com", uri)

    def test_lists_every_tracker_of_every_tier(self):
        uri = tl.magnet_uri(
            FIXTURE_INFOHASH, FIXTURE_NAME, 1, tracker_tiers=TRACKERS, webseeds=[]
        )

        self.assertEqual(uri.count("&tr="), 2)


class WriteTorrent(unittest.TestCase):
    def test_writes_the_file_and_reports_what_it_published(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = write_fixture(root)
            out_dir = root / "torrents"

            result = tl.write_torrent(
                path,
                out_dir,
                tracker_tiers=TRACKERS,
                webseeds=[WEBSEED],
                creation_date=1_700_000_000,
            )

            written = out_dir / f"{FIXTURE_NAME}.torrent"
            self.assertTrue(written.is_file())
            self.assertEqual(result.filename, written.name)
            self.assertEqual(
                result.infohash, tl.infohash_hex(tl.build_info(path, result.piece_length))
            )
            self.assertIn(result.infohash, result.magnet)

    def test_the_torrent_name_is_the_payload_name_plus_the_suffix(self):
        # The download page pairs a .torrent with its installer by that exact
        # name; renaming the pattern breaks the pairing everywhere at once.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = write_fixture(root, name="WarrenVPN-1.2.3-linux-amd64.deb")

            result = tl.write_torrent(
                path,
                root / "torrents",
                tracker_tiers=TRACKERS,
                webseeds=[],
                creation_date=0,
            )

        self.assertEqual(result.filename, "WarrenVPN-1.2.3-linux-amd64.deb.torrent")


class DefaultTrackers(unittest.TestCase):
    def test_opentrackr_leads_the_first_tier(self):
        self.assertEqual(
            tl.DEFAULT_TRACKER_TIERS[0], ["udp://tracker.opentrackr.org:1337/announce"]
        )

    def test_the_http_fallback_reaches_the_same_tracker(self):
        # UDP is blocked on a lot of the corporate and campus networks Warren
        # users sit behind; opentrackr answers a plain HTTP announce on the
        # same port, so the primary tracker stays reachable there.
        self.assertIn("http://tracker.opentrackr.org:1337/announce", tl.DEFAULT_TRACKER_TIERS[1])

    def test_every_tracker_is_declared_once(self):
        flat = [url for tier in tl.DEFAULT_TRACKER_TIERS for url in tier]
        self.assertEqual(len(flat), len(set(flat)))


if __name__ == "__main__":
    unittest.main()
