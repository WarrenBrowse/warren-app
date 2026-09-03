package com.warrenbrowse.vpn.core.animation

// The screen push and pop follow the desktop shape (transition-hooks.ts): the
// arriving screen travels the full width and the one underneath recedes by a
// third, so both clients read a push as descending into the same stack. The
// duration is the one deliberate deviation: the desktop's 450 ms reads as
// sluggish on a phone next to the platform's own 300 to 400 ms large-surface
// motion, so 350 ms is the ceiling taken here. DesignMotionParityTest pins all
// three next to the desktop values.
const val TRANSITION_DEFAULT_DURATION_MS = 350
const val ENTER_TRANSITION_SLIDE_FACTOR = 1f
const val RECEDE_SLIDE_FACTOR = 0.33f
