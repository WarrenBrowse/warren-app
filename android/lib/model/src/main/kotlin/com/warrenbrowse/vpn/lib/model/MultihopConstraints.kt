package com.warrenbrowse.vpn.lib.model

data class MultihopConstraints(
    val entryConstraints: EntryConstraints = EntryConstraints(),
    val exitConstraints: ExitConstraints = ExitConstraints(),
)
