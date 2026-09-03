package com.warrenbrowse.vpn.app

import android.content.Intent
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.fragment.app.FragmentActivity
import androidx.core.util.Consumer
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import androidx.metrics.performance.JankStats
import arrow.core.merge
import co.touchlab.kermit.Logger
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import com.warrenbrowse.vpn.app.forum.ForumDigestPoller
import com.warrenbrowse.vpn.app.forum.forumDigestWanted
import com.warrenbrowse.vpn.app.forum.ForumEvent
import com.warrenbrowse.vpn.app.forum.ForumJournal
import com.warrenbrowse.vpn.app.forum.JournalField
import com.warrenbrowse.vpn.app.forum.LinkSource
import com.warrenbrowse.vpn.app.forum.ForumLinkVerdict
import com.warrenbrowse.vpn.app.forum.ForumLoginController
import com.warrenbrowse.vpn.app.forum.ForumLoginPromptHost
import com.warrenbrowse.vpn.app.forum.classifyForumLoginLink
import com.warrenbrowse.vpn.app.perf.JankLogger
import com.warrenbrowse.vpn.di.uiModule
import com.warrenbrowse.vpn.lib.common.constant.KEY_OPEN_FORUM_ACTIVITY
import com.warrenbrowse.vpn.lib.common.constant.KEY_REQUEST_VPN_PROFILE
import com.warrenbrowse.vpn.lib.common.util.CreateVpnProfile
import com.warrenbrowse.vpn.lib.common.util.prepareVpnSafe
import com.warrenbrowse.vpn.lib.endpoint.ApiEndpointFromIntentHolder
import com.warrenbrowse.vpn.lib.endpoint.getApiEndpointConfigurationExtras
import com.warrenbrowse.vpn.lib.model.PrepareError
import com.warrenbrowse.vpn.lib.model.Prepared
import com.warrenbrowse.vpn.lib.repository.ForumActivityOpenRequests
import com.warrenbrowse.vpn.lib.repository.ForumIdentityRepository
import com.warrenbrowse.vpn.lib.repository.SplashCompleteRepository
import com.warrenbrowse.vpn.lib.repository.UserPreferencesRepository
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.lib.repository.WarrenQuinnConnectInvoker
import com.warrenbrowse.vpn.lib.ui.theme.AppTheme
import com.warrenbrowse.vpn.serviceconnection.ServiceConnectionManager
import com.warrenbrowse.vpn.serviceconnection.ServiceConnectionState
import org.koin.android.ext.android.inject
import org.koin.android.scope.AndroidScopeComponent
import org.koin.androidx.scope.activityScope
import org.koin.core.context.loadKoinModules

// `FragmentActivity` rather than `ComponentActivity`: `BiometricPrompt`
// (used by `BiometricPromptAuthorizer` for D.5 wallet unlock) requires a
// FragmentActivity host. FragmentActivity extends ComponentActivity, so
// this is a compatible upgrade for all existing Compose plumbing.
class MainActivity : FragmentActivity(), AndroidScopeComponent {
    override val scope by activityScope()

    private val launchVpnPermission =
        registerForActivityResult(CreateVpnProfile()) { _ -> dispatchWarrenConnect() }

    private val apiEndpointFromIntentHolder by inject<ApiEndpointFromIntentHolder>()
    private val warrenAppViewModel by inject<WarrenAppViewModel>()
    private val userPreferencesRepository by inject<UserPreferencesRepository>()
    private val serviceConnectionManager by inject<ServiceConnectionManager>()
    private val splashCompleteRepository by inject<SplashCompleteRepository>()
    private val warrenConnect by inject<WarrenQuinnConnectInvoker>()
    private val forumLoginController by inject<ForumLoginController>()
    private val forumEventsJournal by inject<ForumJournal>()
    private val forumDigestPoller by inject<ForumDigestPoller>()
    private val forumActivityOpenRequests by inject<ForumActivityOpenRequests>()
    private val localSettings by inject<WarrenLocalSettingsRepository>()
    private val forumIdentityRepository by inject<ForumIdentityRepository>()

    private fun dispatchWarrenConnect() {
        // Route the post-VPN-profile-grant connect (and the already-prepared
        // `Prepared` branch in handleRequestVpnProfileIntent) through the
        // Warren Quinn use-case.
        lifecycleScope.launch {
            runCatching { warrenConnect.connect(this@MainActivity) }
                .onFailure { e -> Logger.e(throwable = e) { "Warren connect dispatch failed" } }
        }
    }

    private var isReadyNextDraw: Boolean = false

    // Frame deadline accounting for the whole window, folded into one logcat
    // line per janky second (`adb logcat -s WarrenJank`). On device only, in
    // every build: Warren keeps no telemetry, and a release build is the one
    // whose frames are worth reading. Held so it lives as long as the window.
    private var jankStats: JankStats? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        loadKoinModules(listOf(uiModule))

        lifecycle.addObserver(warrenAppViewModel)

