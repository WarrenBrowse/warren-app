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
`warren-forum/tests/forum_login_vector.rs` and by warren-connect.

A fixture is written against the behaviour at HEAD. Changing a rule means
changing the fixture and every reader in the same commit; a reader must never
be loosened to pass. Where a platform still diverges, the case carries a
`skip` list naming that platform, and the platform's reader skips the case;
every lot that closes a divergence removes its entry. An empty `skip`
everywhere is the definition of parity.

## Readers

| fixture | Rust | Android (JVM) | desktop (vitest) | iOS |
|---|---|---|---|---|
| `forum_link.json` login cases | `warren-forum/tests/client_rules.rs` (sid shape, host allowlist, status and cancel URLs; the URL-level classes have no Rust parser yet) | `ForumLoginLinkTest` (`classifyForumLoginLink`, the full class vocabulary) | `forum-login.spec.ts` (`parseForumLoginUrl`, accept or reject only: desktop has no rejection classes) | none yet (no parser test; `SceneDelegate.parseForumLogin` answers the prod scheme only) |
| `forum_link.json` attach cases | none (no attach builder in the crate yet) | none (no attach-logs handler on Android) | `forum-login.spec.ts` (`parseForumAttachUrl`) | none |
| `forum_link.json` sign-in codes | `warren-forum/tests/client_rules.rs` (`normalize_sign_in_code`) | `ForumLoginLinkTest` (`normalizeForumSignInCode`) | `forum-login.spec.ts` (`normalizeForumSignInCode`, `forumLoginRequestFromCode`) | none |
| `forum_link.json` `allowed_hosts`, `schemes`, `pending_ttl_secs` | `client_rules.rs` (hosts, schemes against `product_env.json`) | `ForumLoginLinkTest` (host, login TTL), `ProductEnvBuildConfigTest` (scheme) | `forum-login.spec.ts` (hosts, both TTLs), `product-env.spec.ts` (schemes) | none |
| `forum_outcomes.json` login | `client_rules.rs` (`outcome_for_response`, `envelope`) | `ForumLoginOutcomeTest` (decodes `envelope`; `terminal_kinds` through `isTerminalOutcome`) | `forum-login.spec.ts` (`resultForProviderResponse`, `parseForumIdentityResponse`; `terminal_kinds` through `isTerminalForumLoginResult`) | none yet (`WarrenForumLoginOutcomeTests` pins four envelopes by hand) |
| `forum_outcomes.json` report | `client_rules.rs` (`report_outcome_for_response`, `report_envelope`) | `ReportOutcomeTest` (decodes `envelope`) | `forum-report.spec.ts` (`forumReportResultForResponse`, the `expect` column: kind, topic, trusted URL, logs, identity) | none |
| `product_env.json` | `warren-product-env/tests/client_rules.rs` (every column, and `ProductEnv::anchors_json()` equals the row), `warren-product-env/tests/platform_lockstep.rs` (`product-env.ts`, `tasks/distribution.cjs` and `android/app/build.gradle.kts` read as text and held to the crate), `warren-forum` `client_rules.rs` (connect host, forum origin) | `ProductEnvBuildConfigTest` (`BuildConfig` of the running flavor against the row, and against the row decoded as the native table `WarrenJni.productAnchorsJson()` returns); `ProductAnchorsJniTest` (instrumented, the real native table against `BuildConfig`) | `product-env.spec.ts` (`product-env.ts`, `tasks/distribution.cjs`) | none yet (`warren_product_anchors()` exposes the table; no Swift reader, no product-env plumbing in the Xcode build) |

The vitest readers run on the Node-only desktop CI machine and must stay free
of cargo and Electron; the Rust readers run in `warren-checks.yml`; the JVM
readers in `android-checks.yml`. A change under `fixtures/client-rules/`
triggers all three.

## Skip lists in force

| fixture | case | skipped | why |
|---|---|---|---|
| `forum_link.json` | `no_data` | desktop, ios | an Android intent can carry no data; the desktop is handed argv strings and iOS a URL, so the input does not exist there |
| `forum_link.json` | `beta_link_on_beta_build`, `prod_link_on_beta_build` | ios | iOS registers and parses the prod scheme only; a beta install cannot answer the beta broker (shared-code step 4) |
| `forum_outcomes.json` login | `approved_with_identity`, `approved_without_slot` | ios | the iOS decoder drops the handle and the slot (shared-code step 1) |
| `forum_outcomes.json` login | `expired` | ios | iOS collapses the 404 into the generic failure and keeps the prompt armed for a doomed retry (shared-code step 1) |

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
