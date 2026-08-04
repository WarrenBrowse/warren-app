//
//  OnboardingWizardView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  SwiftUI step views for the first-launch onboarding wizard, mirroring
//  the desktop app (`views/onboarding/*`): Welcome -> Wallet backup
//  reminder -> Subscription -> Privacy preferences -> Done. The steps
//  are pushed on the wizard's UINavigationController by
//  `OnboardingWizardCoordinator`, so back navigation is the native
//  chevron + swipe, exactly like the desktop nav-bar back action.
//

import SwiftUI

/// Shared observable state for the wizard. Owned by the
/// `OnboardingWizardCoordinator`, which is the only writer besides the
/// two-way bindings exposed to the step views.
public final class OnboardingWizardModel: ObservableObject {
    /// Wallet backup reminder step: the phrase, or nil while loading.
    @Published public var mnemonic: String?
    @Published public var mnemonicError: String?
    @Published public var backupAcknowledged = false

    /// Subscription step transient state.
    @Published public var subscriptionChecking = false
    @Published public var subscriptionError: String?

    /// Privacy preferences. Seeded from the current tunnel settings.
    @Published public var multiHopAlways = false
    @Published public var daitaEnabled = false

    public init() {}
}

// MARK: - Step 1 : Welcome

struct OnboardingWelcomeStepView: View {
    var onContinue: () -> Void
    var onSkip: () -> Void

    var body: some View {
        OnboardingStepLayout(
            title: String(localized: "Welcome to Warren VPN", table: "Onboarding"),
            description: String(
                localized: "A VPN experience without compromise on privacy. No logs. No accounts. No tracking. Just bandwidth.",
                table: "Onboarding"
            ),
            onSkip: onSkip
        ) {
            OnboardingBrandHero()
        } actions: {
            OnboardingPrimaryButton(
                title: String(localized: "Get started", table: "Onboarding"),
                identifier: .onboardingWelcomeNextButton,
                action: onContinue
            )
        }
    }
}

/// The Warren identity block: the wordmark, nothing else. The steps keep
/// the desktop's sober settings-style layout.
private struct OnboardingBrandHero: View {
    var body: some View {
        Image("WarrenWordmark")
            .renderingMode(.template)
            .resizable()
            .scaledToFit()
            .frame(height: 44)
            .foregroundColor(.white)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 40)
            .accessibilityHidden(true)
    }
}

// MARK: - Step 2 : Wallet backup reminder

/// Second chance to capture the 12 words before connecting. The wallet
/// already exists (created or restored on the login screen); this step
/// re-shows the phrase and gates Continue behind an explicit
/// acknowledgement, matching the desktop `OnboardingWalletView`.
struct OnboardingWalletStepView: View {
    @ObservedObject var model: OnboardingWizardModel
    var onContinue: () -> Void
    var onSkip: () -> Void

    var body: some View {
        OnboardingStepLayout(
            title: String(localized: "Your Warren wallet", table: "Onboarding"),
            description: String(
                localized: "Write down these 12 words in order and keep them somewhere safe. They are the only way to restore your subscription if you lose access to this device.",
                table: "Onboarding"
            ),
            onSkip: onSkip
        ) {
            if let error = model.mnemonicError {
                VStack(alignment: .leading, spacing: 8) {
                    Text(error)
                        .font(.warrenSmall)
                        .foregroundColor(.Warren.error)
                    OnboardingHelpLink()
                }
            } else if let mnemonic = model.mnemonic {
                VStack(alignment: .leading, spacing: 16) {
                    WarrenMnemonicGrid(mnemonic: mnemonic)
                    WarrenMnemonicCopyButton(mnemonic: mnemonic)
                    WarrenAcknowledgeRow(
                        isOn: $model.backupAcknowledged,
                        label: String(
                            localized: "I have written it down in a safe place.",
                            table: "Onboarding"
                        ),
                        identifier: .onboardingWalletAcknowledgeToggle
                    )
                }
            } else {
                HStack {
                    Spacer()
                    ProgressView()
                        .tint(.white)
                    Spacer()
                }
                .padding(.vertical, 32)
            }
        } actions: {
            OnboardingPrimaryButton(
                title: String(localized: "Continue", table: "Onboarding"),
                identifier: .onboardingWalletContinueButton,
                disabled: !model.backupAcknowledged || model.mnemonic == nil,
                action: onContinue
            )
        }
    }
}

// MARK: - Step 3 : Subscription

