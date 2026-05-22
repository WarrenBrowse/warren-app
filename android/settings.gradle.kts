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
    ":lib:billing",
    ":lib:common",
    ":lib:common-compose",
    ":lib:common-test",
    ":lib:grpc",
    ":lib:endpoint",
    // D.4 step 18: account / addtime modules removed (Mullvad-account
    // identity model ; Warren uses BIP39 wallet via the login/wallet
    // module instead).
    ":lib:feature:anticensorship:impl",
    ":lib:feature:anticensorship:api",
    // D.4 step 33: apiaccess module deleted.
    ":lib:feature:appicon:impl",
    ":lib:feature:appicon:api",
    ":lib:feature:appinfo:impl",
    ":lib:feature:appinfo:api",
    ":lib:feature:applisting:impl",
    ":lib:feature:applisting:api",
    ":lib:feature:appearance:impl",
    ":lib:feature:appearance:api",
    ":lib:feature:autoconnect:impl",
    ":lib:feature:autoconnect:api",
    // D.4 step 27: customlist + filter + location modules deleted
    // (Mullvad relay-list picker, replaced by WarrenLocationPicker).
    // D.4 step 32: daita module deleted (DAITA via WarrenTunnelSettings).
    // D.4 step 18: deleteaccount module removed (no Mullvad account on Warren).
    ":lib:feature:home:impl",
    ":lib:feature:home:api",
    ":lib:feature:language:impl",
    ":lib:feature:language:api",
    ":lib:feature:login:impl",
    ":lib:feature:login:api",
    // D.4 step 18: managedevices module removed (Mullvad multi-device
    // accounting model ; Warren manages devices via the wallet).
    // D.4 step 32: multihop module deleted (multi-hop via WarrenTunnelSettings).
    ":lib:feature:notification:impl",
    ":lib:feature:notification:api",
    ":lib:feature:problemreport:impl",
    ":lib:feature:problemreport:api",
    // D.4 step 18: redeemvoucher module removed (Mullvad voucher
    // subscription model ; Warren billing model is different).
    ":lib:feature:serveripoverride:impl",
    ":lib:feature:serveripoverride:api",
    ":lib:feature:settings:impl",
    ":lib:feature:settings:api",
    ":lib:feature:splittunneling:impl",
    ":lib:feature:splittunneling:api",
    ":lib:feature:vpnsettings:impl",
    ":lib:feature:vpnsettings:api",
    ":lib:map",
    ":lib:model",
    ":lib:navigation",
    ":lib:payment",
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
    //   no longer runs. Warren-API-backed tests land in D.6 alongside the
    //   warren-api-client integration.
    ":test:detekt",
    ":test:baselineprofile",
)
