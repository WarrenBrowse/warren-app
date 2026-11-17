//
//  AccessMethodViewModel.swift
//  MullvadVPN
//
//  Created by pronebird on 14/11/2023.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import Foundation
import WarrenSettings
import WarrenTypes

/// The view model used by view controllers editing access method data.
struct AccessMethodViewModel: Identifiable {
    /// Socks configuration view model.
    struct Socks {
        /// Server IP address input.
        var server = ""
        /// Server port input.
        var port = ""
        /// Authentication username.
        var username = ""
        /// Authentication password.
        var password = ""
        /// Indicates whether authentication is enabled.
        var authenticate = false
    }

    /// Shadowsocks configuration view model.
    struct Shadowsocks {
        /// Server IP address input.
        var server = ""
        /// Server port input.
        var port = ""
        /// Server password.
        var password = ""
        /// Shadowsocks cipher.
        var cipher = ""
    }

    /// Access method testing status view model.
    enum TestingStatus {
        /// The default state before the testing began.
        case initial
        /// Testing is in progress.
        case inProgress
        /// Testing failed.
        case failed
        /// Testing succeeded.
        case succeeded
    }

    /// The unique identifier used for referencing the access method entry in a persistent store.
    var id = UUID()

    /// The user-defined name for access method.
    var name = ""

    /// The selected access method kind.
    /// Determines which subview model is used when presenting proxy configuration in UI.
    var method: AccessMethodKind = .shadowsocks

    /// The flag indicating whether configuration is enabled.
    var isEnabled = true

    /// The flag indicating whether configuration is enabled.
    var canBeToggled = true

    /// The status of testing the entered proxy configuration.
    var testingStatus: TestingStatus = .initial

    /// Socks configuration view model.
    var socks = Socks()

    /// Shadowsocks configuration view model.
    var shadowsocks = Shadowsocks()
}

extension AccessMethodViewModel {
    var infoHeaderConfig: InfoHeaderConfig? {
        switch id {
        case AccessMethodRepository.directId:
            InfoHeaderConfig(
                body: NSLocalizedString("The app communicates with a Warren API server directly.", comment: ""),
                link: String(format: NSLocalizedString("About %@ method...", comment: ""), "Direct")
            )
        default:
            nil
        }
    }

    var infoModalConfig: InfoModalConfig? {
        switch id {
        case AccessMethodRepository.directId:
            InfoModalConfig(
                header: "Direct",
                preamble: NSLocalizedString("The app communicates with a Warren API server directly.", comment: ""),
                body: [
                    NSLocalizedString("With the “Direct” method, the app communicates with a Warren API "
                            + "server directly without any intermediate proxies.",
                        comment: ""
                    ),
                    NSLocalizedString("This can be useful when you are not affected by censorship.", comment: ""),
                ]
            )
        default:
            nil
        }
    }
}
