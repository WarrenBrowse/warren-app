# Vendored swift-log

Vendored copy of [apple/swift-log](https://github.com/apple/swift-log)
v1.8.0 `Sources/Logging/` directory (4 files, ~2 472 LOC).

## Why vendored ?

Xcode 26.4 swift-driver explicit-module + implicit-module both fail
to resolve the `Logging` swiftmodule when consumed from the
`WarrenLogging` framework target. Logging.swiftmodule emits to
`SourcePackages/checkouts/swift-log/build/Release-iphoneos/` but
WarrenLogging's compile flags do not include that search path
(regression introduced by the WireGuardKit SPM removal).

Multiple attempts at workarounds failed :
- `SWIFT_ENABLE_EXPLICIT_MODULES = NO` flips the error from
  "Unable to resolve module dependency" to "no such module" (deeper
  failure).
- `-I "$(SOURCE_PACKAGES_PATH)/checkouts/swift-log/build/$(CONFIGURATION)$(EFFECTIVE_PLATFORM_NAME)"`
  injected into `OTHER_SWIFT_FLAGS` + `SWIFT_INCLUDE_PATHS` +
  `FRAMEWORK_SEARCH_PATHS` : no change.
- Re-adding `wireguard-apple` package_reference as dummy SPM
  placeholder : no change.

Vendoring directly into `WarrenLogging/Vendor/SwiftLog/` bypasses
the SPM consumer-path entirely : the swift-log files are compiled
as part of `WarrenLogging` itself, so the `Logging` types become
part of the `WarrenLogging` module symbol space. Consumers continue
to `import WarrenLogging` and pick up the `Logger`/`LogHandler`/etc.
types transitively (via the existing `@_exported import Logging`
patterns, which need to be removed since `Logging` is no longer a
distinct module).

## License

swift-log is Apache 2.0. Warren is AGPL-3.0. The vendored copy
retains the Apache 2.0 header comments verbatim per the LICENSE
requirements. No source modifications were made — these are
verbatim copies of swift-log 1.8.0 `Sources/Logging/`.

## Update procedure

When swift-log upstream releases a new version :
1. Bump the version pin in this README.
2. `cp -r $(swift-log)/Sources/Logging/*.swift WarrenLogging/Vendor/SwiftLog/`.
3. Re-run xcodebuild and validate no API breaks.
