//
//  InAppPurchaseCoordinator.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Routing
import UIKit

enum PaymentAction {
    case purchase
    case restorePurchase
}

final class InAppPurchaseCoordinator: Coordinator, Presentable, Presenting {
    private var controller: InAppPurchaseViewController?
    private let storePaymentManager: StorePaymentManager
    private let paymentAction: PaymentAction

    var didFinish: ((InAppPurchaseCoordinator) -> Void)?

    var presentedViewController: UIViewController {
        return controller!
    }

    init(storePaymentManager: StorePaymentManager, paymentAction: PaymentAction) {
        self.storePaymentManager = storePaymentManager
        self.paymentAction = paymentAction
    }

    func dismiss() {
        didFinish?(self)
    }

    func start() {
        controller = InAppPurchaseViewController(
            storePaymentManager: storePaymentManager,
            errorPresenter: PaymentAlertPresenter(alertContext: self),
            paymentAction: paymentAction
        )
        controller?.didFinish = dismiss
    }
}
