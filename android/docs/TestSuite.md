# Test Suite Overview

This document describes the test suites of the Warren VPN Android app and how to run them.

## What runs on every push

`.github/workflows/android-checks.yml` runs on every push and pull request touching `android/**`
or `warren-jni/**`:

- `ktfmtCheck`: Kotlin formatting.
- `detekt`: static analysis against `config/detekt.yml`. Findings that predate the gate are listed
  in `config/detekt-baseline.xml`; a new finding fails the job.
- `:app:lintProdDebug`: Android lint against `config/lint.xml` and `config/lint-baseline.xml`.
- `testAllUnitTests`: every JVM unit test (the `prodDebug` variant for the modules that have
  flavors), `:test:arch` and `:test:detekt` included.

## Test types

### 1. Unit tests (`src/test` of each module)

JVM tests of the use cases, view models, repositories, the tunnel adapter and the planners.
`StringResourceCoverageTest` (`app/src/test`) holds every locale to the English keys.

### 2. Instrumented tests (`src/androidTest`)

Compose UI tests and the tests that go through the real `libwarren_jni.so`, such as
`ExitPinJniTest` and `ProductAnchorsJniTest`. They need a device or an emulator and no workflow
runs them. A connected run ends with AGP uninstalling the app under test, which erases its data:
`gradle.properties` sets `android.injected.androidTest.leaveApksInstalledAfterRun=true` so the run
installs over the app instead. Run them against a device whose wallet you can re-provision.

### 3. Architecture tests (`test/arch/`)

[Konsist](https://docs.konsist.lemonappdev.com/) rules on the code base, for example:

- view models carry the `ViewModel` suffix, and their tests install `TestCoroutineRule`;
- Compose previews are private and start with `Preview`;
- data classes hold only immutable properties;
- no `collectAsState()` (the lifecycle-aware collector is used instead), no `android.util.Log`
  outside the frame-timing logcat line.

### 4. Detekt rules (`test/detekt/`)

Custom detekt rules. The only one today checks that `Screen` and `Dialog` composables are called
with named arguments.

### 5. Baseline profile generation (`test/baselineprofile/`)

Not a test suite: it drives the app through the UI tags to generate the ART baseline profile, see
`BuildInstructions.md`, "Baseline profile".

## Running tests

```bash
./gradlew testAllUnitTests            # JVM unit tests, test:arch included
./gradlew ktfmtCheck detekt           # format and static analysis
./gradlew :app:lintProdDebug          # Android lint
./gradlew :app:connectedBetaDebugAndroidTest \
    -Pandroid.testInstrumentationRunnerArguments.class=<fully.qualified.TestClass>
```

To check that a change adds no detekt finding, run `./gradlew detekt --rerun-tasks`: a plain
`detekt` prints nothing when it is up to date.
