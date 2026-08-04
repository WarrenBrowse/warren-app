//
//  PaymentAlertPresenter.swift
//  WarrenVPN
//
//  Copyright © 2026 Warren Browse. All rights reserved.
//

import Routing

@MainActor
struct PaymentAlertPresenter {
    let alertContext: any Presenting

    func showAlertForOutcome(
        _ outcome: StorePaymentOutcome,
        context: StorePaymentOutcome.Context,
        completion: (@MainActor @Sendable () -> Void)? = nil
    ) {
        let presentation = AlertPresentation(
            id: "payment-outcome-alert",
            title: context.alertTitle,
            message: outcome.alertMessage(for: context),
            buttons: [
                AlertAction(
                    title: NSLocalizedString("Got it!", comment: ""),
                    style: .default,
                    handler: {
                        completion?()
                    }
                )
            ]
        )

        let presenter = AlertPresenter(context: alertContext)
        presenter.showAlert(presentation: presentation, animated: true)
    }

    func showAlertForError(
        _ error: StorePaymentError,
        context: StorePaymentOutcome.Context,
        completion: (@MainActor @Sendable () -> Void)? = nil
    ) {
        let presentation = AlertPresentation(
            id: "payment-error-alert",
            title: context.errorTitle,
            message: error.description,
            buttons: [
                AlertAction(
                    title: NSLocalizedString("Got it!", comment: ""),
                    style: .default,
                    handler: {
                        completion?()
                    }
                )
            ]
        )

        let presenter = AlertPresenter(context: alertContext)
        presenter.showAlert(presentation: presentation, animated: true)
    }
}
