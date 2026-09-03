import com.android.build.api.artifact.SingleArtifact
import com.android.build.api.variant.BuildConfigField
import com.github.triplet.gradle.androidpublisher.ReleaseStatus
import org.gradle.internal.extensions.stdlib.capitalized
import utilities.BuildTypes
import utilities.FlavorDimensions
import utilities.Flavors
import utilities.Variant
import utilities.appVersionProvider
import utilities.baselineFilter
import utilities.configureComposeCompiler
import utilities.fullReleaseTasks
import utilities.generateRemapArguments
import utilities.getBooleanProperty
import utilities.getStringListProperty
import utilities.isReleaseBuild
import utilities.matchesAny
import utilities.prodDebugReleaseVariants
import utilities.registerReleaseTask

plugins {
    alias(libs.plugins.warren.utilities)
    alias(libs.plugins.android.application)
    alias(libs.plugins.play.publisher)
    alias(libs.plugins.kotlin.parcelize)
    // Required so the @Serializable WarrenTunnelConfig gets a generated
    // serializer; without it Json.encodeToString/decodeFromString throw
    // "Serializer not found" at runtime when (de)serialising the tunnel config.
    alias(libs.plugins.kotlinx.serialization)
    alias(libs.plugins.kotlin.ksp)
    alias(libs.plugins.compose)
    alias(libs.plugins.baselineprofile)
    alias(libs.plugins.warren.unit.test)
    alias(libs.plugins.rust.android)
    id("de.mannodermaus.android-junit5")
}

val repoRootPath = rootProject.projectDir.absoluteFile.parentFile.absolutePath
val changelogAssetsDirectory = "$repoRootPath/android/src/main/play/release-notes/"
val rustJniLibsDir = layout.buildDirectory.dir("rustJniLibs/android").get()

val appVersion = appVersionProvider.get()

