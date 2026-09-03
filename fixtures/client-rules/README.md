# Client-rule fixtures

App-level rules that every Warren client implements once per platform, pinned
in one file each and replayed by every platform's own unit tests. The Rust
crates (`warren-forum`, `warren-product-env`) are the reference; the Kotlin,
TypeScript and Swift copies are fail-fast mirrors, and a mirror that disagrees
with a fixture fails its own platform's tests.

These files are app data, so unlike a `warren-vectors` vector they may name
the real product anchors (`connect.warrenbrowse.com`, the real deep-link
schemes). The wire itself (signed bytes, the broker's exact answers) is the
Tier A vector `vectors/forum_login_v1.json`, replayed by
`warren-forum/src/forum_login_vector_tests.rs` and by warren-connect.

A fixture is written against the behaviour at HEAD. Changing a rule means
changing the fixture and every reader in the same commit; a reader must never
be loosened to pass. Where a platform still diverges, the case carries a
`skip` list naming that platform, and the platform's reader skips the case;
every lot that closes a divergence removes its entry. An empty `skip`
everywhere is the definition of parity.

## Readers

| fixture | Rust | Android (JVM) | desktop (vitest) | iOS |
|---|---|---|---|---|
| `forum_link.json` login cases | `warren-forum/tests/client_rules.rs` (sid shape, host allowlist, status and cancel URLs; the URL-level classes have no Rust parser yet) | `ForumLoginLinkTest` (`classifyForumLoginLink`, the full class vocabulary) | `forum-login.spec.ts` (`parseForumLoginUrl`, accept or reject only: desktop has no rejection classes) | `WarrenForumLinkTests` (`WarrenForumLinks.classify`, the full class vocabulary; the scheme comes from the Rust table through `WarrenProductAnchors`) |
| `forum_link.json` attach cases | none (no attach builder in the crate yet) | none (no attach-logs handler on Android) | `forum-login.spec.ts` (`parseForumAttachUrl`) | none |
| `forum_link.json` sign-in codes | `warren-forum/tests/client_rules.rs` (`normalize_sign_in_code`; the crate has no code-to-link builder, so `sign_in_code_cross_device` has no Rust reader) | `ForumLoginLinkTest` (`normalizeForumSignInCode`, `forumLoginLinkFromCode` incl. `sign_in_code_cross_device`) | `forum-login.spec.ts` (`normalizeForumSignInCode`, `forumLoginRequestFromCode` incl. `sign_in_code_cross_device`) | `WarrenForumLinkTests` (`WarrenForumLinks.normalizeSignInCode`, `linkFromCode` incl. `sign_in_code_cross_device`) |
| `forum_link.json` `allowed_hosts`, `schemes`, `pending_ttl_secs` | `client_rules.rs` (hosts, schemes against `product_env.json`) | `ForumLoginLinkTest` (host, login TTL), `ProductEnvBuildConfigTest` (scheme) | `forum-login.spec.ts` (hosts, both TTLs), `product-env.spec.ts` (schemes) | `WarrenForumLinkTests` (hosts, schemes against `product_env.json` and the compiled table) |
| `forum_outcomes.json` login | `client_rules.rs` (`outcome_for_response`, `envelope`) | `ForumLoginOutcomeTest` (decodes `envelope`; `terminal_kinds` through `isTerminalOutcome`) | `forum-login.spec.ts` (`resultForProviderResponse`, `parseForumIdentityResponse`; `terminal_kinds` through `isTerminalForumLoginResult`) | `WarrenForumLoginOutcomeTests` (decodes `envelope`, the client-side failures; `terminal_kinds` through `isTerminal`) |
| `forum_outcomes.json` report | `client_rules.rs` (`report_outcome_for_response`, `report_envelope`) | `ReportOutcomeTest` (decodes `envelope`) | `forum-report.spec.ts` (`forumReportResultForResponse`, the `expect` column: kind, topic, trusted URL, logs, identity) | none |
| `product_env.json` | `warren-product-env/tests/client_rules.rs` (every column, and `ProductEnv::anchors_json()` equals the row), `warren-product-env/tests/platform_lockstep.rs` (`product-env.ts`, `tasks/distribution.cjs`, `android/app/build.gradle.kts` and `ios/Configurations/ProductEnv.xcconfig` read as text and held to the crate, and the iOS `Info.plist` URL scheme held to the xcconfig selector), `warren-forum` `client_rules.rs` (connect host, forum origin) | `ProductEnvBuildConfigTest` (`BuildConfig` of the running flavor against the row, and against the row decoded as the native table `WarrenJni.productAnchorsJson()` returns; `testAllUnitTests` runs the **prod flavor only**, so the beta and staging rows are covered by `platform_lockstep.rs` alone); `ProductAnchorsJniTest` (instrumented, the real native table against `BuildConfig`) | `product-env.spec.ts` (`product-env.ts`, `tasks/distribution.cjs`, the `urls.forum` origin) | `WarrenProductAnchorsTests` (the live table `warren_product_anchors()` returns against the row of the compiled environment, and every row through the Swift decoder) |
| `incident_reports.json` | `mullvad-daemon` `warren_report_budget.rs` (the two budget constants, which the storm outcomes are a pure function of), `warren-jni` `incidents.rs` (every storm replayed through the Android copy of the bucket, and the exit-down reason code through the payload builder) | none (the reports are built and sent in Rust) | none (the daemon is the desktop's engine) | none (no incident reports on iOS yet) |

The exit choice has its vector in the contract sibling rather than here:
`warren-contract/warren-discovery/tests/fixtures/exit_pick.json` pins
`pick_exit` (highest weight, ties on the smallest exit id) and `pick_entry`
(the client's continent first). Its readers in this repo: Rust
`mullvad-daemon` (`exit_vectors_replay_through_the_one_hop_selection`,
`entry_vectors_replay_through_the_pair_ranking`, the daemon's own selection
path) and `warren-jni` (`exit_vectors_replay_through_the_jni_contract`, the
JSON the `resolveExitPin` export carries); Android JVM
`JniExitPinResolverTest` (the request and answer bytes, twinned with the Rust
`the_json_contract_*` tests) and the instrumented `ExitPinJniTest` (the vector
through the real library, the file riding the test APK as an asset). Desktop
has no reader of its own: the daemon is its engine. iOS `select_one_hop`
calls the same `pick_exit`.

The vitest readers run on the Node-only desktop CI machine and must stay free
of cargo and Electron; the Rust readers run in `warren-checks.yml`; the JVM
readers in `android-checks.yml`. A change under `fixtures/client-rules/`
triggers all three. The Swift readers run in the `WarrenVPNCI` test plan
(`ios.yml`, dispatch-only in this fork) and read the fixture directory from
the checkout through the simulator; run them locally with
`xcodebuild -project ios/WarrenVPN.xcodeproj -scheme WarrenVPN -testPlan WarrenVPNCI -destination 'platform=iOS Simulator,name=iPhone 17' test`.

## Skip lists in force

| fixture | case | skipped | why |
|---|---|---|---|
| `forum_link.json` | `no_data` | desktop, ios | an Android intent can carry no data; the desktop is handed argv strings and iOS a URL, so the input does not exist there |

## Schema

Every file carries a `_comment`, a `version` (bumped when a field changes
meaning, never when a case is added) and free-form `_comment` fields on cases,
which readers ignore. A case may carry `"skip": ["desktop", "android", "ios",
"rust"]`.

### `forum_link.json`

- `schemes`: `{env: scheme}`, the deep-link scheme per `WARREN_PRODUCT_ENV`,
  equal to `product_env.json`'s `deep_link_scheme` column.
- `allowed_hosts`: the connect hosts a deep link may name.
- `pending_ttl_secs`: `{login, attach}`, how long a received link stays
  answerable before the client drops it (the broker's own session lifetimes).
- `login_cases[]`: `{name, url, expected_scheme, expect, skip?}` where `url`
  is the raw link (or `null` for no data at all), `expected_scheme` the scheme
  the build under test registers, and `expect` one of
  `{"accepted": {sid, host, cross_device}}` or `{"rejected": <class>}`.
  A reader with rejection classes asserts the class; one without asserts the
  rejection. Classes: `no-data`, `not-a-uri`, `wrong-scheme:<scheme>`,
  `wrong-action`, `missing-sid`, `missing-host`, `bad-sid-shape`,
  `host-not-allowlisted`.
- `attach_cases[]`: same shape for `<scheme>://attach-logs?sid&topic&host`,
  with `accepted` carrying `{sid, host, topic_id}`; extra classes
  `missing-topic` and `bad-topic` (a topic id is a decimal integer that fits
  in a JavaScript safe integer, and `0` is the pre-topic variant).
- `sign_in_code_cases[]`: `{name, typed, expect}`, the sid a typed sign-in
  code stands for, or `null` when it is refused.
- `sign_in_code_cross_device`: whether the link a typed code stands for
  carries the cross-device consent prompt. `true`: a typed code arrives with
  no link and no `xd`, so the app cannot tell one the user read off this
  screen from one an attacker sent them, and only that prompt says approving
  signs in whoever sent the code.

### `forum_outcomes.json`

- `login.cases[]`: `{name, status, body, expect, envelope, skip?}`. `status`
  and `body` are the broker's answer to `POST /v1/forum/login`; `expect` is
  `{kind, handle?, notify_slot?, reason?}` with `kind` in `login._kinds`
  (`failed` carries `reason`, `http-<status>` for an HTTP-born failure);
  `envelope` is the exact JSON the shared crate hands the mobile decoders.
- `login.terminal_kinds`: the outcomes after which the pending link is spent
  and the prompt must not offer a retry.
- `login.client_side_failures.cases[]`: `{name, reason, envelope}`, the
  failures that never reached the broker.
- `report.cases[]`: same shape for `POST /v1/forum/report`; `expect` is
  `{kind, topic_id?, topic_url?, logs?, handle?, notify_slot?, reason?}` with
  `kind` in `report._kinds`. A `topic_url` of `null` means the topic is shown
  without a link (the envelope carries no `topic_url`; the Kotlin decoder reads
  it as the empty string).
- `report.client_side_failures.cases[]`: as for login.

### `product_env.json`

- `environments`: `{env: row}` for `prod`, `staging` and `beta`, each row
  `{name, api_url, api_host, desktop_update_url, display_name,
  unix_product_dir, application_id, deep_link_scheme, connect_host,
  forum_public_url}`. `application_id` is both the Electron `appId` and the
  Android `applicationId`; `connect_host` and `forum_public_url` are the same
  in every row today because one broker and one forum serve all three stacks.
