# M4.H.C - short report

**Date** : 2026-05-20
**Verdict** : **GO ULTIMATE**
**Wall-clock** : ~3h
**Cost Hetzner** : 0.00 EUR (UI phase, no infra)

## TL;DR

UI Electron Warren cabled around the M4.E.D stack already wired by
M4.H.B. New dedicated multi-hop view + toggle + entry/exit country
pickers, live status display (reconnect_count + age + M4.0 obfuscation
indicator), and M4.0 always-on obfuscation info banner that replaces
the Mullvad anti-censorship picker when `warren_mode` is on. Plus
opportunistic refactor M4.H.C.PRE that consumes the canonical HTTP
signature primitives from warren-api-client (single source of truth).

8 atomic commits pushed to origin/main warren-app.

## Pipeline gates

- `cargo fmt --check`: clean
- `cargo clippy --workspace --all-targets -- -D warnings`: **No issues found**
- `cargo test --workspace --lib`: **424 passed / 1 ignored / 40 suites**
- `npx tsc --noEmit -p packages/mullvad-vpn`: clean
- `npm test` (desktop/packages/mullvad-vpn): **110 passed / 15 suites**
- `npm run lint`: 7 pre-existing errors in Account/Keys/RestoreMnemonic
  views (inherited from R1 rebrand; verified out of M4.H.C scope).

## M4.H.C.PRE - signature HTTP dedup (commit b2621965)

- Added `pub use warren_identity::auth::{HEADER_*, canonical_message}`
  re-export in `warren-api-client` (cross-repo warren-core bump,
  pinned `bb9b7895`).
- Removed local `HEADER_*` constants and `fn canonical_message` in
  `mullvad-api/src/warren_auth.rs`; consume the re-export instead.
- Wire-format regression test `canonical_message_matches_hardcoded_reference_vector`
  pins the exact byte string for POST /v1/port-forward/request to
  guarantee zero drift post-refactor.
- Header constants regression test pins the 4 `X-Warren-*` header
  names verbatim.

## M4.H.C.0 + M4.H.C.1 - gRPC + daemon handlers (commit 4a96f346)

- Proto extension: `WarrenMultiHopSettings` and `WarrenStatus`
  messages + `GetWarrenMultiHopSettings`, `SetWarrenMultiHopSettings`,
  `GetWarrenStatus`, `WarrenStatusUpdates` (stream) RPCs.
- TS bindings regenerated via the podman codegen container (after
  resetting podman-machine which had been broken).
- `mullvad_types::settings::Settings.warren_multi_hop:
  WarrenMultiHopSettings` field with proto roundtrip + 5 unit tests.
- Daemon: new `warren_status` module (`WarrenStatusCache` +
  `WarrenStatusSnapshot`, `record_reconnect`,
  `set_obfuscation_active`) with 8 unit tests including a
  saturating-counter regression.
- `ManagementServiceImpl` holds a clone of the cache so `get_warren_status`
  and `warren_status_updates` (watch-channel-backed stream) read it
  without round-tripping through the daemon command channel.
- `DaemonCommand::SetWarrenMultiHopSettings` persists via the existing
  settings persister; restart required to apply (mirrors
  `SetWarrenMode`).

## M4.H.C.2 - UI multi-hop view (commit 464f0b0e)

- `desktop/.../shared/daemon-rpc-types.ts`: new `WarrenMultiHopSettings`
  + `WarrenStatus` interfaces. `ISettings.warrenMultiHop` field.
- `grpc-type-convertions.ts`: convertFromSettings populates
  `warrenMultiHop`; new `convertFromWarrenMultiHopSettings` /
  `convertToWarrenMultiHopSettings` / `convertFromWarrenStatus`.
- `daemon-rpc.ts`: `getWarrenMultiHopSettings`, `setWarrenMultiHopSettings`,
  `getWarrenStatus`, plus subscribe/unsubscribe stream helpers.
- IPC schema: `settings.setWarrenMultiHop` invoke channel + `warrenStatus`
  notifyRenderer channel.
- Redux: `IUpdateWarrenMultiHopAction` + `IUpdateWarrenStatusAction`,
  initial state defaults aligned with doctrine (OFF / 4h rotation).
- New view `warren-multi-hop-settings/WarrenMultiHopSettingsView` +
  feature folder `features/warren-multi-hop` (hook
  `useWarrenMultiHop`, components `WarrenMultiHopSwitch`,
  `WarrenMultiHopSetting`, `WarrenMultiHopCountryPickers`).
- `RoutePath.warrenMultiHopSettings = '/settings/warren-multi-hop'`
  registered in AppRouter.
