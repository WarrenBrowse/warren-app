# Parity Audit 04, Location / Relay (Exit & Entry) Selection

**Domain:** Location list, search, filters, favorites, custom lists, recents, entry/exit (multihop), automatic, relay-row details, selection display.
**Reference:** Electron desktop (`desktop/packages/mullvad-vpn/`). **Targets:** Android (`android/`), iOS (`ios/`).
**Date:** 2026-05-31. Evidence-based; no source files modified.

---

## Architectural context (read first)

The three platforms diverge sharply in how much of upstream Mullvad's relay-selection stack survived the Warren fork:

- **Electron**, keeps the **full Mullvad select-location feature** (countries→cities→relays hierarchy, search, ownership/provider filters, DAITA/QUIC/LWO filter chips, custom lists with create/edit/delete dialogs, recents with enable/disable, multihop entry/exit scope bar). This is the parity *reference*.
- **iOS**, also keeps the **full Mullvad SwiftUI/UIKit select-location stack** (`ios/WarrenVPN/View controllers/SelectLocation/`): search bar, filters, custom lists, recents, multihop entry/exit, context menu, active-filter pills, active/inactive status dot. Nearly feature-complete vs desktop. Wired via `ApplicationCoordinator` → `LocationCoordinator`.
- **Android**, **replaced** the Mullvad `SelectLocationScreen` with a **purpose-built minimal `WarrenLocationPicker`** (`android/lib/feature/settings/impl/.../WarrenLocationPickerScreen.kt`). It is a **flat list of exit relays** sourced from `WarrenJni.listRelays`, with recents only. No hierarchy, no search, no filters, no custom lists, no entry/multihop selection. The legacy Mullvad `SelectLocationScreen` and its viewmodels were removed (confirmed by deletion comments in `ConnectScreen.kt:304-325`); orphan relaylist UI components remain in `android/lib/ui/component/.../relaylist/` but are not wired.

**Caveat on filter semantics for Warren:** Several desktop/iOS filters (Obfuscation, DAITA, QUIC, LWO, IPv6) are WireGuard/Mullvad-protocol concepts. Warren's data plane is QUIC/Quinn. These filters are **present-but-likely-dead** for Warren on desktop + iOS, flagged in the table.

---

## Parity Table

