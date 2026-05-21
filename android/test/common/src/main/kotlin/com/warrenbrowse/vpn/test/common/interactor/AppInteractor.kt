package com.warrenbrowse.vpn.test.common.interactor

import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import co.touchlab.kermit.Logger
import java.io.File
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.cancel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointOverride
import com.warrenbrowse.vpn.lib.endpoint.putApiEndpointConfigurationExtra
import com.warrenbrowse.vpn.lib.grpc.ManagementService
import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.IpVersion
import com.warrenbrowse.vpn.lib.model.ObfuscationMode
import com.warrenbrowse.vpn.lib.model.Port
import com.warrenbrowse.vpn.lib.model.QuantumResistantState
import com.warrenbrowse.vpn.test.common.constant.DEFAULT_TIMEOUT
import com.warrenbrowse.vpn.test.common.constant.LONG_TIMEOUT
import com.warrenbrowse.vpn.test.common.extension.findObjectWithTimeout
import com.warrenbrowse.vpn.test.common.page.LoginPage
import com.warrenbrowse.vpn.test.common.page.PrivacyPage
import com.warrenbrowse.vpn.test.common.page.on

class AppInteractor(
    private val device: UiDevice,
    private val targetContext: Context,
    private val customApiEndpointConfiguration: ApiEndpointOverride? = null,
) {
    fun launch() {
        device.pressHome()
        // Wait for launcher
        device.wait(Until.hasObject(By.pkg(device.launcherPackageName).depth(0)), LONG_TIMEOUT)

        val targetPackageName = targetContext.packageName
        val intent =
            targetContext.packageManager.getLaunchIntentForPackage(targetPackageName)?.apply {
                // Clear out any previous instances
                addFlags(Intent.FLAG_ACTIVITY_CLEAR_TASK)
                if (customApiEndpointConfiguration != null) {
                    putApiEndpointConfigurationExtra(customApiEndpointConfiguration)
                }
            }
        targetContext.startActivity(intent)
        device.wait(Until.hasObject(By.pkg(targetPackageName).depth(0)), LONG_TIMEOUT)
    }

    fun launchAndEnsureOnLoginPage() {
        launch()
        on<PrivacyPage> { clickAgreeOnPrivacyDisclaimer() }
        clickAllowOnNotificationPermissionPromptIfApiLevel33AndAbove()
        on<LoginPage>()
    }

    fun launchAndLogIn(accountNumber: String) {
        launchAndEnsureOnLoginPage()
        on<LoginPage> {
            enterAccountNumber(accountNumber)
            clickLoginButton()
        }
    }

    fun clickAllowOnNotificationPermissionPromptIfApiLevel33AndAbove(
        timeout: Long = DEFAULT_TIMEOUT
    ) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            // Skipping as notification permissions are not shown.
            return
        }

        val selector = By.text("Allow")

        device.wait(Until.hasObject(selector), timeout)

        try {
            device.findObjectWithTimeout(selector).click()
        } catch (e: IllegalArgumentException) {
            Logger.e("Failed to allow notification permission within timeout ($timeout ms)", e)
        }
    }

    suspend fun applySettings(
        pq: QuantumResistantState? = null,
        obfuscationMode: ObfuscationMode? = null,
        wireguardPort: Constraint<Port>? = null,
        localNetworkSharing: Boolean? = null,
        daita: DaitaOption? = null,
        multihop: Boolean? = null,
        deviceIpVersion: Constraint<IpVersion>? = null,
    ) = coroutineScope {
        try {
            val job = launch {
                val socket =
                    File(
                        InstrumentationRegistry.getInstrumentation().targetContext.noBackupFilesDir,
                        "rpc-socket",
                    )
                val service = ManagementService(socket, false, this)

                pq?.let { service.setWireguardQuantumResistant(it) }
                obfuscationMode?.let { service.setObfuscation(it) }
                wireguardPort?.let { service.setWireguardObfuscationPort(wireguardPort) }
                localNetworkSharing?.let { service.setAllowLan(it) }
                multihop?.let { service.setMultihop(it) }
                deviceIpVersion?.let { service.setDeviceIpVersion(deviceIpVersion) }
                daita?.let {
                    when (it) {
                        is DaitaOption.Auto -> service.setDaitaEnabled(it.enabled)
                        is DaitaOption.DirectOnly -> service.setDaitaDirectOnly(it.enabled)
                    }
                }
                cancel()
            }
            job.join()
        } catch (_: CancellationException) {
            // Ignore cancel, we have just stopped ManagementService
        }
    }
}

sealed interface DaitaOption {
    data class Auto(val enabled: Boolean) : DaitaOption

    data class DirectOnly(val enabled: Boolean) : DaitaOption
}
