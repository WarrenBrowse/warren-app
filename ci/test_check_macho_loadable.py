#!/usr/bin/env python3
"""Unit tests for ci/check-macho-loadable.py.

Beta 1.1.60 shipped a native module whose string table sat on a 4-byte
boundary. The dyld of macOS 27 refuses such a binary when it was built against
the 27 SDK, the module never loaded, and the app's menu bar icon stopped
answering clicks. Nothing in the build or the release said so. This checker is
what turns that into a red release, so it has to find every Mach-O slice in a
bundle and refuse the layout dyld refuses.

The structural rule is exercised on synthetic Mach-O files, and Apple's own
validator is replaced by a stand-in, so the suite runs on any host.
"""

import importlib.util
import os
import stat
import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

CHECK = Path(__file__).with_name("check-macho-loadable.py")
_SPEC = importlib.util.spec_from_file_location("check_macho_loadable", CHECK)
cm = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(cm)

MH_MAGIC_64 = 0xFEEDFACF
FAT_MAGIC = 0xCAFEBABE
LC_SYMTAB = 0x2
CPU_ARM64 = 0x0100000C
CPU_X86_64 = 0x01000007


def thin(stroff, cpu=CPU_ARM64, size=4096):
    """A 64-bit Mach-O with one LC_SYMTAB, padded to `size` bytes."""
    symtab = struct.pack("<IIIIII", LC_SYMTAB, 24, 1024, 4, stroff, 64)
    header = struct.pack("<IiiIIIII", MH_MAGIC_64, cpu, 0, 6, 1, len(symtab), 0, 0)
    body = header + symtab
    return body + b"\0" * (size - len(body))


def fat(*slices):
    """A universal binary holding the given (cpu, bytes) slices, 4 KiB aligned."""
    offset = 4096
    entries = b""
    payload = b""
    for cpu, data in slices:
        entries += struct.pack(">iiIII", cpu, 0, offset + len(payload), len(data), 12)
        payload += data
    head = struct.pack(">II", FAT_MAGIC, len(slices)) + entries
    return head + b"\0" * (offset - len(head)) + payload


def fake_dyld_info(directory, complaint=""):
    """A dyld_info stand-in that prints the slice header and an optional complaint."""
    tool = Path(directory) / "dyld_info"
    lines = ['for f in "$@"; do [ "$f" = -validate_only ] && continue; echo "$f [arm64]:"']
    if complaint:
        lines.append(f'echo "   {complaint}"; exit 1')
    lines.append("done")
    tool.write_text("#!/bin/sh\n" + "\n".join(lines) + "\n")
    tool.chmod(tool.stat().st_mode | stat.S_IXUSR)
    return str(tool)


class StringPoolAlignment(unittest.TestCase):
    def test_accepts_a_string_pool_on_a_pointer_boundary(self):
        self.assertEqual(cm.layout_defects(thin(stroff=2048)), [])

    def test_refuses_a_string_pool_on_a_four_byte_boundary(self):
        defects = cm.layout_defects(thin(stroff=2052))
        self.assertEqual(len(defects), 1)
        self.assertIn("string pool", defects[0])
        self.assertIn("arm64", defects[0])

    def test_reads_every_slice_of_a_universal_binary(self):
        data = fat((CPU_ARM64, thin(2048)), (CPU_X86_64, thin(2052, cpu=CPU_X86_64)))
        defects = cm.layout_defects(data)
        self.assertEqual(len(defects), 1)
        self.assertIn("x86_64", defects[0])

    def test_is_not_asked_of_a_file_that_is_not_mach_o(self):
        self.assertIsNone(cm.layout_defects(b"#!/bin/sh\necho hi\n"))


class BundleWalk(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.app = self.root / "Example.app" / "Contents"
        (self.app / "Resources" / "native").mkdir(parents=True)
        (self.app / "Resources" / "script.sh").write_text("#!/bin/sh\n")
        (self.app / "MacOS").mkdir()
        (self.app / "MacOS" / "Example").write_bytes(thin(2048))
        self.tools = self.root / "tools"
        self.tools.mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def run_check(self, dyld_info, *paths):
        env = dict(os.environ, WARREN_DYLD_INFO=dyld_info)
        return subprocess.run(
            [sys.executable, "-I", str(CHECK), *map(str, paths)],
            env=env,
            capture_output=True,
            text=True,
        )

    def test_passes_a_bundle_whose_binaries_dyld_accepts(self):
        result = self.run_check(fake_dyld_info(self.tools), self.root / "Example.app")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_fails_on_a_misaligned_module_deep_in_the_bundle(self):
        module = self.app / "Resources" / "native" / "index.node"
        module.write_bytes(thin(2052))
        result = self.run_check(fake_dyld_info(self.tools), self.root / "Example.app")
        self.assertEqual(result.returncode, 1)
        self.assertIn("index.node", result.stderr)

    def test_fails_on_whatever_apples_validator_refuses(self):
        result = self.run_check(
            fake_dyld_info(self.tools, complaint="missing LC_BUILD_VERSION"),
            self.root / "Example.app",
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("missing LC_BUILD_VERSION", result.stderr)

    def test_refuses_to_pass_anything_without_apples_validator(self):
        result = self.run_check(str(self.tools / "absent"), self.root / "Example.app")
        self.assertEqual(result.returncode, 2)

    def test_refuses_a_path_holding_no_mach_o_at_all(self):
        # A wrong path (a renamed bundle, an empty staging dir) must not read
        # as a clean release.
        empty = self.root / "empty"
        empty.mkdir()
        result = self.run_check(fake_dyld_info(self.tools), empty)
        self.assertEqual(result.returncode, 2)


if __name__ == "__main__":
    unittest.main()
