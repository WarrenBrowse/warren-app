#!/usr/bin/env python3
"""BitTorrent v1 metainfo for the Warren release pipeline.

Turns one release installer into one single-file .torrent plus its magnet URI.
stdlib only, like the metadata generator that imports it: the release runners
carry a stock python3 and nothing else, so a dependency here would have to be
provisioned on every pool before a release could be cut.

Three choices are load-bearing and must not drift:

- **v1 only.** A hybrid v1+v2 torrent would be strictly better, but no verified
  hybrid creator exists on the release runners, and an unverifiable merkle tree
  shipped under a magnet link is a swarm nobody can join. Revisit when a
  reference implementation can cross-check the output in CI, the way
  test_torrentlib.py cross-checks the v1 infohash today.
- **No `private` flag.** DHT and PEX are what keep the release reachable when
  the tracker or the website is blocked, which is the whole point of offering
  BitTorrent to VPN users.
- **A webseed on every torrent** (BEP 19). The update host already serves the
  installer over HTTPS, so declaring it as a web seed makes the swarm
  permanently seeded at zero extra infrastructure: a lone downloader with no
  peer still completes at full speed.
"""

from __future__ import annotations

import hashlib
import urllib.parse
from collections import namedtuple
from pathlib import Path

# BEP 12 tiers, tried in order. Tier 0 is the tracker Warren announces on;
# tier 1 is the same tracker over HTTP, which is what still answers on the
# networks that drop UDP; tier 2 is redundancy, so a single tracker outage
# never leaves a release with tracker-less peer discovery (DHT still works,
# but it is slower to bootstrap a young swarm).
DEFAULT_TRACKER_TIERS = [
    ["udp://tracker.opentrackr.org:1337/announce"],
    ["http://tracker.opentrackr.org:1337/announce"],
    [
        "udp://open.demonii.com:1337/announce",
        "udp://tracker.torrent.eu.org:451/announce",
        "udp://open.stealth.si:80/announce",
    ],
]

# Pieces are powers of two, floored at 256 KiB (below it the pieces string
# bloats the .torrent for no transfer benefit) and capped at 4 MiB (above it a
# peer re-downloads too much after a failed hash check).
MIN_PIECE_LENGTH = 1 << 18
MAX_PIECE_LENGTH = 1 << 22
TARGET_PIECES = 1500

CREATED_BY = "warren-release"

TorrentResult = namedtuple(
    "TorrentResult", ["filename", "infohash", "magnet", "piece_length", "pieces"]
)

TorrentDescription = namedtuple(
    "TorrentDescription", ["infohash", "name", "size", "trackers", "webseeds"]
)


def bencode(value) -> bytes:
    """Bencode a metainfo value.

    Dictionary keys are emitted sorted by their raw bytes, which the format
    requires and the infohash depends on.
    """
    if isinstance(value, bool):
        # bool is an int subclass; encoding True as i1e would be a silent
        # type confusion in a structure hashed into an identity.
        raise TypeError("bencode does not represent booleans")
    if isinstance(value, int):
        return b"i%de" % value
    if isinstance(value, bytes):
        return b"%d:%s" % (len(value), value)
    if isinstance(value, str):
        return bencode(value.encode("utf-8"))
    if isinstance(value, (list, tuple)):
        return b"l" + b"".join(bencode(item) for item in value) + b"e"
    if isinstance(value, dict):
        items = sorted(
            (key.encode("utf-8") if isinstance(key, str) else key, val)
            for key, val in value.items()
        )
        return b"d" + b"".join(bencode(k) + bencode(v) for k, v in items) + b"e"
    raise TypeError(f"bencode cannot represent {type(value).__name__}")


def choose_piece_length(size_bytes: int) -> int:
    """Smallest power of two that keeps the piece count near TARGET_PIECES."""
    length = MIN_PIECE_LENGTH
    while length < MAX_PIECE_LENGTH and size_bytes // length > TARGET_PIECES:
        length <<= 1
    return length


def hash_pieces(path: Path, piece_length: int) -> bytes:
    """Concatenated SHA-1 of each piece, the trailing partial one included."""
    digests = []
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(piece_length)
            if not chunk:
                break
            digests.append(hashlib.sha1(chunk).digest())
    return b"".join(digests)


def build_info(path: Path, piece_length: int) -> dict:
    """The hashed half of a single-file metainfo.

    Nothing may be added here without changing every published infohash.
    """
    return {
        "length": path.stat().st_size,
        "name": path.name,
        "piece length": piece_length,
        "pieces": hash_pieces(path, piece_length),
    }


def infohash_hex(info: dict) -> str:
    return hashlib.sha1(bencode(info)).hexdigest()