| Feature | Electron | Android | iOS | Severity | Notes (file:line) |
|---|---|---|---|---|---|
| **Country → city → relay hierarchy (expand/collapse)** | Full accordion hierarchy | Flat exit list only (no hierarchy) | Full hierarchy (expandable nodes) | **P1** | EL: `select-location/components/location-list-item/components/location-accordion/`. AND: `WarrenLocationPickerScreen.kt:103-109` flat `LazyColumn`. iOS: `SelectLocation/Views/LocationListItem.swift`, `LocationNode.swift` |
| **Search / filter locations** | Search field, filters list live | **Absent** | Floating search bar | **P1** | EL: `select-location/components/location-search-field/LocationSearchField.tsx`. iOS: `SelectLocation/Views/FloatingSearchBar.swift`. AND: none in `WarrenLocationPickerScreen.kt` |
| **Filter by provider** | ProviderFilter view + chip | **Absent** | RelayFilter provider section + pill | P2* | EL: `views/filter/components/provider-filter/ProviderFilter.tsx`, chip `providers-filter-chip/`. iOS: `RelayFilter/RelayFilterViewController.swift`, `SelectLocationFilter.swift:43`. *P2 not P1: Warren relay schema (`WarrenRelaySummary`) has no provider field, feature may be non-applicable for Warren |
| **Filter by ownership (owned/rented)** | OwnershipFilter view + chip | **Absent** | owned/rented filter pills | P2* | EL: `views/filter/components/ownership-filter/OwnershipFilter.tsx`. iOS: `SelectLocationFilter.swift:39-45,110-117`. *Warren has no ownership concept, likely non-applicable |
| **Filter by obfuscation** | DAITA/QUIC/LWO filter chips | **Absent** | DAITA/Obfuscation/IPv6 pills | P2-DEAD | EL: `daita-filter-chip/`, `quic-filter-chip/`, `lwo-filter-chip/`. iOS: `SelectLocationFilter.swift:5-11,33-38`. **Present-but-dead for Warren** (WireGuard concepts; Warren=QUIC/Quinn). Flag as dead UI on EL+iOS |
| **Favorites / pinned locations** | **Absent upstream** | Absent | Absent | OK | Mullvad has no pin/favorite; "recents" is the closest analog. No gap. |
| **Custom lists (user-defined groups)** | Full CRUD: create/edit/delete + add/remove location dialogs + menus | **Absent** | Full custom-list stack | **P2** | EL: `features/custom-lists/components/{create,edit,delete}-custom-list-dialog/`, `add-location-to-custom-list-dialog/`. iOS: `Coordinators/CustomLists/` (12 files), `SelectLocation/DataSource/CustomListsDataSource.swift`. AND: none. P2 (power feature) |
| **Recently used locations (recents)** | Recents section + enable/disable + dialog | Recents section (cap 5) | Recents section | OK | EL: `select-location/components/recent-locations/`, `header-menu/HeaderMenu.tsx:42-53` (enable/disable). AND: `WarrenLocationPickerScreen.kt:83-101`, `WarrenLocalSettingsRepository.kt:113-114,190-195` (cap 5). iOS: `SelectLocation/RecentListDataSource.swift`, `RecentsInteractor.swift` |
| → Recents: user toggle to disable | Yes (menu + confirm dialog) | **No toggle** (always on) | (check) | P2 | EL: `HeaderMenu.tsx:42`, `DisableRecentsDialog`. AND: no disable path in `WarrenLocalSettingsRepository.kt` |
| **Entry vs exit selection (multihop)** | Scope bar (Entry/Exit) | **Exit only** (no entry pick) | Multihop entry/exit selection | **P1** | EL: `SelectLocationView.tsx:73-78` scope bar gated on `multihop`. iOS: `SelectLocation/MultihopSelection/`, `Views/{Entry,Exit}LocationView.swift`. AND: picker is exit-only, `selectedExitId` only; no `selectedEntryId` in `WarrenLocalSettingsRepository.kt` |
| **"Closest / automatic" selection** | Implicit (select country = auto relay) | Implicit ("clear → auto-pick first active") | Explicit "Automatic" list item | OK/P2 | EL: country-level selection = auto. AND: `WarrenLocationPickerScreen.kt:78,86-90` second tap clears to auto-pick. iOS: `Views/AutomaticLocationListItem.swift`. Minor UX divergence, not a gap |
| **Relay row: country + city** | Yes | Yes (`country • city`) | Yes | OK | AND: `WarrenLocationPickerScreen.kt:146` |
| **Relay row: server name / endpoint** | Relay hostname | endpoint string | server node label | OK | AND: `WarrenLocationPickerScreen.kt:151` (`relay.endpoint`) |
| **Relay row: latency / ping** | **Absent upstream** | Absent | Absent | OK | Mullvad shows no per-relay ping. No gap across all 3. |
| **Relay row: active / availability indicator** | Status dot | "Inactive" text label + 0.5 alpha | Status dot (active/inactive color) | OK | AND: `WarrenLocationPickerScreen.kt:132,162-168`. iOS: `LocationCell.swift:303-307`, `Views/LocationListItem.swift:34` |
| **Relay row: flag icons** | Country flag icons | **No flags** | Country flag icons | P2 | EL: FlagIcon in location rows. iOS: flag rendering in cells. AND: text-only `country • city`, no flag (`WarrenLocationPickerScreen.kt:146`) |
| **Selected-location display + confirmation** | Highlighted row, immediate apply | Highlighted card + "Selected" label, immediate apply | Highlighted/checked row | OK | AND: `WarrenLocationPickerScreen.kt:137-141,155-160`, persists to `selectedExitId`. Tap-to-confirm, no extra step on any platform |
| **No-search-result empty state** | NoSearchResult component | Empty-catalogue message (no search) | (search empty state) | P2 | EL: `select-location/components/no-search-result/`. AND: `WarrenLocationPickerScreen.kt:71-75` only "no relays" (no search to be empty) |

