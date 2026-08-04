pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal() {
            content {
                // Exclude gRPC artifacts - they're only available in Maven Central
                excludeGroup("io.grpc")
            }
        }
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

includeBuild("gradle/build-logic")

enableFeaturePreview("TYPESAFE_PROJECT_ACCESSORS")

rootProject.name = "WarrenVPN"

include(":app")

include(
    // lib:billing deleted (Mullvad Play Store VPN billing
    // dead on Warren - BIP39 wallet replaces VPN subscriptions).
    ":lib:common",
    ":lib:common-compose",
    ":lib:common-test",
    // lib:grpc deleted (Mullvad daemon gRPC bridge dead).
    ":lib:endpoint",
    // account / addtime modules removed (Mullvad-account
    // identity model ; Warren uses BIP39 wallet via the login/wallet
    // module instead).
    // anticensorship module deleted.
    // apiaccess module deleted.
    // feature.appicon module deleted (Mullvad obfuscation dead).
    ":lib:feature:appinfo:impl",
    ":lib:feature:appinfo:api",
    ":lib:feature:applisting:impl",
    ":lib:feature:applisting:api",
    ":lib:feature:autoconnect:impl",
    ":lib:feature:autoconnect:api",
    // customlist + filter + location modules deleted
    // (Mullvad relay-list picker, replaced by WarrenLocationPicker).
    // daita module deleted (DAITA via WarrenTunnelSettings).
    // deleteaccount module removed (no Mullvad account on Warren).
    ":lib:feature:home:impl",
    ":lib:feature:home:api",
    ":lib:feature:language:impl",
    ":lib:feature:language:api",
    ":lib:feature:login:impl",
    ":lib:feature:login:api",
    // managedevices module removed (Mullvad multi-device
    // accounting model ; Warren manages devices via the wallet).
    // multihop module deleted (multi-hop via WarrenTunnelSettings).
    ":lib:feature:notification:impl",
    ":lib:feature:notification:api",
    // redeemvoucher module removed (Mullvad voucher
    // subscription model ; Warren billing model is different).
    // serveripoverride module deleted (Warren exit fleet
    // is sovereign ; no per-relay IP overrides).
    ":lib:feature:settings:impl",
    ":lib:feature:settings:api",
    ":lib:feature:splittunneling:impl",
    ":lib:feature:splittunneling:api",
    // feature.vpnsettings module deleted (Mullvad daemon
    // MTU/DNS/QuantumResistant/etc. settings sync dead on Warren).
    ":lib:map",
    ":lib:model",
    ":lib:navigation",
    // lib:payment deleted (Mullvad PaymentProvider abstraction
    // is dead alongside lib:billing on Warren).
    ":lib:push-notification",
    ":lib:repository",
    ":lib:screen-test",
    ":lib:talpid",
    ":lib:tv",
    ":lib:ui:designsystem",
    ":lib:ui:component",
    ":lib:ui:icon",
    ":lib:ui:resource",
    ":lib:ui:tag",
    ":lib:ui:theme",
    ":lib:ui:util",
    ":lib:usecase",
)

include(
    ":test",
    ":test:arch",
    ":test:common",
    ":test:e2e",
    // ":test:mockapi" - dropped: simulates the Mullvad API for tests Warren
    //   no longer runs. Warren-API-backed tests land alongside the
    //   warren-api-client integration.
    ":test:detekt",
    ":test:baselineprofile",
)
