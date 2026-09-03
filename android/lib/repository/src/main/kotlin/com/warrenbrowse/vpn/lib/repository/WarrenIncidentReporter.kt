package com.warrenbrowse.vpn.lib.repository

/**
 * The user-driven half of the incident telemetry: what the "Report to Warren" choice on the
 * key-mismatch dialog sends. The automatic half (an exit a drop retry gave up on) is posted by the
 * tunnel adapter and never reaches a screen.
 *
 * The interface lives here so `lib/feature/home/impl` can offer the choice without depending on
 * the `:app` module; the implementation is `app/connect/WarrenIncidentReportUseCase`.
 */
interface WarrenIncidentReporter {
    /**
     * Tell the operator that [exitIdHex] served [newPubkeyHex] where this device had pinned
     * [oldPubkeyHex], so a substitution attempt can be correlated against the access log. Every
     * field is already public through the signed relay list, and the server records no signer.
     *
     * Best effort: returns whether the report left, and the caller stays disconnected either way.
     * Never throws.
     */
    suspend fun reportPubkeyMismatch(
        exitIdHex: String,
        oldPubkeyHex: String,
        newPubkeyHex: String,
        countryCode: String,
        city: String,
    ): Boolean
}