\* Provider/ownership filters: marked **P2** rather than P1 because the Warren relay model (`WarrenRelaySummary` in `WarrenConnectSurface.kt:75-83`) carries no provider/ownership data, these filters are likely non-applicable to Warren's relay catalogue, not just missing.

---

## Severity summary

- **P1 (real UX parity gaps on Android):**
  1. **No location hierarchy**, Android is a flat exit list vs country→city→relay tree on EL/iOS.
  2. **No search**, Android has no way to filter a long relay list by name.
  3. **No entry/multihop selection**, Android picker is exit-only; cannot choose an entry relay despite Warren supporting multihop. (Contradicts the audit brief's assumption that "Android already has entry/exit country selection", that is **not** present in the current `WarrenLocationPicker`; only a single exit is selectable.)

- **P2 (power/secondary features missing on Android, or dead UI):**
  - Custom lists absent on Android (full on EL+iOS).
  - Flag icons absent on Android.
  - Recents-disable toggle absent on Android.
  - Provider/ownership filters absent on Android (but likely non-applicable to Warren data model).

- **P2-DEAD / flag for cleanup (EL + iOS):**
  - **Obfuscation / DAITA / QUIC / LWO / IPv6 filter chips** (`SelectLocationFilter.swift:5-11`, EL `*-filter-chip/`) are WireGuard/Mullvad-protocol filters surfaced in Warren UI. Warren's transport is QUIC/Quinn, these are **present-but-dead** and should be audited for removal to avoid showing non-functional filters.

- **OK (genuine parity / no upstream feature):**
  - Recents core, selected-location display, active/inactive indicator, automatic selection, server-name display. Favorites and per-relay latency do not exist upstream, no gap.

---

## Top gaps (ranked)

1. **[P1] Android: flat exit-only list**, no country/city/relay hierarchy, no search, no entry/multihop selection. Android is far below EL/iOS parity. The brief's premise that Android already has entry/exit + recents holds only for **recents**; entry/exit and hierarchy are missing.
2. **[P2] Android: no custom lists, no flag icons, no recents-disable toggle.**
3. **[P2-DEAD] EL + iOS: obfuscation/DAITA/QUIC/LWO filter pills are Mullvad-protocol leftovers** likely non-functional for Warren, candidates for removal.
4. **[P2-N/A] Provider/ownership filters** exist on EL+iOS but the Warren relay schema has no provider/ownership data, confirm whether these are intended for Warren at all before treating as a gap.

### Key evidence files
- Electron: `desktop/packages/mullvad-vpn/src/renderer/components/views/select-location/SelectLocationView.tsx`, `.../components/header-menu/HeaderMenu.tsx`, `.../views/filter/FilterView.tsx`, `.../features/custom-lists/`, `.../features/locations/`.
- Android: `android/lib/feature/settings/impl/.../WarrenLocationPickerScreen.kt`, `android/lib/repository/.../WarrenConnectSurface.kt:75`, `android/lib/repository/.../WarrenLocalSettingsRepository.kt:105-195`, `android/lib/feature/home/impl/.../connect/ConnectScreen.kt:304-325` (legacy SelectLocation removal note).
- iOS: `ios/WarrenVPN/View controllers/SelectLocation/` (full stack), `SelectLocationFilter.swift`, `Views/FloatingSearchBar.swift`, `Coordinators/LocationCoordinator.swift`, `Coordinators/CustomLists/`.