struct OnboardingSubscriptionStepView: View {
    @ObservedObject var model: OnboardingWizardModel
    var onPurchase: () -> Void
    var onOpenWeb: () -> Void
    var onRedeemVoucher: () -> Void
    var onVerify: () -> Void
    var onSkip: () -> Void

    var body: some View {
        OnboardingStepLayout(
            title: String(localized: "Your subscription", table: "Onboarding"),
            description: String(
                localized: "You don't have an active subscription yet. Plans start at a few euros per month - no recurring billing, no account creation, pay as you go.",
                table: "Onboarding"
            ),
            onSkip: onSkip
        ) {
            if let error = model.subscriptionError {
                VStack(alignment: .leading, spacing: 8) {
                    Text(error)
                        .font(.warrenSmall)
                        .foregroundColor(.Warren.error)
                    OnboardingHelpLink()
                }
            }
        } actions: {
            OnboardingPrimaryButton(
                title: String(localized: "Buy VPN time", table: "Onboarding"),
                disabled: model.subscriptionChecking,
                action: onPurchase
            )
            OnboardingSecondaryButton(
                title: String(localized: "Pay by card on the web", table: "Onboarding"),
                disabled: model.subscriptionChecking,
                action: onOpenWeb
            )
            OnboardingSecondaryButton(
                title: String(localized: "Redeem voucher", table: "Onboarding"),
                identifier: .onboardingSubscriptionRedeemVoucher,
                disabled: model.subscriptionChecking,
                action: onRedeemVoucher
            )
            OnboardingSecondaryButton(
                title: String(localized: "I already have a subscription", table: "Onboarding"),
                identifier: .onboardingSubscriptionLaterCheck,
                disabled: model.subscriptionChecking,
                busy: model.subscriptionChecking,
                action: onVerify
            )
        }
    }
}

// MARK: - Step 4 : Privacy preferences

struct OnboardingPreferencesStepView: View {
    @ObservedObject var model: OnboardingWizardModel
    var onContinue: () -> Void
    var onSkip: () -> Void

    var body: some View {
        OnboardingStepLayout(
            title: String(localized: "Privacy preferences", table: "Onboarding"),
            description: String(
                localized: "Pick the defenses you want from day one. You can change all of these later from Settings.",
                table: "Onboarding"
            ),
            onSkip: onSkip
        ) {
            VStack(spacing: 12) {
                OnboardingPrefRow(
                    title: String(localized: "Multi-hop routing", table: "Onboarding"),
                    subtitle: String(
                        localized: "Adds an entry relay before your exit. Extra latency for harder traffic correlation.",
                        table: "Onboarding"
                    ),
                    systemImage: "arrow.triangle.branch",
                    isOn: $model.multiHopAlways
                )
                OnboardingPrefRow(
                    title: String(localized: "DAITA traffic shaping", table: "Onboarding"),
                    subtitle: String(
                        localized: "Defense against AI-guided traffic analysis. ~10% bandwidth overhead.",
                        table: "Onboarding"
                    ),
                    systemImage: "waveform.path.ecg",
                    isOn: $model.daitaEnabled
                )
                OnboardingPrefRow(
                    title: String(localized: "HTTP/3 mimicry", table: "Onboarding"),
                    subtitle: String(
                        localized: "Always on. Makes VPN traffic indistinguishable from regular HTTPS.",
                        table: "Onboarding"
                    ),
                    systemImage: "checkmark.shield.fill",
                    isOn: .constant(true),
                    disabled: true
                )
            }
        } actions: {
            OnboardingPrimaryButton(
                title: String(localized: "Continue", table: "Onboarding"),
                identifier: .onboardingPreferencesContinueButton,
                action: onContinue
            )
        }
    }
}

// MARK: - Step 5 : Done

struct OnboardingDoneStepView: View {
    var onFinish: () -> Void

    var body: some View {
        OnboardingStepLayout(
            title: String(localized: "All set", table: "Onboarding"),
            description: String(
                localized: "Configuration complete. Pick a country and connect to Warren.",
                table: "Onboarding"
            ),
            onSkip: nil
        ) {
            OnboardingBrandHero()
        } actions: {
            OnboardingPrimaryButton(
                title: String(localized: "Pick a country and connect", table: "Onboarding"),
                identifier: .onboardingDoneFinishButton,
                action: onFinish
            )
        }
    }
}

// MARK: - Shared layout + primitives

