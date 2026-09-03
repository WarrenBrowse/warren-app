package utilities

import org.gradle.api.Project
import org.gradle.kotlin.dsl.configure
import org.jetbrains.kotlin.compose.compiler.gradle.ComposeCompilerGradlePluginExtension

/**
 * The Compose compiler options every module with composables shares.
 *
 * The stability configuration declares the model classes composables take as
 * parameters that the compiler cannot prove stable on its own (a `List`
 * field, an external class), so a composable taking a `TunnelState` skips
 * when the state has not changed instead of being restartable only.
 *
 * The reports and metrics are opt-in (`-Pwarren.app.build.compose.reports=true`):
 * they land in `build/compose_reports` and `build/compose_metrics` of each
 * module, where `*-classes.txt` names every unstable class and
 * `*-composables.txt` every composable that is restartable but not
 * skippable. Off by default because they are an input of the build cache key.
 */
fun Project.configureComposeCompiler() {
    extensions.configure<ComposeCompilerGradlePluginExtension> {
        stabilityConfigurationFiles.add(
            rootProject.layout.projectDirectory.file("compose_compiler_config.conf")
        )
        if (getBooleanProperty("warren.app.build.compose.reports")) {
            reportsDestination.set(layout.buildDirectory.dir("compose_reports"))
            metricsDestination.set(layout.buildDirectory.dir("compose_metrics"))
        }
    }
}