        installSplashScreen().setKeepOnScreenCondition {
            val isReady = isReadyNextDraw
            isReadyNextDraw = splashCompleteRepository.isSplashComplete()
            !isReady
        }

        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
        )

        super.onCreate(savedInstanceState)

        setContent {
            AppTheme {
                WarrenApp(serviceConnectionManager)
                // Overlay: shows the forum-login consent prompt when a
                // `warren://forum-login` deep link has been captured below.
                ForumLoginPromptHost()
            }
        }
        val jankLogger = JankLogger()
        jankStats =
            JankStats.createAndTrack(window) { frame ->
                val endNanos = frame.frameStartNanos + frame.frameDurationUiNanos
                jankLogger
                    .onFrame(endNanos, frame.frameDurationUiNanos, frame.isJank)
                    ?.let { line -> Log.w(JANK_LOG_TAG, line) }
            }

        // This is to protect against tapjacking attacks
        // This is applied at an OS level since Android 12 so it is only required on older versions
        // of Android
        // See https://developer.android.com/privacy-and-security/risks/tapjacking for more
        // information
        if (Build.VERSION.SDK_INT <= Build.VERSION_CODES.R) {
            window.decorView.rootView.filterTouchesWhenObscured = true
        }

        // Needs to be before we start the service, since we need to access the intent there
        lifecycleScope.launch { intents().collect(::handleIntent) }

        // We use lifecycleScope here to get less start service in background exceptions
        // Se this article for more information:
        // https://medium.com/@lepicekmichal/android-background-service-without-hiccup-501e4479110f
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                if (userPreferencesRepository.preferences().isPrivacyDisclosureAccepted) {
                    bindService()
                }
            }
        }

        // The forum digest poll lives exactly as long as the window is visible
        // and the digest has a reader (`forumDigestWanted`): a fetch on every
        // start (what the app missed while away), then one a minute, cancelled
        // at stop. Nothing polls in the background, so the badge never turns
        // the app into a periodic presence signal.
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                forumDigestPoller.runWhile(
                    forumDigestWanted(
                        userPreferencesRepository.preferencesFlow().map {
                            it.isPrivacyDisclosureAccepted
                        },
                        localSettings.forumNotificationsEnabled,
                        forumIdentityRepository.identity,
                    )
                )
            }
        }
    }

    override fun onRestoreInstanceState(savedInstanceState: Bundle) {
        super.onRestoreInstanceState(savedInstanceState)
        // Splash dismissal is unconditional: the splash decision tree in
        // `SplashViewModel` already routes the user to the right destination
        // before the splash completes.
        lifecycleScope.launch { splashCompleteRepository.onSplashCompleted() }
    }

    fun bindService() {
        serviceConnectionManager.bind()
    }

    override fun onStop() {
        super.onStop()
        if (serviceConnectionManager.connectionState.value == ServiceConnectionState.Bound) {
            serviceConnectionManager.unbind()
        }
    }

    override fun onDestroy() {
        lifecycle.removeObserver(warrenAppViewModel)
        jankStats?.isTrackingEnabled = false
        jankStats = null
        super.onDestroy()
    }

    private fun handleIntent(intent: Intent) {
        when (val action = intent.action) {
            Intent.ACTION_MAIN ->
                apiEndpointFromIntentHolder.setApiEndpointOverride(
                    intent.getApiEndpointConfigurationExtras()
                )
            KEY_REQUEST_VPN_PROFILE -> handleRequestVpnProfileIntent()
            // The forum notification was tapped: the panel opens once the main
            // flow is on screen (the navigator may not exist yet on a cold start).
            KEY_OPEN_FORUM_ACTIVITY -> forumActivityOpenRequests.request()
            Intent.ACTION_VIEW -> handleForumDeepLink(intent)
            else -> Logger.w("Unhandled intent action: $action")
        }
    }

    /**
     * A `warren://forum-login` deep link: validate + stash it; the consent
     * prompt (never a silent sign-in) reads it and signs on approval. A
     * rejected link is logged by class (scheme, action, sid shape, host), never
     * by value: the class is the fact a report needs to show a broker/app
     * drift, and it used to be dropped in silence.
     */
    private fun handleForumDeepLink(intent: Intent) {
        val coldStart = !forumLoginController.hasSeenAnyLink()
        when (val verdict = classifyForumLoginLink(intent.dataString)) {
            is ForumLinkVerdict.Accepted -> {
                forumEventsJournal.record(
                    ForumEvent.LINK_RECEIVED,
                    JournalField.Verdict("accepted"),
                    JournalField.Source(LinkSource.DEEP_LINK),
                    JournalField.CrossDevice(verdict.link.crossDevice),
                    JournalField.ColdStart(coldStart),
                    JournalField.Referrer(referrer?.host),
                )
                forumLoginController.request(verdict.link)
            }
            is ForumLinkVerdict.Rejected -> {
                Logger.w("Ignoring a deep link the forum flow does not accept: ${verdict.reason}")
                forumEventsJournal.record(ForumEvent.LINK_RECEIVED, JournalField.Verdict(verdict.reason))
            }
        }
    }

    private fun handleRequestVpnProfileIntent() {
        val prepareResult = prepareVpnSafe().merge()
        when (prepareResult) {
            is PrepareError.NotPrepared -> launchVpnPermission.launch(prepareResult.prepareIntent)
            // If legacy or other always on connect at let daemon generate a error state
            is PrepareError.OtherLegacyAlwaysOnVpn,
            is PrepareError.OtherAlwaysOnApp,
            Prepared -> dispatchWarrenConnect()
        }
    }

    private fun ComponentActivity.intents() =
        callbackFlow<Intent> {
            send(intent)
            // The launching intent is consumed once. A rotation or a low-memory
            // recreation re-delivers it otherwise, and a `forum-login` link that
            // was already approved (or expired) came back as a fresh prompt whose
            // approval could only meet "unknown session".
            if (intent.action == Intent.ACTION_VIEW) {
                setIntent(Intent(Intent.ACTION_MAIN))
            }

            val listener = Consumer<Intent> { intent -> trySend(intent) }

            addOnNewIntentListener(listener)

            awaitClose { removeOnNewIntentListener(listener) }
        }

    private companion object {
        const val JANK_LOG_TAG = "WarrenJank"
    }
}
