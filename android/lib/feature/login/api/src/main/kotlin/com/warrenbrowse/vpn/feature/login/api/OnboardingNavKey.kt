package com.warrenbrowse.vpn.feature.login.api

import com.warrenbrowse.vpn.core.NavKey2
import kotlinx.parcelize.Parcelize

/**
 * The first-launch wizard's welcome step. Lives in the login api module rather than the app's
 * navigation package so the settings root can re-enter the wizard ("Replay onboarding", desktop
 * `ReplayOnboardingListItem`).
 */
@Parcelize object OnboardingNavKey : NavKey2
