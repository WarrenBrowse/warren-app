//
//  OnboardingWizardView.swift
//  WarrenVPN
//
//  Created by Warren on 2026-05-21.
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  SwiftUI 5-step onboarding wizard for first-launch flow:
//  Welcome -> Wallet -> Subscription -> Privacy preferences -> Done.
//  The Wallet step delegates to `WarrenWalletCoordinator` via the
//  coordinator-supplied callback, so the existing generate/import
//  flow + Keychain integration is reused unmodified.
//

import SafariServices
import SwiftUI

/// Steps in the onboarding wizard.
public enum OnboardingStep: Int, CaseIterable, Identifiable {
    case welcome = 0
    case wallet = 1
    case subscription = 2
    case privacy = 3
    case done = 4

    public var id: Int { rawValue }
}

/// State container shared across the wizard. Owned by the
/// `OnboardingWizardCoordinator`.
public final class OnboardingWizardState: ObservableObject {
    @Published public var currentStep: OnboardingStep = .welcome
    @Published public var hasWallet: Bool = false
    @Published public var multiHopEnabled: Bool = false
    @Published public var daitaEnabled: Bool = false

    public init() {}

    public func advance() {
        guard let next = OnboardingStep(rawValue: currentStep.rawValue + 1) else { return }
        withAnimation(.spring(response: 0.4, dampingFraction: 0.85)) {
            currentStep = next
        }
    }

    public func goBack() {
        guard let prev = OnboardingStep(rawValue: currentStep.rawValue - 1) else { return }
        withAnimation(.spring(response: 0.4, dampingFraction: 0.85)) {
            currentStep = prev
        }
    }
}

/// Root SwiftUI view for the onboarding wizard. The wallet step
/// surfaces two callbacks (`onGenerateWallet`, `onImportWallet`) so the
/// owning UIKit Coordinator can push the `WarrenWalletCoordinator`
/// without duplicating UI.
public struct OnboardingWizardView: View {
    @ObservedObject public var state: OnboardingWizardState
    public var onGenerateWallet: () -> Void
    public var onImportWallet: () -> Void
    public var onOpenSubscription: () -> Void
    public var onFinish: () -> Void

    public init(
        state: OnboardingWizardState,
        onGenerateWallet: @escaping () -> Void,
        onImportWallet: @escaping () -> Void,
        onOpenSubscription: @escaping () -> Void,
        onFinish: @escaping () -> Void
    ) {
        self.state = state
        self.onGenerateWallet = onGenerateWallet
        self.onImportWallet = onImportWallet
        self.onOpenSubscription = onOpenSubscription
        self.onFinish = onFinish
    }

    public var body: some View {
        VStack(spacing: 0) {
            stepIndicator
                .padding(.top, 24)
                .padding(.bottom, 16)

            TabView(selection: $state.currentStep) {
                WelcomeStepView(onContinue: state.advance)
                    .tag(OnboardingStep.welcome)

                WalletStepView(
                    hasWallet: state.hasWallet,
                    onGenerate: onGenerateWallet,
                    onImport: onImportWallet,
                    onSkip: { state.advance() }
                )
                .tag(OnboardingStep.wallet)

                SubscriptionStepView(
                    onOpenSubscription: onOpenSubscription,
                    onContinue: state.advance
                )
                .tag(OnboardingStep.subscription)

                PrivacyPrefsStepView(
                    multiHopEnabled: $state.multiHopEnabled,
                    daitaEnabled: $state.daitaEnabled,
                    onContinue: state.advance
                )
                .tag(OnboardingStep.privacy)

                DoneStepView(onLaunch: onFinish)
                    .tag(OnboardingStep.done)
            }
            .tabViewStyle(.page(indexDisplayMode: .never))
            .animation(.spring(response: 0.4, dampingFraction: 0.85), value: state.currentStep)
        }
        .background(Color.Warren.navy.ignoresSafeArea())
        .preferredColorScheme(.dark)
    }

    @ViewBuilder
    private var stepIndicator: some View {
        HStack(spacing: 8) {
            ForEach(OnboardingStep.allCases) { step in
                Capsule()
                    .fill(step.rawValue <= state.currentStep.rawValue ? Color.Warren.yellow : Color.white.opacity(0.2))
                    .frame(width: step == state.currentStep ? 28 : 8, height: 8)
                    .animation(.spring(response: 0.4, dampingFraction: 0.85), value: state.currentStep)
            }
        }
        .accessibilityLabel(
            String(
                format: String(localized: "Step %lld of %lld", table: "Onboarding"),
                state.currentStep.rawValue + 1,
                OnboardingStep.allCases.count
            )
        )
    }
}

