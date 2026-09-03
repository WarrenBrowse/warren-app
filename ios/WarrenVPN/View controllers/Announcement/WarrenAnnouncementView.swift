//
//  WarrenAnnouncementView.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The launch announcement in full: the operator's headline and body, the
//  voucher code drawn for this account, and the call to action.
//
//  Every string that comes from the server (the headline, the body, the
//  call-to-action label) is rendered as plain text, with `Text(verbatim:)` so
//  nothing is looked up as a localization key and no markup is interpreted:
//  the signed channel exists so that what the operator wrote is what the
//  reader reads.
//

import SwiftUI
import UIKit
import WarrenRustRuntime

struct WarrenAnnouncementView: View {
    let announcement: WarrenAnnouncement
    /// The call to action, opened through the app's external-link path.
    var onOpenLink: ((URL) -> Void)?
    var onDismiss: (() -> Void)?

    /// Whether the code block belongs on the screen. `false` both when the
    /// announcement carries no campaign at all and when this account is outside
    /// its cohort: either way the operator's text reaches the reader and no
    /// empty field promises a code that does not exist.
    static func showsVoucherWell(_ announcement: WarrenAnnouncement) -> Bool {
        announcement.voucherCode != nil
    }

    var body: some View {
        VStack(spacing: 0) {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    Text(verbatim: announcement.headline)
                        .font(.warrenLarge)
                        .foregroundStyle(Color.MullvadText.onBackgroundEmphasis100)

                    Text(verbatim: announcement.body)
                        .font(.warrenSmall)
                        .foregroundStyle(Color.MullvadText.onBackground)
                        .fixedSize(horizontal: false, vertical: true)

                    if Self.showsVoucherWell(announcement), let code = announcement.voucherCode {
                        WarrenVoucherWell(code: code)
                    }

                    if let cta = announcement.cta {
                        Button(action: { onOpenLink?(cta.url) }) {
                            Text(verbatim: cta.label)
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(MainButtonStyle(.default))
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.top, 24)
            }
            .scrollBounceBehavior(.automatic)

            MainButton(text: "Got it", style: .success) {
                onDismiss?()
            }
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 16)
        .background(Color.warrenBackground)
    }
}

/// The code this account was pre-assigned, in a well of its own so it reads as
/// a field to act on rather than as more prose.
///
/// Selectable, and in a monospaced face: a 16 character code has to be
/// transcribable by eye when the clipboard is not where the reader needs it.
/// Nothing wipes the clipboard afterwards, unlike the recovery phrase: the
/// code is meant to be pasted into the production app, possibly after
/// installing it, and a timed wipe would take it away mid-errand.
struct WarrenVoucherWell: View {
    let code: String
    @State private var didCopy = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(didCopy ? "Copied" : "Your code")
                .font(.warrenTinySemiBold)
                .foregroundStyle(Color.MullvadText.onBackground)

            HStack(alignment: .center, spacing: 12) {
                Text(verbatim: code)
                    .font(.system(.body, design: .monospaced))
                    .foregroundStyle(Color.MullvadText.onBackgroundEmphasis100)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)

                Button(action: copy) {
                    Image(systemName: didCopy ? "checkmark" : "doc.on.doc")
                        .foregroundStyle(Color.Warren.yellow)
                }
                // The confirmation reaches a screen reader too: the label is
                // the only thing that changes for it when the glyph does.
                .accessibilityLabel(didCopy ? Text("Copied") : Text("Copy the code"))
            }
        }
        .padding(12)
        .background(Color.Warren.surface)
        .clipShape(RoundedCornerShape(radius: 8))
        .overlay(
            RoundedCornerShape(radius: 8)
                .stroke(Color.Warren.accentTint.opacity(0.4), lineWidth: 1)
        )
    }

    private func copy() {
        UIPasteboard.general.string = code
        didCopy = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            didCopy = false
        }
    }
}

/// A rounded rectangle, named apart so the well's fill and its border share
/// one radius.
private struct RoundedCornerShape: Shape {
    let radius: CGFloat

    func path(in rect: CGRect) -> Path {
        Path(roundedRect: rect, cornerRadius: radius, style: .continuous)
    }
}

/// Presents one announcement over whatever is on screen. Held apart from the
/// notification provider so the provider stays free of UIKit presentation and
/// can be exercised without a window.
enum WarrenAnnouncementPresenter {
    @MainActor
    static func present(_ announcement: WarrenAnnouncement, over presenter: UIViewController) {
        var controller: UIViewController?
        let view = WarrenAnnouncementView(
            announcement: announcement,
            onOpenLink: { url in
                // The same external-link path every other Warren screen takes.
                // Rust already refused a URL that is not safe to render as a
                // link, so what arrives here is https with a plain host.
                UIApplication.shared.open(url, options: [:], completionHandler: nil)
            },
            onDismiss: { controller?.dismiss(animated: true) }
        )
        let hosting = UIHostingController(rootView: view)
        hosting.modalPresentationStyle = .pageSheet
        controller = hosting
        presenter.present(hosting, animated: true)
    }
}

#Preview {
    WarrenAnnouncementView(
        announcement: WarrenAnnouncement(
            id: "a1",
            headline: "Production is open",
            body: "Warren is out of beta. Your account gets a free month on the production "
                + "service, and the code below redeems it.",
            level: .warning,
            cta: WarrenAnnouncementCta(
                label: "Get Warren",
                url: URL(string: "https://warren.ro/download")!
            ),
            voucherCampaignID: "prod-launch",
            voucherCode: "ABCD1234EFGH5678"
        )
    )
}