/// Shared chrome for a wizard step, mirroring the desktop
/// `OnboardingLayout`: large title, tagline, scrollable content, sticky
/// action footer and the discreet "Skip wizard" link. Back navigation is
/// NOT rendered here: the hosting UINavigationController provides the
/// native chevron and edge-swipe.
struct OnboardingStepLayout<Content: View, Actions: View>: View {
    let title: String
    var description: String?
    var onSkip: (() -> Void)?
    @ViewBuilder var content: () -> Content
    @ViewBuilder var actions: () -> Actions

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(title)
                            .font(.warrenBig)
                            .foregroundColor(.white)
                        if let description {
                            Text(description)
                                .font(.warrenSmall)
                                .foregroundColor(.white.opacity(0.8))
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                    content()
                }
                .padding(.horizontal, 24)
                .padding(.top, 8)
                .padding(.bottom, 32)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            VStack(spacing: 12) {
                actions()
                if let onSkip {
                    Button(action: onSkip) {
                        Text(String(localized: "Skip wizard (advanced)", table: "Onboarding"))
                            .font(.warrenMicroSemiBold)
                            .foregroundColor(.white.opacity(0.6))
                            .underline()
                    }
                    .accessibilityIdentifier(AccessibilityIdentifier.onboardingSkipButton.asString)
                    .padding(.top, 4)
                }
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 24)
        }
        .background(Color.Warren.navy.ignoresSafeArea())
        .preferredColorScheme(.dark)
    }
}

/// External link to the public help page, shown under error messages
/// (desktop `OnboardingForumHint`).
struct OnboardingHelpLink: View {
    var body: some View {
        Link(destination: ApplicationConfiguration.helpURL) {
            HStack(spacing: 4) {
                Text(String(localized: "Having trouble? Visit our help page", table: "Onboarding"))
                    .font(.warrenMicroSemiBold)
                    .underline()
                Image(systemName: "arrow.up.right")
                    .font(.caption2)
            }
            .foregroundColor(.white.opacity(0.8))
        }
    }
}

struct OnboardingPrimaryButton: View {
    var title: String
    var identifier: AccessibilityIdentifier?
    var disabled: Bool = false
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .font(.warrenSmallSemiBold)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 16)
                .background(Color.Warren.success.opacity(disabled ? 0.4 : 1))
                .foregroundColor(.white.opacity(disabled ? 0.6 : 1))
                .cornerRadius(12)
        }
        .disabled(disabled)
        .accessibilityAddTraits(.isButton)
        .modifier(OptionalIdentifier(identifier: identifier))
    }
}

struct OnboardingSecondaryButton: View {
    var title: String
    var identifier: AccessibilityIdentifier?
    var disabled: Bool = false
    var busy: Bool = false
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Group {
                if busy {
                    ProgressView().tint(.white)
                } else {
                    Text(title)
                        .font(.warrenSmall)
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 14)
            .foregroundColor(.white.opacity(disabled ? 0.5 : 0.85))
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(Color.white.opacity(0.25), lineWidth: 1)
            )
        }
        .disabled(disabled)
        .accessibilityAddTraits(.isButton)
        .modifier(OptionalIdentifier(identifier: identifier))
    }
}

private struct OptionalIdentifier: ViewModifier {
    var identifier: AccessibilityIdentifier?

    func body(content: Self.Content) -> some View {
        if let identifier {
            content.accessibilityIdentifier(identifier.asString)
        } else {
            content
        }
    }
}

private struct OnboardingPrefRow: View {
    var title: String
    var subtitle: String
    var systemImage: String
    @Binding var isOn: Bool
    var disabled: Bool = false

    var body: some View {
        HStack(alignment: .top, spacing: 14) {
            Image(systemName: systemImage)
                .font(.title3)
                .foregroundColor(.Warren.yellow)
                .frame(width: 32, height: 32)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.warrenSmallSemiBold)
                    .foregroundColor(.white)
                Text(subtitle)
                    .font(.warrenMicro)
                    .foregroundColor(.white.opacity(0.65))
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 12)
            Toggle("", isOn: $isOn)
                .labelsHidden()
                .tint(.Warren.success)
                .disabled(disabled)
        }
        .padding(16)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(Color.Warren.surface)
        )
    }
}

#if DEBUG
#Preview("Welcome") {
    OnboardingWelcomeStepView(onContinue: {}, onSkip: {})
}

#Preview("Wallet") {
    let model = OnboardingWizardModel()
    model.mnemonic = "wisdom fault frame lecture pistol pill glare hazard node vast phrase mimic"
    return OnboardingWalletStepView(model: model, onContinue: {}, onSkip: {})
}
#endif
