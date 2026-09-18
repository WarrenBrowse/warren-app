//
//  WarrenForumActivityView.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Community-forum activity, opened from the header bell. A screen rather
//  than a popover: Account and Settings both open this way, and a dropdown
//  over the connect screen exists nowhere else in this app.
//

import SwiftUI
import WarrenRustRuntime

struct WarrenForumActivityView: View {
    @ObservedObject var viewModel: WarrenForumActivityViewModel

    /// The forum name this wallet posts under, shown once at the top so the
    /// reader knows whose activity this is.
    var handle: String?

    /// Opens a forum page. Handed in so the view never reaches for
    /// `UIApplication` itself.
    var openURL: (URL) -> Void

    var body: some View {
        SettingsInfoContainerView {
            VStack(alignment: .leading, spacing: 16) {
                if let handle {
                    Text(handle)
                        .font(.warrenSmall)
                        .foregroundStyle(.white.opacity(0.4))
                        .padding(.horizontal, 16)
                }

                switch viewModel.state {
                case .loading:
                    centered {
                        ProgressView()
                            .tint(.white)
                    }
                case let .rows(rows) where rows.isEmpty:
                    centered {
                        Text(
                            NSLocalizedString(
                                "Nothing new on the forum.",
                                tableName: "Settings",
                                comment: ""
                            )
                        )
                        .font(.warrenSmall)
                        .multilineTextAlignment(.center)
                        .foregroundStyle(.white.opacity(0.6))
                    }
                case let .rows(rows):
                    GroupedRowView {
                        ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
                            WarrenForumNotificationRowView(notification: row, openURL: openURL)
                            if index < rows.count - 1 {
                                RowSeparator()
                            }
                        }
                    }
                    .padding(.horizontal, 16)
                case .failed:
                    centered {
                        VStack(spacing: 16) {
                            Text(
                                NSLocalizedString(
                                    "Could not reach the forum.",
                                    tableName: "Settings",
                                    comment: ""
                                )
                            )
                            .font(.warrenSmall)
                            .multilineTextAlignment(.center)
                            .foregroundStyle(.white.opacity(0.6))

                            MainButton(
                                text: "Try again",
                                style: .default,
                                action: { Task { await viewModel.load() } }
                            )
                        }
                    }
                }
            }
        }
        .task { await viewModel.load() }
    }

    private func centered<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        HStack {
            Spacer()
            content()
            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 32)
    }
}

/// One row: the glyph, who did what, the topic it happened in, and how long
/// ago. Nothing here is markup: every field was validated in Rust.
struct WarrenForumNotificationRowView: View {
    let notification: WarrenForumNotification
    let openURL: (URL) -> Void

    var body: some View {
        Button {
            if let url = WarrenForumActivityRow.url(for: notification) {
                openURL(url)
            }
        } label: {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: WarrenForumActivityRow.symbol(for: notification.kind))
                    .foregroundStyle(.white.opacity(notification.unread ? 1 : 0.4))
                    .frame(width: 20)

                VStack(alignment: .leading, spacing: 2) {
                    Text(WarrenForumActivityRow.headline(for: notification))
                        .font(.warrenSmallSemiBold)
                        .foregroundStyle(.white)
                    if let title = notification.title {
                        Text(title)
                            .font(.warrenSmall)
                            .foregroundStyle(.white.opacity(0.6))
                            .lineLimit(2)
                    }
                    Text(WarrenForumActivityRow.age(of: notification.createdAt))
                        .font(.warrenMini)
                        .foregroundStyle(.white.opacity(0.4))
                }

                Spacer(minLength: 0)
            }
            .padding(16)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(WarrenForumActivityRow.url(for: notification) == nil)
    }
}
