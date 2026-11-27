#!/usr/bin/env python3
"""Refuse a macOS release carrying a Mach-O binary that dyld would not load.

    ci/check-macho-loadable.py <file|bundle-dir>...

Every Mach-O slice found under the given paths is held to two checks:

* its string table starts on an 8-byte boundary. rustc strips Apple binaries
  with rust-objcopy (llvm-objcopy), which can leave it on a 4-byte one. The
  dyld of macOS 27 refuses to load a library with that layout ("mis-aligned
  LINKEDIT string pool") when it was built against the 27 SDK, and accepted it
  before. Beta 1.1.60 shipped such a native module: it never loaded and the
  menu bar icon stopped answering clicks. Executables with the same layout
  still launch, but Apple's validator already refuses them, so they are held
  to the rule too. It is spelled out here so the release fails on any runner,
  whatever macOS its own dyld_info knows.
* Apple's validator, `dyld_info -validate_only`, accepts it. That one tracks
  whatever dyld enforces next.

Exit 0 when every binary passes, 1 when one does not, 2 when the check could
not run (no validator, nothing to check).
"""

import os
import shutil
import struct
import subprocess
import sys

MH_MAGIC_64 = 0xFEEDFACF
FAT_MAGIC = 0xCAFEBABE
FAT_MAGIC_64 = 0xCAFEBABF
LC_SYMTAB = 0x2
CPU_NAMES = {0x0100000C: "arm64", 0x01000007: "x86_64"}


def _slices(data):
    """(cpu, offset) of each 64-bit slice, or None when `data` is not Mach-O."""
    if len(data) < 8:
        return None
    if struct.unpack_from("<I", data)[0] == MH_MAGIC_64:
        return [(struct.unpack_from("<i", data, 4)[0], 0)]
    magic, count = struct.unpack_from(">II", data)
    # 0xCAFEBABE also opens a Java class file, whose next word is a version
    # number far above any plausible slice count.
    if magic not in (FAT_MAGIC, FAT_MAGIC_64) or not 0 < count <= 16:
        return None
    slices = []
    for i in range(count):
        if magic == FAT_MAGIC:
            cpu, _, offset, _, _ = struct.unpack_from(">iiIII", data, 8 + i * 20)
        else:
            cpu, _, offset, _, _, _ = struct.unpack_from(">iiQQII", data, 8 + i * 32)
        if struct.unpack_from("<I", data, offset)[0] == MH_MAGIC_64:
            slices.append((cpu, offset))
    return slices


def layout_defects(data):
    """The layout defects of a Mach-O file's slices, or None for any other file."""
    slices = _slices(data)
    if slices is None:
        return None
    defects = []
    for cpu, base in slices:
        arch = CPU_NAMES.get(cpu, hex(cpu))
        ncmds = struct.unpack_from("<I", data, base + 16)[0]
        cursor = base + 32
        for _ in range(ncmds):
            cmd, size = struct.unpack_from("<II", data, cursor)
            if cmd == LC_SYMTAB:
                stroff = struct.unpack_from("<I", data, cursor + 16)[0]
                if stroff % 8:
                    defects.append(
                        f"[{arch}] string pool at file offset {stroff:#x} "
                        "is not 8-byte aligned"
                    )
            cursor += size
    return defects


def _validator_complaints(tool, path):
    result = subprocess.run(
        [tool, "-validate_only", path], capture_output=True, text=True
    )
    # It prints "<path> [<arch>]:" per slice, then one indented line per defect.
    complaints = [
        line.strip()
        for line in (result.stdout + result.stderr).splitlines()
        if line[:1].isspace() and line.strip()
    ]
    if result.returncode != 0 and not complaints:
        complaints.append(f"dyld_info exited {result.returncode}")
    return complaints


def _files(paths):
    for path in paths:
        if os.path.isfile(path) and not os.path.islink(path):
            yield path
            continue
        for directory, _, names in os.walk(path):
            for name in sorted(names):
                candidate = os.path.join(directory, name)
                if os.path.isfile(candidate) and not os.path.islink(candidate):
                    yield candidate


def main(argv):
    if not argv:
        print("usage: check-macho-loadable.py <file|bundle-dir>...", file=sys.stderr)
        return 2
    tool = shutil.which(os.environ.get("WARREN_DYLD_INFO", "dyld_info"))
    if tool is None:
        print("::error::dyld_info not found, the Mach-O check cannot run", file=sys.stderr)
        return 2

    checked = 0
    failed = 0
    for path in _files(argv):
        with open(path, "rb") as handle:
            head = handle.read(4)
            if len(head) < 4 or (
                struct.unpack("<I", head)[0] != MH_MAGIC_64
                and struct.unpack(">I", head)[0] not in (FAT_MAGIC, FAT_MAGIC_64)
            ):
                continue
            data = head + handle.read()
        defects = layout_defects(data)
        if defects is None:
            continue
        checked += 1
        defects += _validator_complaints(tool, path)
        for defect in defects:
            print(f"::error::{path}: {defect}", file=sys.stderr)
        failed += bool(defects)

    if checked == 0:
        print("::error::no Mach-O binary found under " + " ".join(argv), file=sys.stderr)
        return 2
    print(f"{checked} Mach-O binaries checked, {failed} refused")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