- `Settings.tsx` linked the new entry via `WarrenMultiHopListItem`.

## M4.H.C.3 - status display (commit c4c9ebcd)

- `ConnectionDetails.tsx` extended with `WarrenStatusRows` (rendered
  only when warren_mode is on): reconnect count, "Last reconnect"
  formatted age, obfuscation active indicator.
- Main process subscribes to `WarrenStatusUpdates` at daemon connect
  and forwards via the new `warrenStatus` IPC channel; teardown on
  daemon disconnect / bootstrap error.
- Renderer dispatches `updateWarrenStatus` into the redux store on
  every push.

## M4.H.C.4 - killswitch IPv6 + DNS (already exposed)

- The Mullvad upstream `EnableIpv6Setting` (= block IPv6 when off)
  and `DnsBlockerSettings` are already exposed via the existing
  `VpnSettingsView` in the Warren UI flow (linked from
  `SettingsView`). M4.E.C IPv6 killswitch + F11+Round25 DNS leak
  doctrine documents that they work correctly in Warren mode. No
  new toggle required.

## M4.H.C.5 - obfuscation indicator (commit 902c1a7a)

- `AntiCensorshipView` conditional on `warrenMode`: when off, the
  Mullvad picker (Shadowsocks / UDP-over-TCP / QUIC / LWO) is
  preserved; when on, a `WarrenObfuscationIndicator` info banner
  replaces it.
- Banner reads `warrenStatus.obfuscationActive`, defaults to true if
  no snapshot yet (avoids alarming false OFF on boot).
- Per doctrine `warren_obfuscation_doctrine_v1`: no toggle in /v1.

## M4.H.C.6 - i18n FR + EN (commit 39713110)

- `npm run update-translations` extracted 21 new strings into
  `messages.pot`.
- All 21 strings translated in `locales/fr/messages.po`
  (warren-multi-hop-view + warren-status-view contexts + the
  settings-view "Warren multi-hop" navigation label).

## Caveats residuels (out-of-scope)

1. **Lint debt** (Account/Keys/RestoreMnemonic views): 7 pre-existing
   `react/jsx-no-bind` errors inherited from R1 rebrand. Hardening
   them would touch unrelated code paths.
2. **Warren multi-hop supervisor metrics**: `WarrenStatusCache.record_reconnect`
   exists but the multi-hop supervisor in talpid-warren-tunnel does
   not yet call it. Wiring it is M4.H.C.X (small follow-up) so the
   live counter actually advances; until then it stays at 0 and the
   UI displays "Reconnects: 0 / Last: never" steady state.
3. **Hetzner SSH provisioning bug** (M4.H.B caveat): scope ops poka,
   does not affect M4.H.C UI work.
4. **daemon-fork `account create` Remote LOCAL=0** (M4.H.A.X caveat):
   not touched, scope opportuniste not encountered.
5. **wapi VAL1/2 client-side regression**: independent of warren-app.

## Commits push origin/main

1. `b262196502` refactor(mullvad-api): consume canonical_message + HEADER_* from warren-api-client
2. `4a96f346f4` feat(mgmt-iface,daemon): expose Warren multi-hop settings + status via gRPC
3. `d149ada0e8` docs(warren): translate French comments to English (cross-repo)
4. `464f0b0eec` feat(desktop): Warren multi-hop UI view + toggle + country pickers
5. `137065b732` docs(warren): translate remaining French comments to English (rust crates)
6. `c4c9ebcd67` feat(desktop): Warren live status display
7. `902c1a7a58` feat(desktop): Warren M4.0 obfuscation indicator
8. `3971311094` i18n(desktop): FR + EN strings

Plus the cross-repo warren-core commit `bb9b7895`
("feat(warren-api-client): re-export HEADER_* + canonical_message").

## Memory updates

- `warren_m4h_c_delivered.md` (new)
- `project_warren_app_state_post_m4hc.md` (new, supersedes
  `project_warren_app_state_post_m4hb` for the UI surface)
- `MEMORY.md` index updated

## Next steps orchestrateur

- **M4.H.C.X follow-up**: wire `WarrenStatusCache.record_reconnect`
  call from the multi-hop supervisor in `talpid-warren-tunnel` so
  the live counter actually advances. Small (one call site, the cache
  handle is already accessible via a `Daemon`-held clone).
- **M4.H.D débloqué**: build pipeline DMG / AppImage / MSI + signing
  keys + CI release.
- Lint hardening on inherited views (Account/Keys/RestoreMnemonic)
  if a dedicated rebrand cleanup phase is opened.
- Bench infra Hetzner SSH bug investigation (still scope ops poka).
