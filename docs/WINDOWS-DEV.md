# Windows development: build, run and debug warren-app

Moved out of `CLAUDE.md` on 2026-08-11: this is a runbook (prerequisites,
commands, an end state), and the always-loaded layer is for the rules that
bind every session. The VM this usually runs on, and its own traps, are the
`warren-windows-vm` skill.

Supported on Windows 10/11, x64 and ARM64 (a Parallels Windows-on-ARM VM is a
valid target). All tooling lives in `scripts/dev/windows/`. Run the `.sh` helpers
from **Git Bash** and the `.ps1` helpers from PowerShell.

## Prerequisites (one-time, via winget; force `--source winget`, msstore has a cert error)

- **VS 2022 Build Tools** with the C++ workload + the target ARM64 *and* x86/x64
  toolsets + a Windows 11 SDK + clang (`Microsoft.VisualStudio.2022.BuildTools`).
- **Rust** (`Rustlang.Rustup`), then `rustup target add aarch64-pc-windows-msvc
  x86_64-pc-windows-msvc i686-pc-windows-msvc` (i686 is needed for the NSIS plugins).
- **zig**, **Go**, **protoc** (`Google.Protobuf`), **volta** (provides the Node/npm
  pinned in `desktop/package.json`), **Git for Windows**.
- `git config --global core.longpaths true` is **required**: some renderer paths
  exceed the 260-char limit and `git checkout` fails without it.
- podman is NOT needed: the gRPC bindings are committed under
  `desktop/packages/management-interface/dist`.

## Siblings + quinn fork

`../warrenguard`, `../warren-sdk-rs` and `../warren-contract` must be checked out
next to this repo at the SHAs pinned in `.warrenguard-version` /
`.warren-sdk-version` / `.warren-contract-version` (see the dependency layout
section above). The quinn fork needs no local setup: it is the published
`WarrenBrowse/warren-quinn` git-dep pinned by tag in this repo's root
`[patch.crates-io]`, fetched by cargo like any other git dependency. There is no
vendored tree to regenerate and no setup script to run.

## Build

Both wrappers take the product environment as `--prod`, `--beta` or
`--staging` (falling back to `WARREN_PRODUCT_ENV`, which is what the VM's
`wbuild.cmd` / `wbuildbeta.cmd` set) and print their help without building
anything when neither is given.

- Full app + NSIS installer: `scripts/dev/windows/build-app.sh --beta [--optimize]`
  (installer lands in `dist/`).
- Daemon + CLI only, for fast dev iteration:
  `scripts/dev/windows/build-daemon.sh --beta`. It refuses to stage a
  `winfw.dll` whose stamped environment differs from the daemon's: the WFP
  object keys are salted per environment, so a mismatched pair arms the kill
  switch under keys this environment's teardown never sweeps. Rebuild the dll
  with `./build-windows-modules.sh --beta winfw`.

These wrappers source `scripts/vcvars.sh` (locates `vcvarsall.bat` via vswhere, so
Community *or* Build Tools works) and put `msbuild.exe` on PATH (vcvarsall does
not). `scripts/utils/host` detects the host arch from the OS architecture string
**locale-independently** (it must match `*ARM*64*`; a French Windows reports
"Processeur ARM 64 bits"). `build-app.sh` sets `TARGETS=<host triple>` so
electron-builder packages for the host arch (its `pack-windows` defaults to x64
otherwise). Native routing (the split-default that sends traffic through the TUN)
runs through the engine's `warrenguard-winroute` crate (Win32 IP Helper API),
not PowerShell.

## Run / debug

The daemon MUST run as the SYSTEM user on Windows (it edits the WFP firewall and
creates the WinTUN adapter). Use the dev service:

```powershell
# once (elevates via UAC): registers WarrenVPN as a SYSTEM service and delegates
# start/stop to interactive users, so no further elevation is needed.
powershell -ExecutionPolicy Bypass -File scripts/dev/windows/dev-service.ps1 -Action Install
powershell -ExecutionPolicy Bypass -File scripts/dev/windows/dev-service.ps1 -Action Start   # / Stop / Restart / Status / Logs
```

Daemon logs: `dev-logs/daemon.log` (verbose). The Electron app (hot-reload):

```powershell
powershell -ExecutionPolicy Bypass -File scripts/dev/windows/run-gui.ps1
```

`run-gui.ps1` must run in the logged-on user's interactive session (Chromium does
not render from a background/non-interactive context, you get a white window).
