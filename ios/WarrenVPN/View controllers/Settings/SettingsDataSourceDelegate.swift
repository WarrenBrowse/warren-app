//
//  SettingsDataSourceDelegate.swift
//  MullvadVPN
//
//  Created by pronebird on 19/10/2021.
//  Copyright © 2026 Mullvad VPN AB. All rights reserved.
//

import WarrenSettings
import UIKit

protocol SettingsDataSourceDelegate: AnyObject {
    func didSelectItem(item: SettingsDataSource.Item)
}
