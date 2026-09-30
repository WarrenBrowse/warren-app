# Per-app route plan fixtures

The route plan of "country per app" (`docs/app-routing.md` sections 2.2 and
2.6) has one implementation of its rules, `warren_app_routes::plan`, and one
adapter per client: the desktop daemon's (`mullvad-daemon/src/warren_app_routes.rs`)
and the Android engine's (`warren-jni/src/app_routes_plan.rs`). Each adapter
keeps what the client does its own way: where the exits in force come from,
how a city is spelled, how an exit is picked inside a choice, how the main
exit is known, and the format the statuses are shown in.
`plan_cases.json` pins what both must plan and show from the same inputs, and
each client replays it through its real adapter, so a change of either
adapter, or of the shared rules, that makes one client plan differently fails
that client's tests.

A case is written against the behaviour at HEAD. Changing a rule means
changing the fixture and both readers in the same commit; a reader is never
loosened to pass. Every case picks its nodes so that each choice has one exit
both selectors must land on (the heaviest by `pick_exit` on one hop, the only
pair on two), since the two selectors are allowed to differ where a choice has
several.

## Readers

| client | test | how the case is fed |
|---|---|---|
| desktop | `mullvad-daemon` `warren_app_routes::tests::the_plan_parity_cases_replay_through_the_daemon` | app `x` is the executable `/opt/apps/x` (`C:\apps\x.exe` on Windows) in `AppRoutingSettings`; the main circuit is the one-hop circuit on `main_exit`; `plan` then `statuses`, reasons by the serde names of `UnavailableReason` |
| Android | `warren-jni` `app_routes_plan::tests::the_plan_parity_cases_replay_through_the_android_planner` | app `x` is the package `org.x` in the `AppExitSpec` list Kotlin sends; `plan` then `statuses_json`, read back from the JSON Kotlin parses |

Both run in `warren-tests.yml` (the Warren test scope), which a change here
triggers. The reader of the file and the signed pretend fleet it builds live in
`warren-app-routes/src/plan/fixture.rs`, behind the crate's `test-helpers`
feature.

## Skip lists in force

A case the client cannot express, or on which it still diverges, carries a
`skip` list naming it, and that client's reader skips the case. Closing a
divergence removes its entry in the same commit.

| case | skipped | why |
|---|---|---|
| `without_a_main_exit_its_country_gets_a_route` | android | Android has no custom exit: its main session is always on an exit |

## Schema

- `_comment`, and `_comment` on a case: free text, ignored.
- `version`: 1. Bumped when a field changes meaning, never when a case is
  added.
- `nodes`: the directory every case plans in. A node of `tag` `n` has relay id
  and exit id `[n; 16]`, keys derived from `n`, and is listed on
  `198.51.100.n:443`, which is also the address its apps appear from.
  `country` (lowercase), `city` (the relay list's name), `weight`.
- `cases[]`:
  - `name`; `skip` (optional, `desktop` or `android`).
  - `two_hop`: whether the main connection has two hops, so every route does.
  - `entry_country`: the main connection's entry country, `null` for any.
  - `main_exit`: the tag of the exit the main session is on, `null` while a
    custom exit is on.
  - `drained` (optional): tags of the exits that announced a drain.
  - `previous` (optional): the circuits of the last plan, each a choice
    (`country`, `city`) with the `entry` and `exit` tags of its circuit.
  - `exits`: the exits in force, each an `app` name, a `country` and a `city`
    (a lowercase relay-list city code) or `null`.
  - `connected`: whether the main connection is up; `reports` (optional): what
    the tunnel reports of its route sessions, each an `exit` tag and a `state`
    among `connecting`, `connected`, `waiting`, `no_token`, `refused`,
    `failed`, `no_reachable_entry`.
  - `expect`:
    - `routes`: the route sessions, in order, each with its `entry` and `exit`
      tags and its `apps`.
    - `main`: the apps the main session carries, by main `exit` tag.
    - `blocked`: the apps no session carries.
    - `resolutions`: per choice (`country`, `city`), `kind` `main` with its
      `public_ip`, `route` with its `exit` tag and `public_ip`, or
      `unavailable` with its `reason`.
    - `statuses`: what the user sees per choice, in order: `state`
      (`connecting`, `connected`, `unavailable`), `reason` (`tunnel_down`,
      `no_token`, `limit_reached`, `no_relay`, `waiting_for_route`,
      `no_dialable_network`, or `null`), `public_ip`, and the `apps`.
