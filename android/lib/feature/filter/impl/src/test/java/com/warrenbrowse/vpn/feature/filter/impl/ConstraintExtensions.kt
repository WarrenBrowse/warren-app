package com.warrenbrowse.vpn.feature.filter.impl

import com.warrenbrowse.vpn.lib.model.Constraint
import com.warrenbrowse.vpn.lib.model.Ownership
import com.warrenbrowse.vpn.lib.model.Providers

fun Ownership?.toOwnershipConstraint(): Constraint<Ownership> =
    when (this) {
        null -> Constraint.Any
        else -> Constraint.Only(this)
    }

fun Providers.toConstraintProviders(allProviders: Providers): Constraint<Providers> =
    if (size == allProviders.size) {
        Constraint.Any
    } else {
        Constraint.Only(this)
    }
