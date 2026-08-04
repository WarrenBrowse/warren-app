//
//  StorePaymentObserver.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

protocol StorePaymentObserver: AnyObject, Sendable {
    @MainActor func storePaymentManager(didReceiveEvent event: StorePaymentEvent)
}
