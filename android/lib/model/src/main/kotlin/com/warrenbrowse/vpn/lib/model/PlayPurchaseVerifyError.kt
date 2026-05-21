package com.warrenbrowse.vpn.lib.model

enum class PlayPurchaseVerifyError {
    NoProducts,
    MissingObfuscatedAccountId,
    NoPurchaseToken,
    InvalidPurchase,
    OtherError,
}