def build_metainfo(
    path: Path,
    *,
    tracker_tiers: list,
    webseeds: list,
    creation_date: int,
    piece_length: int | None = None,
    comment: str | None = None,
) -> dict:
    info = build_info(path, piece_length or choose_piece_length(path.stat().st_size))
    meta = {
        "announce": tracker_tiers[0][0],
        "announce-list": [list(tier) for tier in tracker_tiers],
        "created by": CREATED_BY,
        "creation date": creation_date,
        "info": info,
    }
    if webseeds:
        meta["url-list"] = list(webseeds)
    if comment:
        meta["comment"] = comment
    return meta


def magnet_uri(
    infohash: str, name: str, size_bytes: int, *, tracker_tiers: list, webseeds: list
) -> str:
    """A magnet that works without the .torrent ever being fetched.

    `ws` carries the web seed, so a visitor who only copies the magnet still
    gets the permanently seeded HTTPS source rather than depending on peers.
    """
    quote = lambda value: urllib.parse.quote(str(value), safe="")  # noqa: E731
    parts = [f"xt=urn:btih:{infohash}", f"dn={quote(name)}", f"xl={size_bytes}"]
    for tier in tracker_tiers:
        parts.extend(f"tr={quote(url)}" for url in tier)
    parts.extend(f"ws={quote(url)}" for url in webseeds)
    return "magnet:?" + "&".join(parts)


def write_torrent(
    path: Path,
    out_dir: Path,
    *,
    tracker_tiers: list,
    webseeds: list,
    creation_date: int,
    comment: str | None = None,
) -> TorrentResult:
    """Write `<installer name>.torrent` next to nothing, and describe it.

    The name pairs the torrent with its installer on the download page and on
    the update host; the prune step there matches on the same pattern.
    """
    meta = build_metainfo(
        path,
        tracker_tiers=tracker_tiers,
        webseeds=webseeds,
        creation_date=creation_date,
        comment=comment,
    )
    out_dir.mkdir(parents=True, exist_ok=True)
    filename = f"{path.name}.torrent"
    (out_dir / filename).write_bytes(bencode(meta))
    info = meta["info"]
    infohash = infohash_hex(info)
    return TorrentResult(
        filename=filename,
        infohash=infohash,
        magnet=magnet_uri(
            infohash,
            info["name"],
            info["length"],
            tracker_tiers=tracker_tiers,
            webseeds=webseeds,
        ),
        piece_length=info["piece length"],
        pieces=len(info["pieces"]) // 20,
    )


def bdecode(data: bytes):
    """Decode a whole bencoded document, refusing anything left over.

    Trailing bytes mean a truncated or concatenated file, which must not be
    read as if it were a valid torrent.
    """
    value, offset = _bdecode_at(data, 0)
    if offset != len(data):
        raise ValueError(f"{len(data) - offset} trailing byte(s) after the document")
    return value


def _bdecode_at(data: bytes, i: int):
    kind = data[i : i + 1]
    if kind == b"i":
        end = data.find(b"e", i)
        if end < 0:
            raise ValueError("unterminated integer")
        return int(data[i + 1 : end]), end + 1
    if kind == b"l":
        i += 1
        out = []
        while True:
            if i >= len(data):
                raise ValueError("unterminated list")
            if data[i : i + 1] == b"e":
                return out, i + 1
            value, i = _bdecode_at(data, i)
            out.append(value)
    if kind == b"d":
        i += 1
        out = {}
        while True:
            if i >= len(data):
                raise ValueError("unterminated dictionary")
            if data[i : i + 1] == b"e":
                return out, i + 1
            key, i = _bdecode_at(data, i)
            out[key], i = _bdecode_at(data, i)
    colon = data.find(b":", i)
    if colon < 0:
        raise ValueError("unterminated string length")
    length = int(data[i:colon])
    end = colon + 1 + length
    if end > len(data):
        raise ValueError("string runs past the end of the document")
    return data[colon + 1 : end], end


def describe_torrent(path: Path) -> TorrentDescription:
    """Read back what a published .torrent announces.

    The infohash is taken over the RAW `info` bytes of the file rather than
    over a re-encoding of the decoded dict, so a torrent written by any other
    tool is read at its true identity.
    """
    raw = path.read_bytes()
    meta = bdecode(raw)
    start = raw.find(b"4:info")
    if start < 0:
        raise ValueError(f"{path.name} carries no info dictionary")
    start += len(b"4:info")
    _, end = _bdecode_at(raw, start)
    info = meta[b"info"]
    trackers = [
        url.decode("utf-8")
        for tier in meta.get(b"announce-list", [])
        for url in tier
    ] or [meta[b"announce"].decode("utf-8")]
    return TorrentDescription(
        infohash=hashlib.sha1(raw[start:end]).hexdigest(),
        name=info[b"name"].decode("utf-8"),
        size=info[b"length"],
        trackers=trackers,
        webseeds=[url.decode("utf-8") for url in meta.get(b"url-list", [])],
    )


def infohash_of_file(path: Path) -> str:
    return describe_torrent(path).infohash