// MARK: - Step 1 : Welcome

private struct WelcomeStepView: View {
    var onContinue: () -> Void

    var body: some View {
        StepContainer {
            VStack(spacing: 24) {
                Image(systemName: "shield.lefthalf.filled")
                    .font(.system(size: 96, weight: .light))
                    .foregroundColor(.Warren.yellow)
                    .accessibilityHidden(true)

                Text(String(localized: "Welcome to Warren VPN", table: "Onboarding"))
                    .font(.warrenBig)
                    .foregroundColor(.white)
                    .multilineTextAlignment(.center)

                Text(String(localized: "A decentralized VPN with non-custodial wallet authentication, always-on HTTP/3 mimicry, and optional multi-hop routing.", table: "Onboarding"))
                    .font(.warrenSmall)
                    .foregroundColor(.white.opacity(0.7))
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)
            }
        } primary: {
            PrimaryStepButton(
                title: String(localized: "Get started", table: "Onboarding"),
                action: onContinue
            )
        }
    }
}

// MARK: - Step 2 : Wallet

private struct WalletStepView: View {
    var hasWallet: Bool
    var onGenerate: () -> Void
    var onImport: () -> Void
    var onSkip: () -> Void

    var body: some View {
        StepContainer {
            VStack(spacing: 24) {
                Image(systemName: "key.horizontal.fill")
                    .font(.system(size: 84, weight: .light))
                    .foregroundColor(.Warren.yellow)
                    .accessibilityHidden(true)

                Text(String(localized: "Your Warren wallet", table: "Onboarding"))
                    .font(.warrenBig)
                    .foregroundColor(.white)
                    .multilineTextAlignment(.center)

                Text(String(localized: "Warren uses a non-custodial Ed25519 wallet derived from a 12-word recovery phrase. You alone hold the keys, and you can restore your subscription on any device using the same phrase.", table: "Onboarding"))
                    .font(.warrenSmall)
                    .foregroundColor(.white.opacity(0.7))
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)

                if hasWallet {
                    Label(
                        String(localized: "Wallet ready", table: "Onboarding"),
                        systemImage: "checkmark.circle.fill"
                    )
                    .font(.warrenSmallSemiBold)
                    .foregroundColor(.Warren.yellow)
                }
            }
        } primary: {
            VStack(spacing: 12) {
                PrimaryStepButton(
                    title: hasWallet
                        ? String(localized: "Continue", table: "Onboarding")
                        : String(localized: "Generate new wallet", table: "Onboarding"),
                    action: hasWallet ? onSkip : onGenerate
                )
                if !hasWallet {
                    SecondaryStepButton(
                        title: String(localized: "I already have a recovery phrase", table: "Onboarding"),
                        action: onImport
                    )
                }
            }
        }
    }
}

// MARK: - Step 3 : Subscription

private struct SubscriptionStepView: View {
    var onOpenSubscription: () -> Void
    var onContinue: () -> Void

    var body: some View {
        StepContainer {
            VStack(spacing: 24) {
                Image(systemName: "tag.fill")
                    .font(.system(size: 80, weight: .light))
                    .foregroundColor(.Warren.yellow)
                    .accessibilityHidden(true)

                Text(String(localized: "Activate your subscription", table: "Onboarding"))
                    .font(.warrenBig)
                    .foregroundColor(.white)
                    .multilineTextAlignment(.center)

                Text(String(localized: "Subscriptions are paid in privacy-preserving currencies on warrenbrowse.com. Your wallet pubkey is the only identifier the Warren network sees; no email or account number required.", table: "Onboarding"))
                    .font(.warrenSmall)
                    .foregroundColor(.white.opacity(0.7))
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)
            }
        } primary: {
            VStack(spacing: 12) {
                PrimaryStepButton(
                    title: String(localized: "Open warrenbrowse.com", table: "Onboarding"),
                    action: onOpenSubscription
                )
                SecondaryStepButton(
                    title: String(localized: "Maybe later", table: "Onboarding"),
                    action: onContinue
                )
            }
        }
    }
}

