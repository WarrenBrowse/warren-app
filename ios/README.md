# Warren VPN iOS app

This is the iOS version of the Warren VPN app.

The iOS app has not been released yet. When it is, `release.yml` builds it from the same tags
as the other platforms (`vX.Y.Z` for prod, `beta-vX.Y.Z` for beta), once the repository
variable `WARREN_ENABLE_IOS` is set and the signing secrets exist. Changes are listed in the
[changelog].

For Xcode project configuration please refer to [Configure Xcode project] section of build 
instructions document.

[changelog]: CHANGELOG.md
[Configure Xcode project]: BuildInstructions.md#configure-xcode-project

## Code formatting

The codebase is formatted using [swift-format](https://github.com/swiftlang/swift-format). Please 
format all contributions using the latest version of formatter.

```
./ios/format.sh format
```

## Screenshots for AppStore

The process of taking AppStore screenshots is automated using a UI Testing bundle and Snapshot tool,
a part of Fastlane tools.

### Configuration

The screenshot run logs in with a real test wallet, provided to the UI-test runner through its
environment (see `WarrenVPNUITests/README.md`); `Screenshots.xcconfig` only carries signing
configuration.

1. Create the build configuration using a template file:
   
   ```
   cp ios/Configurations/Screenshots.xcconfig.template ios/Configurations/Screenshots.xcconfig
   ```

1. Adjust the signing settings in the configuration file if needed:
   
   ```
   vim ios/Configurations/Screenshots.xcconfig
   ```

### Prerequisites

1. Make sure you have [rvm](https://rvm.io) installed.
1. Install Ruby 2.5.1 or later using `rvm install <VERSION>`.
1. Install necessary third-party ruby gems:
   
   ```
   cd ios
   bundle install
   ```

### Take screenshots

Run the following command to take screenshots:

```
cd ios
bundle exec fastlane snapshot
```

Once done all screenshots should be saved under `ios/Screenshots` folder.

### Localizations

The app is translated through the string catalogs (`*.xcstrings`) under `ios/Assets/`,
`ios/PacketTunnel/` and `ios/WarrenVPN/Supporting Files/`, one entry per language.

## Bundled relay list

`WarrenREST/Assets/relays.json` is committed. The "Check relays" build phase
(`checkRelays.swift`) refuses a Release or MockRelease build whose copy is missing, empty, or
lists no relay.
