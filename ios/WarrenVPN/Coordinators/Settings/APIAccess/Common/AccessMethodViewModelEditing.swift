//
//  AccessMethodViewModelEditing.swift
//  MullvadVPN
//
//  Created by Jon Petersson on 2024-01-23.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenTypes

protocol AccessMethodEditing: AnyObject {
    func accessMethodDidSave(_ accessMethod: PersistentAccessMethod)
}