android {
    namespace = "com.warrenbrowse.vpn"
    compileSdk = libs.versions.compile.sdk.major.get().toInt()
    compileSdkMinor = libs.versions.compile.sdk.minor.get().toInt()
    buildToolsVersion = libs.versions.build.tools.get()
    ndkVersion = libs.versions.ndk.get()

    defaultConfig {
        applicationId = "com.warrenbrowse.vpn"
        minSdk = libs.versions.min.sdk.get().toInt()
        targetSdk = libs.versions.target.sdk.get().toInt()
        versionCode = appVersion.code
        versionName = appVersion.name
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        lint {
            lintConfig = file("${rootProject.projectDir}/config/lint.xml")
            baseline = file("${rootProject.projectDir}/config/lint-baseline.xml")
            abortOnError = true
            checkAllWarnings = true
            warningsAsErrors = true
            checkDependencies = true
        }
    }

    playConfigs {
        // BILLING flavor collapsed - variant is now `prodRelease`.
        register("prodRelease") {
            enabled = !appVersion.isDev
            releaseStatus.set(ReleaseStatus.DRAFT)
            track.set(
                when {
                    appVersion.isStable -> "production"
                    appVersion.isBeta -> "beta"
                    else -> "internal"
                }
            )
        }
    }

    androidResources {
        @Suppress("UnstableApiUsage")
        // Due to a bug in the Android platform we need to disable this as the auto-generated local
        // config causes a crash on some versions of android.
        // See: https://issuetracker.google.com/issues/399131926#comment29
        // Restoring this behavior when the issue has been resolved is tracked in: DROID-2163
        generateLocaleConfig = false
    }

    // Release signing: opt-in via Gradle properties OR env vars.
    // The Gradle-property path is preferred (`-Pwarren.keystore.path=...`)
    // because env-var reads happen at configure time and a long-lived
    // Gradle daemon's environment is FROZEN at daemon start; a CI job
    // that exports WARREN_KEYSTORE_* after the daemon is already warm
    // will silently ship an UNSIGNED build. Gradle properties bypass
    // the daemon-env trap because the property store is invalidated
    // per-invocation. The env fallback is retained for local dev use.
    //
    // CI guidance: prefer
    //   ./gradlew --no-daemon -Pwarren.keystore.path=$X ...
    // or run `./gradlew --stop` before the signing build.
    val warrenKeystorePath: String? =
        (project.findProperty("warren.keystore.path") as String?)
            ?: System.getenv("WARREN_KEYSTORE_PATH")
    val warrenKeystorePassword: String? =
        (project.findProperty("warren.keystore.password") as String?)
            ?: System.getenv("WARREN_KEYSTORE_PASSWORD")
    val warrenKeyAlias: String? =
        (project.findProperty("warren.key.alias") as String?) ?: System.getenv("WARREN_KEY_ALIAS")
    val warrenKeyPassword: String? =
        (project.findProperty("warren.key.password") as String?)
            ?: System.getenv("WARREN_KEY_PASSWORD")
    val signingConfigured =
        warrenKeystorePath != null &&
            warrenKeystorePassword != null &&
            warrenKeyAlias != null &&
            warrenKeyPassword != null

    if (signingConfigured) {
        signingConfigs {
            create("warrenRelease") {
                storeFile = file(warrenKeystorePath!!)
                storePassword = warrenKeystorePassword
                keyAlias = warrenKeyAlias
                keyPassword = warrenKeyPassword
                // AGP defaults: v1 (jar) ON, v2 ON, v3 OFF, v4 OFF.
                // Play Store accepts v2+, so the defaults are fine.
            }
        }
    }

    buildTypes {
        getByName(BuildTypes.RELEASE) {
            signingConfig =
                if (signingConfigured) {
                    signingConfigs.getByName("warrenRelease")
                } else {
                    null
                }
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
        getByName(BuildTypes.DEBUG) { isPseudoLocalesEnabled = true }
    }

    // BILLING flavor dimension (OSS/PLAY) collapsed (Mullvad
    // Play Store in-app purchase billing dead on Warren - single Warren
    // build, no Play VPN-time subscriptions). INFRASTRUCTURE (PROD-only)
    // kept for the API endpoint override slot used by debug/staging builds.
    flavorDimensions += FlavorDimensions.INFRASTRUCTURE

    productFlavors {
        create(Flavors.PROD) {
            dimension = FlavorDimensions.INFRASTRUCTURE
            isDefault = true
            buildConfigField("String", "API_ENDPOINT", "\"\"")
            buildConfigField("String", "API_IP", "\"\"")
            // Per-flavor deep-link scheme so the beta and prod apps never
            // fight over the same URL registration on one device.
            manifestPlaceholders["warrenDeepLinkScheme"] = "warren"
            buildConfigField("String", "DEEP_LINK_SCHEME", "\"warren\"")
        }
        // Separate installable beta app (coexists with prod on one device):
        // own applicationId, own label (src/beta res overlay). The endpoint
        // that release builds actually honor is the one compiled into the
        // Rust datapath: the cargo block below exports WARREN_PRODUCT_ENV
        // matching the built flavor, guarded per-variant at execution time.
        create(Flavors.BETA) {
            dimension = FlavorDimensions.INFRASTRUCTURE
            applicationIdSuffix = ".beta"
            buildConfigField("String", "API_ENDPOINT", "\"api.beta.warrenbrowse.com\"")
            buildConfigField("String", "API_IP", "\"\"")
            manifestPlaceholders["warrenDeepLinkScheme"] = "warren-beta"
            buildConfigField("String", "DEEP_LINK_SCHEME", "\"warren-beta\"")
        }
        // Same shape as beta, against the staging backend.
        create(Flavors.STAGING) {
            dimension = FlavorDimensions.INFRASTRUCTURE
            applicationIdSuffix = ".staging"
            buildConfigField("String", "API_ENDPOINT", "\"api.staging.warrenbrowse.com\"")
            buildConfigField("String", "API_IP", "\"\"")
            manifestPlaceholders["warrenDeepLinkScheme"] = "warren-staging"
            buildConfigField("String", "DEEP_LINK_SCHEME", "\"warren-staging\"")
        }
    }

    sourceSets {
        getByName("main") { assets.directories.add(changelogAssetsDirectory) }
        // Workaround to include all instrumented tests in app module. Without this we'd have to
        // create an APK for each submodule and pass each on for testing with the orchestrator.
        getByName("androidTest") {
            val instrumentedTests =
                rootProject.subprojects
                    .mapNotNull { subProject ->
                        subProject.file("src/androidTest/kotlin").takeIf { it.exists() }
                    }
                    .map { it.absolutePath }
            kotlin.directories.addAll(instrumentedTests)
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    testOptions {
        unitTests.all { test ->
            test.testLogging {
                test.outputs.upToDateWhen { false }
                events("passed", "skipped", "failed", "standardOut", "standardError")
                showCauses = true
                showExceptions = true
                showStandardStreams = true
            }
        }
    }

    packaging {
        if (getBooleanProperty("warren.app.build.keepDebugSymbols")) {
            jniLibs.keepDebugSymbols.add("**/*.so")
        }
        jniLibs.useLegacyPackaging = true
        resources {
            pickFirsts +=
                setOf(
                    // Fixes packaging error caused by: androidx.compose.ui:ui-test-junit4
                    "META-INF/AL2.0",
                    "META-INF/LGPL2.1",
                    // Fixes packaging error caused by: jetified-junit-*
                    "META-INF/LICENSE.md",
                    "META-INF/LICENSE-notice.md",
                    "META-INF/io.netty.versions.properties",
                    "META-INF/INDEX.LIST",
                )
        }
    }
}

androidComponents {
    onVariants { variant ->
        val mainSources = variant.sources.getByName("main")
        mainSources.addStaticSourceDirectory(changelogAssetsDirectory)
    }

    onVariants {
        it.buildConfigFields!!.put(
            "ENABLE_IN_APP_VERSION_NOTIFICATIONS",
            BuildConfigField(
                "boolean",
                getBooleanProperty("warren.app.config.inAppVersionNotifications.enable"),
                "Show in-app version notifications",
            ),
        )
        // Warren fetches relays via warren-api-client at runtime, no
        // bundled JSON asset to gate on. The upstream
        // `REQUIRE_BUNDLED_RELAY_FILE` BuildConfig field was dropped.
    }
    onVariants {
        val productFlavors = it.productFlavors.toMap()
        val buildType = it.buildType

        val artifactSuffix = buildString {
            // BILLING flavor dropped, single Warren build.

            productFlavors[FlavorDimensions.INFRASTRUCTURE]?.let { infrastructureFlavorName ->
                if (infrastructureFlavorName != Flavors.PROD) {
                    append(".$infrastructureFlavorName")
                }
            }

            if (buildType != BuildTypes.RELEASE) {
                append(".${buildType}")
            }
        }

        val variantName = it.name
        val capitalizedVariantName = variantName.capitalized()
        val artifactName = "WarrenVPN-${appVersion.name}${artifactSuffix}"

        tasks.register<Copy>("create${capitalizedVariantName}DistApk") {
            from(it.artifacts.get(SingleArtifact.APK))
            into("${rootDir.parent}/dist")
            include { it.name.endsWith(".apk") }
            rename { "$artifactName.apk" }
        }

        tasks.register<Copy>("create${capitalizedVariantName}DistBundle") {
            from(it.artifacts.get(SingleArtifact.BUNDLE))
            into("${rootDir.parent}/dist")
            include { it.name.endsWith(".aab") }
            rename { "$artifactName.aab" }
        }

        tasks.findByPath("generate${capitalizedVariantName}BaselineProfile")?.let {
            val baselineFile = "baseline-prof.txt"
            val sourceDir = "${rootProject.projectDir}/app/src"
            val fromDir = "$sourceDir/$variantName/generated/baselineProfiles"
            val toDir = "$sourceDir/main"
            val fromFile = file("$fromDir/$baselineFile")
            val toFile = file("$toDir/$baselineFile")
            it.doLast { fromFile.renameTo(toFile) }
        }
    }
}

// Ensure that we have all the JNI libs before merging them.
tasks
    .matching { it.name.matches(Regex("merge.*JniLibFolders")) }
    .configureEach {
        // This is required for the merge task to run every time the .so files are updated.
        // See this comment for more information:
        // https://github.com/mozilla/rust-android-gradle/issues/118#issuecomment-1569407058
        inputs.dir(rustJniLibsDir)
        dependsOn("cargoBuild")
        // Refuse to package a .so compiled for another product environment:
        // a beta APK carrying a prod-compiled libwarren_jni.so would silently
        // talk to the prod backend (release builds have no runtime override).
        // Locals only (plain Strings): the closure must not capture the
        // build script object or the configuration cache refuses to store.
        val expectedEnv =
            when {
                name.contains("Beta") -> Flavors.BETA
                name.contains("Staging") -> Flavors.STAGING
                else -> Flavors.PROD
            }
        val actualEnv = rustProductEnv
        val taskName = name
        doFirst {
            check(actualEnv == expectedEnv) {
                "Variant '$taskName' needs WARREN_PRODUCT_ENV=$expectedEnv but cargoBuild ran " +
                    "with '$actualEnv'. Build one flavor per invocation " +
                    "(e.g. assembleBetaDebug) or pass -Pwarren.app.build.productEnv=$expectedEnv."
            }
        }
    }

configureComposeCompiler()

kotlin {
    compilerOptions {
        allWarningsAsErrors = true
        freeCompilerArgs =
            listOf(
                // Opt-in option for Koin annotation of KoinComponent.
                "-opt-in=kotlin.RequiresOptIn",
                "-XXLanguage:+WhenGuards",
            )
    }
}

junitPlatform {
    instrumentationTests {
        version.set(libs.versions.junit5.android.asProvider())
        includeExtensions.set(true)
    }
}

// Rust product environment for this invocation. Release APKs ignore every
// runtime endpoint override, so the beta flavor's endpoint MUST reach cargo
// as WARREN_PRODUCT_ENV=beta (compiled into libwarren_jni.so). The cargo
// plugin exposes ONE variant-agnostic cargoBuild task, so the environment
// derives from the requested tasks (explicit -Pwarren.app.build.productEnv
// wins); a mixed prod+beta invocation cannot be represented in a single
// cargoBuild and is refused by the per-variant guard on the JNI merge tasks.
val rustProductEnv: String = run {
    val explicit = project.findProperty("warren.app.build.productEnv") as String?
    val requested = gradle.startParameter.taskNames
    val requestedEnvs =
        listOf(Flavors.PROD to "Prod", Flavors.BETA to "Beta", Flavors.STAGING to "Staging")
            .filter { (_, marker) -> requested.any { task -> task.contains(marker) } }
            .map { (env, _) -> env }
    // A mixed invocation names no single environment, so it falls back to prod
    // and the per-variant guard on the JNI merge tasks refuses the non-prod
    // half rather than packaging a .so built for the wrong backend.
    explicit ?: requestedEnvs.singleOrNull() ?: Flavors.PROD
}

cargo {
    val isReleaseBuild = isReleaseBuild()
    val generateDebugSymbolsForReleaseBuilds =
        getBooleanProperty("warren.app.build.cargo.generateDebugSymbolsForReleaseBuilds")
    val enableApiOverride = !isReleaseBuild || appVersion.isDev || appVersion.isAlpha
    environmentalOverrides["WARREN_PRODUCT_ENV"] = rustProductEnv
    module = repoRootPath
    libname = "warren-jni"
    // The rust-android-gradle linker wrapper shells out to this command; default
    // "python" no longer exists on modern macOS/CI (only "python3"), which makes
    // the Android native link fail with exit 127. The wrapper script is python3
    // compatible, so pin python3 explicitly.
    pythonCommand = "python3"
    // All available targets:
    // https://github.com/mozilla/rust-android-gradle/tree/master?tab=readme-ov-file#targets
    targets = getStringListProperty("warren.app.build.cargo.targets")
    profile =
        if (isReleaseBuild) {
            if (generateDebugSymbolsForReleaseBuilds) "release-debuginfo" else "release"
        } else {
            "debug"
        }
    targetDirectory = "$repoRootPath/target"
    features {
        val enabledFeatures =
            buildList {
                    if (enableApiOverride) {
                        add("api-override")
                    }
                }
                .toTypedArray()

        @Suppress("SpreadOperator") defaultAnd(*enabledFeatures)
    }
    targetIncludes = arrayOf("libwarren_jni.so")
    extraCargoBuildArguments = buildList {
        add("--package=warren-jni")
        add("--locked")
    }

    if (getBooleanProperty("warren.app.build.replaceRustPathPrefix")) {
        environmentalOverrides["RUSTFLAGS"] = generateRemapArguments()
    }
}

// Every flavor writes its `libwarren_jni.so` into ONE shared directory, and
// cargo only rewrites the ABIs it was asked for. An invocation that builds a
// subset (-Pwarren.app.build.cargo.targets=arm64) therefore packages the
// PREVIOUS environment's datapath for every other ABI, silently talking to
// another backend. The merge guard cannot catch it: it compares the cargo
// invocation with the variant, not the files on disk. So drop the directory
// whenever the environment it holds is not the one being built. No declared
// outputs: this must run on every build, including the ones where cargoBuild
// itself is up to date.
val dropForeignRustJniLibs =
    tasks.register("dropForeignRustJniLibs") {
        // Locals only (plain File/String): capturing the build script object
        // makes the configuration cache refuse to store the entry.
        val jniLibsDir = rustJniLibsDir.asFile
        val stamp = File(jniLibsDir.parentFile, "warren-product-env.txt")
        val env = rustProductEnv
        doLast {
            if (!stamp.isFile || stamp.readText().trim() != env) {
                jniLibsDir.deleteRecursively()
                stamp.parentFile.mkdirs()
                stamp.writeText(env)
            }
        }
    }

tasks.matching { it.name == "cargoBuild" }.configureEach { dependsOn(dropForeignRustJniLibs) }

tasks.register<Exec>("cargoClean") {
    workingDir = File(repoRootPath)
    commandLine("cargo", "clean")
}

if (getBooleanProperty("warren.app.build.cargo.cleanBuild")) {
    tasks["clean"].dependsOn("cargoClean")
}

baselineProfile { warnings { disabledVariants = false } }

androidComponents {
    beforeVariants { variantBuilder ->
        variantBuilder.enable =
            Variant(variantBuilder.buildType, variantBuilder.productFlavors)
                .matchesAny(prodDebugReleaseVariants, baselineFilter)
    }
}

tasks.register("printVersion") {
    val versionCode = project.android.defaultConfig.versionCode
    val versionName = project.android.defaultConfig.versionName
    doLast {
        println("versionCode=$versionCode")
        println("versionName=$versionName")
    }
}

tasks.register("debug") { dependsOn("assembleOssProdDebug") }

tasks.register("debugPlay") { dependsOn("assemblePlayProdDebug") }

registerReleaseTask(
    "fdroidRelease",
    appVersion,
    listOf("createOssProdReleaseDistApk"),
    skipClean = true,
    skipDirtyCheck = true,
)

registerReleaseTask("fullRelease", appVersion, fullReleaseTasks(appVersion))

play {
    System.getenv("PLAY_CREDENTIALS_PATH")?.let { serviceAccountCredentials.set(file(it)) }
    // Disable for all flavors by default. Only specific flavors should be enabled using
    // PlayConfigs.
    enabled = false
    // This property refers to the Publishing API (not git).
    commit = true
    defaultToAppBundles = true
    track = "internal"
    releaseStatus = ReleaseStatus.COMPLETED
    userFraction = 1.0
}

dependencies {
    implementation(projects.lib.common)
    implementation(project(":lib:common-compose"))
    // lib.grpc dropped (Mullvad daemon gRPC bridge dead).
    implementation(projects.lib.endpoint)
    // account/addtime/deleteaccount/managedevices/redeemvoucher
    // modules deleted (Mullvad-account features without Warren equivalent).
    // anticensorship module deleted.
    // apiaccess module deleted.
    // feature.appicon dropped (Mullvad obfuscation dead).
    implementation(projects.lib.feature.appinfo.impl)
    implementation(projects.lib.feature.appinfo.api)
    implementation(projects.lib.feature.applisting.impl)
    implementation(projects.lib.feature.applisting.api)
    implementation(projects.lib.feature.autoconnect.impl)
    // customlist + filter + location modules deleted
    // (Mullvad relay-list picker, replaced by WarrenLocationPicker).
    // daita module deleted (DAITA via WarrenTunnelSettings).
    implementation(projects.lib.feature.home.impl)
    implementation(projects.lib.feature.home.api)
    implementation(projects.lib.feature.language.impl)
    implementation(projects.lib.feature.login.impl)
    implementation(projects.lib.feature.login.api)
    // multihop module deleted (multi-hop via WarrenTunnelSettings).
    implementation(projects.lib.feature.notification.impl)
    // serveripoverride module deleted (Warren exit fleet
    // is sovereign, no per-relay IP overrides).
    implementation(projects.lib.feature.settings.impl)
    implementation(projects.lib.feature.settings.api)
    implementation(projects.lib.feature.splittunneling.impl)
    // feature.vpnsettings module deleted (Mullvad daemon
    // settings sync dead - Warren-native settings live in feature.settings).
    implementation(projects.lib.map)
    implementation(projects.lib.model)
    implementation(projects.lib.pushNotification)
    implementation(projects.lib.navigation)
    // lib.payment + lib.billing dropped (Mullvad Play Store
    // billing dead on Warren - BIP39 wallet replaces VPN subscriptions).
    implementation(projects.lib.repository)
    implementation(projects.lib.talpid)
    implementation(projects.lib.tv)
    implementation(projects.lib.ui.designsystem)
    implementation(projects.lib.ui.component)
    implementation(projects.lib.ui.icon)
    implementation(projects.lib.ui.resource)
    implementation(projects.lib.ui.tag)
    implementation(projects.lib.ui.theme)
    implementation(projects.lib.ui.util)
    implementation(projects.lib.usecase)
    implementation(libs.androidx.profileinstaller)
    implementation(libs.androidx.navigation3.ui)

    // Baseline profile
    baselineProfile(projects.test.baselineprofile)

    // playImplementation(projects.lib.billing) dropped.

    // This dependency can be replaced when minimum SDK is 29 or higher.
    // It can then be replaced with InetAddress.isNumericAddress
    implementation(libs.commons.validator) {
        // This dependency has a known vulnerability
        // https://osv.dev/vulnerability/GHSA-wxr5-93ph-8wr9
        // It is not used so let's exclude it.
        // Unfortunately, this is not possible to do using libs.version.toml
        // https://github.com/gradle/gradle/issues/26367#issuecomment-2120830998
        exclude("commons-beanutils", "commons-beanutils")
    }
    implementation(libs.accompanist.permissions)
    implementation(libs.androidx.activity.compose)
    // FragmentActivity baseline for MainActivity (BiometricPrompt host).
    implementation(libs.androidx.fragment)
    implementation(libs.androidx.datastore)
    implementation(libs.androidx.coresplashscreen)
    implementation(libs.androidx.credentials) {
        // This dependency adds a lot of unused permissions to the app.
        // It is not used so let's exclude it.
        // Unfortunately, this is not possible to do using libs.version.toml
        // https://github.com/gradle/gradle/issues/26367#issuecomment-2120830998
        exclude("androidx.biometric", "biometric")
    }
    implementation(libs.androidx.ktx)
    implementation(libs.androidx.lifecycle.runtime)
    implementation(libs.androidx.lifecycle.viewmodel)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.metrics.performance)
    implementation(libs.androidx.tv)
    implementation(libs.androidx.work.runtime.ktx)
    implementation(libs.arrow)
    implementation(libs.arrow.optics)
    implementation(libs.arrow.resilience)
    implementation(libs.compose.constrainlayout)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.compose.icons.extended)
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.util)

    implementation(libs.kermit)
    implementation(libs.koin)
    implementation(libs.koin.android)
    implementation(libs.koin.compose)
    implementation(libs.kotlin.reflect)
    implementation(libs.kotlin.stdlib)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.protobuf.kotlin.lite)

    // UI tooling
    implementation(libs.compose.ui.tooling.preview)
    debugImplementation(libs.compose.ui.tooling)

    // HACK:
    // Not used by app module, but otherwise an older version pre 1.8.0 will be used at runtime for
    // the e2e tests. This causes the deserialization to fail because of a missing function that was
    // introduced in 1.8.0.
    implementation(libs.kotlinx.serialization.json)

    // UI test dependencies

    // Needed for createComposeExtension() and createAndroidComposeExtension()
    debugImplementation(libs.compose.ui.test.manifest)
    androidTestImplementation(libs.koin.test)
    androidTestImplementation(libs.kotlin.test)
    androidTestImplementation(libs.mockk.android)
    androidTestImplementation(libs.turbine)
    androidTestImplementation(libs.junit.jupiter.api)
    androidTestImplementation(libs.junit5.android.test.compose)
    androidTestImplementation(libs.androidx.espresso)
    androidTestImplementation(projects.lib.screenTest)
}
