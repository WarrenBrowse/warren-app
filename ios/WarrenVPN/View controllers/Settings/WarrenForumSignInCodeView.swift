//
//  WarrenForumSignInCodeView.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//
//  Settings, "Sign in to the forum with a code". The forum sign-in finished
//  by hand: the approval page shows its session id as a code when tapping
//  its button did not open the app (a browser that asks first, no handler
//  registered, an old install). Typing it here raises the very same consent
//  prompt a deep link would, so the browser stops being a single point of
//  failure between the forum and the wallet. Same copy as the desktop and
//  Android screens.
//

import SwiftUI

public struct WarrenForumSignInCodeView: View {
    /// Hands the typed code to the login flow; false when it is not a session
    /// id, in which case the field shows why.
    public let onSubmit: (String) -> Bool

    @State private var code = ""
    @State private var invalid = false

    public init(onSubmit: @escaping (String) -> Bool) {
        self.onSubmit = onSubmit
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(
                    String(
                        localized:
                            "On the forum sign-in page, the session code is shown under the button. Type it here to approve that sign-in from this app. The page keeps waiting while you do.",
                        table: "Settings",
                        comment: "Explains where the forum sign-in code comes from and what typing it does"
                    )
                )
                .font(.body)
                .foregroundColor(.white.opacity(0.8))

                VStack(alignment: .leading, spacing: 6) {
                    Text(
                        String(
                            localized: "Sign-in code", table: "Settings",
                            comment: "Label of the forum sign-in code input field")
                    )
                    .font(.warrenSmallSemiBold)
                    .foregroundColor(.white)

                    TextField("0123456789abcdef0123456789abcdef", text: $code)
                        .font(.system(.body, design: .monospaced))
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.asciiCapable)
                        .submitLabel(.done)
                        .onSubmit(submit)
                        .onChange(of: code) { _, _ in invalid = false }
                        .padding(12)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(Color.Warren.surface)
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 8)
                                .stroke(invalid ? Color.red : Color.clear, lineWidth: 1)
                        )
                        .accessibilityIdentifier("forumSignInCodeField")

                    if invalid {
                        Text(
                            String(
                                localized: "A sign-in code is 32 letters and digits, as shown on the forum page.",
                                table: "Settings",
                                comment: "Shown when the typed forum sign-in code is not a 32-character session id"
                            )
                        )
                        .font(.warrenMicro)
                        .foregroundColor(.red)
                    }
                }

                Button(action: submit) {
                    Text(
                        String(
                            localized: "Continue", table: "Settings",
                            comment: "Button that hands the typed forum sign-in code to the consent prompt")
                    )
                    .font(.warrenSmallSemiBold)
                    .foregroundColor(.Warren.navy)
                    .frame(maxWidth: .infinity)
                    .padding(12)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(Color.Warren.yellow)
                    )
                }
                .disabled(code.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .opacity(code.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? 0.5 : 1)
                .accessibilityIdentifier("forumSignInCodeContinue")

                Spacer()
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(Color.Warren.navy)
    }

    private func submit() {
        // Checked in the flow as the boundary that decides what a code
        // stands for; the view only reflects its answer.
        if !onSubmit(code) {
            invalid = true
        }
    }
}
