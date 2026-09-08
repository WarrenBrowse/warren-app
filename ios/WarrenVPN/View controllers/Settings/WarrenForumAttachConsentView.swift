//
//  WarrenForumAttachConsentView.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  The attach-logs consent prompt (doc 55), the iOS mirror of the desktop
//  `ForumAttachPrompt` and Android's `ForumAttachPromptHost`. Names the topic
//  (or says it is a report still being composed, or a typed code with a topic
//  field), lets the exact report be read first ("View the logs"), and uploads
//  only on Approve. Cancel notifies the provider so the waiting forum page
//  shows "cancelled". The copy is held to the string catalog by
//  `WarrenForumAttachCopyTests`.
//

import SwiftUI
import WarrenRustRuntime

struct WarrenForumAttachConsentView: View {
    @ObservedObject var state: WarrenForumAttachPromptState
    let onApprove: () -> Void
    let onCancel: () -> Void
    let onViewLogs: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(
                    NSLocalizedString(
                        "Attach your logs to your forum report?",
                        comment: "Forum attach consent prompt title")
                )
                .font(.headline)
                .foregroundColor(.white)

                Text(bodyText)
                    .font(.body)
                    .foregroundColor(.white.opacity(0.85))

                Text(
                    NSLocalizedString(
                        "The logs are anonymized: wallet addresses and personal data are removed. They go privately to the Warren support team and never appear publicly on the forum. You can view exactly what will be sent before approving. Only approve if you opened that forum page yourself.",
                        comment: "Forum attach consent prompt, what leaves the device and the caution")
                )
                .font(.footnote)
                .foregroundColor(.white.opacity(0.7))

                if state.needsTopic {
                    topicField
                }

                Button(action: onViewLogs) {
                    HStack(spacing: 8) {
                        Text(
                            NSLocalizedString("View the logs", comment: "Forum attach, open the report preview"))
                        if state.collecting {
                            ProgressView().tint(.white)
                        }
                    }
                    .font(.warrenSmallSemiBold)
                    .foregroundColor(.Warren.yellow)
                }
                .disabled(state.collecting || state.busy)

                if state.collecting {
                    Text(
                        NSLocalizedString(
                            "Preparing the report, please wait.", comment: "Forum attach, collecting the report")
                    )
                    .font(.warrenMicro)
                    .foregroundColor(.white.opacity(0.7))
                }
                if state.collectFailed {
                    Text(
                        NSLocalizedString(
                            "The logs could not be collected. Try again in a moment.",
                            comment: "Forum attach, the report could not be collected")
                    )
                    .font(.warrenMicro)
                    .foregroundColor(.Warren.error)
                }
                if let failure = state.failure {
                    Text(failure)
                        .font(.warrenMicro)
                        .foregroundColor(.Warren.error)
                        .accessibilityAddTraits(.updatesFrequently)
                }
                if state.busy {
                    Text(
                        NSLocalizedString(
                            "Attaching the logs, please wait.", comment: "Forum attach, the upload is in flight"))
                        .font(.warrenMicro)
                        .foregroundColor(.white.opacity(0.7))
                }

                buttons
                Spacer()
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(Color.Warren.navy)
        .sheet(
            isPresented: Binding(get: { state.preview != nil }, set: { if !$0 { state.closePreview() } })
        ) {
            WarrenForumAttachPreview(text: state.preview ?? "", onClose: { state.closePreview() })
        }
    }

    private var bodyText: String {
        if let topicId = state.link.topicId {
            if topicId == ForumAttachLink.preTopic {
                return NSLocalizedString(
                    "The bug report you are writing on the Warren forum asks this app to attach its technical logs, to help diagnose the problem. They join the report once you post it.",
                    comment: "Forum attach consent prompt body, a report still being composed")
            }
            return String(
                format: NSLocalizedString(
                    "Your bug report on the Warren forum (topic %lld) asks this app to attach its technical logs, to help diagnose the problem.",
                    comment: "Forum attach consent prompt body, a numbered topic"),
                Int64(topicId))
        }
        return NSLocalizedString(
            "This code belongs to a forum page that asks this app to attach its technical logs to a bug report, to help diagnose the problem.",
            comment: "Forum attach consent prompt body, a session id typed by hand")
    }

    private var topicField: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(NSLocalizedString("Topic number", comment: "Forum attach, the topic-number field label"))
                .font(.warrenSmallSemiBold)
                .foregroundColor(.white)
            TextField("", text: Binding(get: { state.topicInput }, set: { state.updateTopicInput($0) }))
                .keyboardType(.numberPad)
                .padding(12)
                .background(RoundedRectangle(cornerRadius: 8).fill(Color.Warren.surface))
                .foregroundColor(.white)
                .disabled(state.busy)
                .accessibilityIdentifier("forumAttachTopicField")
            Text(
                NSLocalizedString(
                    "The number in the topic's address on the forum. Leave it empty if you are still writing the report.",
                    comment: "Forum attach, the topic-number field hint")
            )
            .font(.warrenMicro)
            .foregroundColor(.white.opacity(0.6))
        }
    }

    private var buttons: some View {
        HStack(spacing: 12) {
            Button(action: onCancel) {
                Text(NSLocalizedString("Cancel", comment: "Forum attach consent prompt, the refusing button"))
                    .font(.warrenSmallSemiBold)
                    .foregroundColor(.white)
                    .frame(maxWidth: .infinity)
                    .padding(12)
                    .background(RoundedRectangle(cornerRadius: 8).stroke(Color.white.opacity(0.4)))
            }
            .disabled(state.busy)
            .accessibilityIdentifier("forumAttachCancel")

            Button(action: onApprove) {
                Text(NSLocalizedString("Attach the logs", comment: "Forum attach consent prompt, the approving button"))
                    .font(.warrenSmallSemiBold)
                    .foregroundColor(.Warren.navy)
                    .frame(maxWidth: .infinity)
                    .padding(12)
                    .background(RoundedRectangle(cornerRadius: 8).fill(Color.Warren.yellow))
            }
            .disabled(!state.canApprove)
            .opacity(state.canApprove ? 1 : 0.5)
            .accessibilityIdentifier("forumAttachApprove")
        }
    }
}

/// The exact redacted report about to be sent, read-only, over the prompt.
private struct WarrenForumAttachPreview: View {
    let text: String
    let onClose: () -> Void

    var body: some View {
        NavigationView {
            ScrollView([.vertical, .horizontal]) {
                Text(text)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(.white)
                    .padding(16)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .background(Color.Warren.navy)
            .navigationTitle(NSLocalizedString("Logs to be sent", comment: "Forum attach, the report preview title"))
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(NSLocalizedString("Done", comment: "Forum attach, close the report preview"), action: onClose)
                }
            }
        }
    }
}