// MARK: - Step 4 : Privacy preferences

private struct PrivacyPrefsStepView: View {
    @Binding var multiHopEnabled: Bool
    @Binding var daitaEnabled: Bool
    var onContinue: () -> Void

    var body: some View {
        StepContainer {
            VStack(alignment: .leading, spacing: 16) {
                Text(String(localized: "Privacy preferences", table: "Onboarding"))
                    .font(.warrenBig)
                    .foregroundColor(.white)

                Text(String(localized: "Pick your defaults. You can change these anytime in Settings.", table: "Onboarding"))
                    .font(.warrenSmall)
                    .foregroundColor(.white.opacity(0.7))

                VStack(spacing: 12) {
                    PrefRow(
                        title: String(localized: "Multi-hop routing", table: "Onboarding"),
                        subtitle: String(localized: "Adds an entry relay before your exit. Extra latency for harder traffic correlation.", table: "Onboarding"),
                        systemImage: "arrow.triangle.branch",
                        isOn: $multiHopEnabled
                    )
                    PrefRow(
                        title: String(localized: "DAITA traffic shaping", table: "Onboarding"),
                        subtitle: String(localized: "Defense against AI-guided traffic analysis. ~10% bandwidth overhead.", table: "Onboarding"),
                        systemImage: "waveform.path.ecg",
                        isOn: $daitaEnabled
                    )
                    PrefRow(
                        title: String(localized: "HTTP/3 mimicry", table: "Onboarding"),
                        subtitle: String(localized: "Always on. Makes VPN traffic indistinguishable from regular HTTPS.", table: "Onboarding"),
                        systemImage: "checkmark.shield.fill",
                        isOn: .constant(true),
                        disabled: true
                    )
                }
            }
        } primary: {
            PrimaryStepButton(
                title: String(localized: "Continue", table: "Onboarding"),
                action: onContinue
            )
        }
    }
}

// MARK: - Step 5 : Done

private struct DoneStepView: View {
    var onLaunch: () -> Void

    var body: some View {
        StepContainer {
            VStack(spacing: 24) {
                Image(systemName: "checkmark.seal.fill")
                    .font(.system(size: 96, weight: .light))
                    .foregroundColor(.Warren.yellow)
                    .accessibilityHidden(true)

                Text(String(localized: "You're all set", table: "Onboarding"))
                    .font(.warrenBig)
                    .foregroundColor(.white)
                    .multilineTextAlignment(.center)

                Text(String(localized: "Tap Launch Warren to connect to the network. Your wallet stays in your secure enclave; no Warren server ever sees your private key.", table: "Onboarding"))
                    .font(.warrenSmall)
                    .foregroundColor(.white.opacity(0.7))
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)
            }
        } primary: {
            PrimaryStepButton(
                title: String(localized: "Launch Warren", table: "Onboarding"),
                action: onLaunch
            )
        }
    }
}

// MARK: - Step layout + button primitives

private struct StepContainer<Content: View, Primary: View>: View {
    @ViewBuilder var content: () -> Content
    @ViewBuilder var primary: () -> Primary

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                content()
                    .padding(.horizontal, 24)
                    .padding(.vertical, 32)
                    .frame(maxWidth: .infinity, alignment: .center)
            }
            primary()
                .padding(.horizontal, 24)
                .padding(.bottom, 24)
        }
    }
}

private struct PrimaryStepButton: View {
    var title: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .font(.warrenSmallSemiBold)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 16)
                .background(Color.Warren.yellow)
                .foregroundColor(.black)
                .cornerRadius(12)
        }
        .accessibilityAddTraits(.isButton)
    }
}

private struct SecondaryStepButton: View {
    var title: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .font(.warrenSmall)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 14)
                .background(Color.clear)
                .foregroundColor(.white.opacity(0.85))
                .overlay(
                    RoundedRectangle(cornerRadius: 12)
                        .stroke(Color.white.opacity(0.25), lineWidth: 1)
                )
                .cornerRadius(12)
        }
        .accessibilityAddTraits(.isButton)
    }
}

private struct PrefRow: View {
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
                .tint(.Warren.yellow)
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
#Preview {
    let state = OnboardingWizardState()
    return OnboardingWizardView(
        state: state,
        onGenerateWallet: {},
        onImportWallet: {},
        onOpenSubscription: {},
        onFinish: {}
    )
}
#endif
