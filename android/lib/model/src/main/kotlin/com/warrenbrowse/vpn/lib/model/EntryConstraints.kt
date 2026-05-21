package com.warrenbrowse.vpn.lib.model

data class EntryConstraints(
    val generalConstraints: ExitConstraints = ExitConstraints(),
    val obfuscation: Constraint<ObfuscationSettings> = Constraint.Any,
    val daitaSettings: Constraint<DaitaSettings> = Constraint.Any,
    val ipVersion: Constraint<IpVersion> = Constraint.Any,
)
