package com.warrenbrowse.vpn.lib.model

sealed interface RelaySelectorPredicate {
    data class SingleHop(val entryConstraints: EntryConstraints = EntryConstraints()) :
        RelaySelectorPredicate

    data class Autohop(val entryConstraints: EntryConstraints = EntryConstraints()) :
        RelaySelectorPredicate

    data class Entry(val multihopConstraints: MultihopConstraints = MultihopConstraints()) :
        RelaySelectorPredicate

    data class Exit(val multihopConstraints: MultihopConstraints = MultihopConstraints()) :
        RelaySelectorPredicate
}
