package com.warrenbrowse.vpn.lib.model

data class ExitConstraints(
    val location: Constraint<RelayItemId> = Constraint.Any,
    val providers: Constraint<Providers> = Constraint.Any,
    val ownership: Constraint<Ownership> = Constraint.Any,
)
