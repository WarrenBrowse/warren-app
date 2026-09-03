package com.warrenbrowse.vpn.screen.onboarding

import com.warrenbrowse.vpn.core.NavKey2
import com.warrenbrowse.vpn.core.Navigator
import com.warrenbrowse.vpn.feature.home.api.ConnectNavKey
import com.warrenbrowse.vpn.feature.login.api.WarrenWalletNavKey
import com.warrenbrowse.vpn.lib.repository.WarrenLocalSettingsRepository
import com.warrenbrowse.vpn.screen.navigation.OnboardingBetaAccessNavKey
import com.warrenbrowse.vpn.screen.navigation.OnboardingSubscriptionNavKey

/**
 * Forward hop from the welcome step into wallet creation.
 *
 * Deliberately writes nothing and keeps the welcome step on the stack: the wizard is only
 * "completed" at one of its real exits (see [leaveWizard]). Stamping the flag here would send an
 * interrupted first run straight past the funding step on the next launch, landing an unfunded
 * wallet on Connect.
 *
 * A replay from the settings root runs with the wallet already on the device. Desktop's wizard
 * never creates or imports in that case (`OnboardingWalletView` only re-shows the phrase), so the
 * hop skips straight to the step after the wallet: offering "Create a new account" over a funded
 * identity is how a replay would overwrite it.
 */
internal fun enterWalletStep(
    navigator: Navigator,
    walletPresent: Boolean = false,
    isBeta: Boolean = false,
) {
    val next: NavKey2 =
        when {
            !walletPresent -> WarrenWalletNavKey(onboarding = true)
            isBeta -> OnboardingBetaAccessNavKey
            else -> OnboardingSubscriptionNavKey
        }
    navigator.navigate(next)
}

/**
 * Where the welcome step's skip link lands.
 *
 * With no wallet the skip still routes through wallet creation, because the wallet IS the identity
 * on Android: what is skipped is the guided funding and preferences steps.
 *
 * With a wallet already on the device (every replay from the settings root) that screen is a dead
 * end: it leaves on the transition into `WalletState.Ready`, which happened long ago, so its only
 * remaining exit is the settings cogwheel, and Connect is unreachable until the app is killed.
 */
internal fun skipDestination(walletPresent: Boolean): NavKey2 =
    if (walletPresent) ConnectNavKey else WarrenWalletNavKey(onboarding = false)

/**
 * The wizard's real exits: the terminal Done CTA and every skip link. Stamps the completed flag,
 * then roots [destination] so the wizard is not walkable back into (desktop `OnboardingLayout.skip`
 * / `OnboardingDoneView`).
 */
internal fun leaveWizard(
    settings: WarrenLocalSettingsRepository,
    navigator: Navigator,
    destination: NavKey2,
) {
    settings.setOnboardingCompleted(true)
    navigator.navigate(destination, clearBackStack = true)
}

/**
 * One-shot forward guard for the funding step, mirroring the desktop `navigatedRef`: the step
 * advances the moment credit lands, whatever credited it, and never twice. The outgoing screen
 * stays composed during the transition, so an unguarded effect would push the next step a second
 * time.
 *
 * [alreadyAdvanced] is held by the caller in saveable state, because the step stays on the back
 * stack: without it, walking back into a funded step would throw the user straight forward again
 * and turn the back chevron into a trap.
 *
 * [held] suspends the advance while a modal the user is still reading is up (the voucher success
 * confirmation).
 */
internal fun shouldAdvanceFromFundingStep(
    alreadyAdvanced: Boolean,
    funded: Boolean,
    held: Boolean = false,
): Boolean = !alreadyAdvanced && funded && !held
