package com.warrenbrowse.vpn.test.baselineprofile

import androidx.benchmark.macro.junit4.BaselineProfileRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import androidx.test.uiautomator.waitForStableInActiveWindow
import com.warrenbrowse.vpn.lib.ui.tag.CONNECT_CARD_HEADER_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.SELECT_LOCATION_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.TOP_BAR_SETTINGS_BUTTON_TEST_TAG
import com.warrenbrowse.vpn.lib.ui.tag.VPN_SETTINGS_CELL_TEST_TAG
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Generates the baseline profile for the Warren app, on Warren's own flow:
 * the cold start through the splash to the Connect screen, the location
 * picker, and the settings tree. Run it from gradle with
 * `./gradlew :app:generateBetaNonMinifiedReleaseBaselineProfile` (or the
 * prod variant); the profile lands in `app/src/main/baseline-prof.txt`.
 *
 * The device needs a wallet set up first, so the splash routes to Connect:
 * the runner clears the package data, so the generator walks the privacy
 * disclosure and the onboarding wizard to a fresh wallet on its first
 * iteration and the existing one after. No tunnel is dialled: the profile is
 * about the classes the UI loads, and a real dial would need a subscription
 * and a network. Regenerate it whenever the startup path or the main
 * screens change substantially.
 *
 * NOTE: API 33+ or rooted API 28+ is required.
 */
@RunWith(AndroidJUnit4::class)
@LargeTest
class BaselineProfileGenerator {

    @get:Rule val rule = BaselineProfileRule()

    @Test
    fun generate() {
        rule.collect(
            packageName =
                InstrumentationRegistry.getArguments().getString("targetAppId")
                    ?: error("targetAppId not passed as instrumentation runner arg"),
            // See:
            // https://d.android.com/topic/performance/baselineprofiles/dex-layout-optimizations
            includeInStartupProfile = true,
            maxIterations = 5,
        ) {
            pressHome()
            startActivityAndWait()
            device.reachConnectScreen()

            // The picker: the accordion, its search field and its rows.
            device.findObject(By.res(SELECT_LOCATION_BUTTON_TEST_TAG)).click()
            device.wait(Until.hasObject(By.text("Select location")), UI_TIMEOUT_MS)
            device.waitForStableInActiveWindow()
            device.pressBack()
            device.wait(Until.hasObject(By.res(CONNECT_CARD_HEADER_TEST_TAG)), UI_TIMEOUT_MS)

            // Settings, then the VPN settings list, and back to Connect.
            device.findObject(By.res(TOP_BAR_SETTINGS_BUTTON_TEST_TAG)).click()
            device.wait(Until.hasObject(By.res(VPN_SETTINGS_CELL_TEST_TAG)), UI_TIMEOUT_MS)
            device.findObject(By.res(VPN_SETTINGS_CELL_TEST_TAG)).click()
            device.waitForStableInActiveWindow()
            device.pressBack()
            device.waitForStableInActiveWindow()
            device.pressBack()
            device.wait(Until.hasObject(By.res(CONNECT_CARD_HEADER_TEST_TAG)), UI_TIMEOUT_MS)
        }
    }

    /**
     * Walk whatever the splash routed to until the Connect screen is up: the
     * privacy disclosure and the onboarding wizard on a fresh install (the
     * wizard creates the wallet), nothing when a wallet already exists.
     */
    private fun UiDevice.reachConnectScreen() {
        repeat(WIZARD_STEPS_MAX) {
            if (hasObject(By.res(CONNECT_CARD_HEADER_TEST_TAG))) return
            waitForStableInActiveWindow()
            val next =
                WIZARD_BUTTONS.firstNotNullOfOrNull { text -> findObject(By.text(text)) }
                    ?: return@repeat
            next.click()
            wait(Until.hasObject(By.res(CONNECT_CARD_HEADER_TEST_TAG)), STEP_TIMEOUT_MS)
        }
        if (!wait(Until.hasObject(By.res(CONNECT_CARD_HEADER_TEST_TAG)), UI_TIMEOUT_MS)) {
            error("the Connect screen was not reached; the wizard copy may have changed")
        }
    }

    private companion object {
        const val UI_TIMEOUT_MS = 10_000L
        const val STEP_TIMEOUT_MS = 2_000L
        const val WIZARD_STEPS_MAX = 12

        /**
         * The forward controls of the privacy disclosure and the onboarding
         * wizard (`strings.xml`: `agree_and_continue`, `onboarding_get_started`,
         * `wallet_create_cta`, `wallet_backup_confirm_cta`, `onboarding_skip`,
         * `onboarding_done_cta`), in the order a step may show more than one of
         * them. The backup step's checkbox text ticks the box, which enables
         * its button on the next pass.
         */
        val WIZARD_BUTTONS =
            listOf(
                "Agree and continue",
                "Get started",
                "Create a new account",
                "I have written down my recovery phrase in a safe place.",
                "Skip for now",
                "Connect",
            )
    }
}
